//! # Подсистема базы данных SQLite (WAL Mode, Single-Writer / Multi-Reader)
//!
//! Гарантирует отсутствие конфликтов блокировок `database is locked` при высокочастотной
//! круглосуточной записи телеметрии, метрик и системных событий.
//!
//! ## Реляционная схема:
//! - `users`: Учетные записи операторов и администраторов (Argon2id пароли, статусы активности).
//! - `roles`, `permissions`, `role_permissions`, `user_roles`: Ролевая модель доступа RBAC.
//! - `kv_store`: Изолированное хранилище конфигураций плагинов (`module:{id}`) и системы (`system`).
//! - `event_journal`: Надежный журнал системных событий с индексацией по топикам и времени.
//! - `audit_logs`: Журнал аудита действий пользователей и подсистем.

pub mod kv;

use nms_common::error::{AppError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::Path;
use std::str::FromStr;
use tracing::info;

/// Менеджер базы данных платформы SQLite (Single-Writer / Multi-Reader)
///
/// Разделяет соединения на:
/// - `writer_pool`: строго одно соединение для записи (исключает `SQLITE_BUSY` и блокировки WAL).
/// - `reader_pool`: масштабируемый пул соединений для параллельного неблокирующего чтения.
#[derive(Debug, Clone)]
pub struct Db {
    /// Пул для монопольной записи (1 соединение)
    writer_pool: Pool<Sqlite>,
    /// Масштабируемый пул для параллельного чтения
    reader_pool: Pool<Sqlite>,
}

impl Db {
    /// Инициализировать базу данных SQLite по указанному пути в режиме WAL
    ///
    /// Автоматически создает директорию, настраивает WAL mode, `busy_timeout`,
    /// foreign keys и накатывает все встроенные миграции схемы.
    ///
    /// # Аргументы
    /// * `db_path` — Путь к файлу базы данных SQLite на диске.
    /// * `max_readers` — Максимальный размер пула соединений на чтение.
    /// * `busy_timeout_ms` — Таймаут ожидания освобождения базы данных в миллисекундах.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое создания каталога,
    /// подключения к SQLite или выполнения миграций схемы.
    pub async fn init(db_path: &Path, max_readers: u32, busy_timeout_ms: u64) -> Result<Self> {
        // Создаем родительскую директорию, если она не существует
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    AppError::database(format!("Failed to create database directory {:?}: {}", parent, e))
                })?;
            }
        }

        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let connect_opts = SqliteConnectOptions::from_str(&db_url)
            .map_err(|e| AppError::database(format!("Invalid SQLite connection string: {}", e)))?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_millis(busy_timeout_ms))
            .foreign_keys(true);

        // 1. Single-Writer Pool: ровно 1 соединение на запись
        let writer_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .connect_with(connect_opts.clone())
            .await
            .map_err(|e| AppError::database(format!("Failed to create SQLite writer pool: {}", e)))?;

        // 2. Multi-Reader Pool: пул соединений на чтение
        let reader_pool = SqlitePoolOptions::new()
            .max_connections(max_readers.max(2))
            .min_connections(1)
            .connect_with(connect_opts)
            .await
            .map_err(|e| AppError::database(format!("Failed to create SQLite reader pool: {}", e)))?;

        let db = Self {
            writer_pool,
            reader_pool,
        };

        // Запуск миграций схемы
        db.run_migrations().await?;
        info!("SQLite database initialized in WAL mode at {:?}", db_path);

        Ok(db)
    }

    /// Инициализировать изолированную базу данных в оперативной памяти (для unit/integration тестов)
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое создания in-memory пула или миграций.
    pub async fn init_in_memory() -> Result<Self> {
        let connect_opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(|e| AppError::database(e.to_string()))?
            .busy_timeout(std::time::Duration::from_millis(5000))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_opts)
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        let db = Self {
            writer_pool: pool.clone(),
            reader_pool: pool,
        };

        db.run_migrations().await?;
        Ok(db)
    }

    /// Получить ссылку на пул соединений для операций записи (INSERT, UPDATE, DELETE)
    pub fn writer(&self) -> &Pool<Sqlite> {
        &self.writer_pool
    }

    /// Получить ссылку на пул соединений для параллельных операций чтения (SELECT)
    pub fn reader(&self) -> &Pool<Sqlite> {
        &self.reader_pool
    }

    /// Выполнить создание и миграции схемы реляционных таблиц SQLite
    ///
    /// Создает таблицы `users`, `roles`, `permissions`, `role_permissions`, `user_roles`,
    /// `kv_store`, `audit_logs`, `event_journal` и заполняет системные роли/права по умолчанию.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое выполнения DDL/DML запросов.
    async fn run_migrations(&self) -> Result<()> {
        let pool = &self.writer_pool;

        // 1. Таблица пользователей
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                full_name TEXT,
                email TEXT,
                department TEXT,
                password_hash TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                is_superuser INTEGER NOT NULL DEFAULT 0,
                must_change_password INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_login_at TEXT
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::database(format!("Failed to create users table: {}", e)))?;

        // Миграции существующей таблицы users
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN must_change_password INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN department TEXT")
            .execute(pool)
            .await;

        // 2. Таблицы RBAC (роли и права)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS roles (
                name TEXT PRIMARY KEY,
                description TEXT NOT NULL,
                is_system INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS permissions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                category TEXT NOT NULL,
                description TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS role_permissions (
                role_name TEXT NOT NULL,
                permission_id TEXT NOT NULL,
                PRIMARY KEY (role_name, permission_id),
                FOREIGN KEY (role_name) REFERENCES roles(name) ON DELETE CASCADE,
                FOREIGN KEY (permission_id) REFERENCES permissions(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS user_roles (
                user_id TEXT NOT NULL,
                role_name TEXT NOT NULL,
                PRIMARY KEY (user_id, role_name),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (role_name) REFERENCES roles(name) ON DELETE CASCADE
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::database(format!("Failed to create RBAC tables: {}", e)))?;

        // 3. Таблица Key-Value хранилища (изолированное состояние ядра и плагинов)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS kv_store (
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (namespace, key)
            );
            CREATE INDEX IF NOT EXISTS idx_kv_namespace ON kv_store(namespace);
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::database(format!("Failed to create kv_store table: {}", e)))?;

        // 4. Таблица журнала системных событий (Reliable Event Journal)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS event_journal (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_uuid TEXT NOT NULL UNIQUE,
                topic TEXT NOT NULL,
                source TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_event_topic ON event_journal(topic);
            CREATE INDEX IF NOT EXISTS idx_event_created_at ON event_journal(created_at);
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::database(format!("Failed to create event_journal table: {}", e)))?;

        // 5. Таблица журнала аудита
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT,
                username TEXT,
                action TEXT NOT NULL,
                resource TEXT NOT NULL,
                status TEXT NOT NULL,
                details TEXT,
                ip_address TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audit_created_at ON audit_logs(created_at);
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::database(format!("Failed to create audit_logs table: {}", e)))?;

        // Сидирование стандартных ролей и прав
        self.seed_default_rbac().await?;

        Ok(())
    }

    /// Заполнить базу данных стандартными системными ролями и правами доступа (RBAC seeding)
    ///
    /// Регистрирует категории прав `System`, `Users`, `Modules`, `Events`,
    /// создает роли `admin`, `operator`, `viewer` и настраивает базовые связи `role_permissions`.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое вставки в таблицы прав и ролей.
    async fn seed_default_rbac(&self) -> Result<()> {
        let pool = &self.writer_pool;

        // Встроенные права
        let default_permissions = [
            ("system.view", "Просмотр системы", "System", "Просмотр системного статуса и настроек"),
            ("system.manage", "Управление системой", "System", "Изменение системных параметров и перезапуск"),
            ("users.view", "Просмотр пользователей", "Users", "Просмотр списка учетных записей"),
            ("users.manage", "Управление пользователями", "Users", "Создание, редактирование и блокировка пользователей"),
            ("modules.view", "Просмотр модулей", "Modules", "Просмотр установленных плагинов"),
            ("modules.manage", "Управление модулями", "Modules", "Установка, включение, отключение и настройка плагинов"),
            ("events.view", "Просмотр событий", "Events", "Чтение системного журнала событий"),
        ];

        for (id, name, cat, desc) in default_permissions {
            sqlx::query(
                "INSERT OR IGNORE INTO permissions (id, name, category, description) VALUES (?, ?, ?, ?)",
            )
            .bind(id)
            .bind(name)
            .bind(cat)
            .bind(desc)
            .execute(pool)
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        }

        // Встроенные роли
        let default_roles = [
            ("superuser", "Суперпользователь с наивысшими правами и полным управлением системой", true),
            ("admin", "Полный административный доступ ко всем сервисам", true),
            ("operator", "Оператор мониторинга и управления конфигурациями", true),
            ("viewer", "Только просмотр статусов и метрик", true),
        ];

        for (name, desc, is_sys) in default_roles {
            sqlx::query(
                "INSERT OR IGNORE INTO roles (name, description, is_system) VALUES (?, ?, ?)",
            )
            .bind(name)
            .bind(desc)
            .bind(is_sys)
            .execute(pool)
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        }

        // Назначение прав ролям
        let admin_perms = [
            "system.view", "system.manage", "users.view", "users.manage",
            "modules.view", "modules.manage", "events.view",
        ];
        for perm in admin_perms {
            sqlx::query(
                "INSERT OR IGNORE INTO role_permissions (role_name, permission_id) VALUES ('admin', ?)",
            )
            .bind(perm)
            .execute(pool)
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        }

        let viewer_perms = ["system.view", "modules.view", "events.view"];
        for perm in viewer_perms {
            sqlx::query(
                "INSERT OR IGNORE INTO role_permissions (role_name, permission_id) VALUES ('viewer', ?)",
            )
            .bind(perm)
            .execute(pool)
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        }

        Ok(())
    }
}
