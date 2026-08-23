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

use aethercore_common::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::info;

/// Статистика хранилища и физического состояния базы данных SQLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStorageStats {
    /// Размер основного файла базы данных в байтах
    pub db_size_bytes: u64,
    /// Размер файла журнала упреждающей записи WAL в байтах
    pub wal_size_bytes: u64,
    /// Размер файла разделяемой памяти SHM в байтах
    pub shm_size_bytes: u64,
    /// Общий размер хранилища на диске в байтах
    pub total_size_bytes: u64,
    /// Размер одной страницы SQLite в байтах
    pub page_size: u32,
    /// Общее количество страниц в базе данных
    pub page_count: u32,
    /// Количество свободных страниц (freelist)
    pub freelist_count: u32,
    /// Количество пользовательских таблиц
    pub tables_count: u32,
    /// Активен ли режим WAL
    pub wal_mode: bool,
}

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
    /// Путь к файлу базы данных на диске (если не in-memory)
    db_path: Option<PathBuf>,
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
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое создания каталога,
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
            db_path: Some(db_path.to_path_buf()),
        };

        // Запуск миграций схемы
        db.run_migrations().await?;
        info!("SQLite database initialized in WAL mode at {:?}", db_path);

        Ok(db)
    }

    /// Инициализировать изолированную базу данных в оперативной памяти (для unit/integration тестов)
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое создания in-memory пула или миграций.
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
            db_path: None,
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

    /// Получить путь к файлу базы данных на диске
    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    /// Получить подробную статистику физического хранилища базы данных
    pub async fn get_storage_stats(&self) -> Result<DbStorageStats> {
        let page_size: i64 = sqlx::query_scalar("PRAGMA page_size;")
            .fetch_one(&self.reader_pool)
            .await
            .unwrap_or(4096);

        let page_count: i64 = sqlx::query_scalar("PRAGMA page_count;")
            .fetch_one(&self.reader_pool)
            .await
            .unwrap_or(0);

        let freelist_count: i64 = sqlx::query_scalar("PRAGMA freelist_count;")
            .fetch_one(&self.reader_pool)
            .await
            .unwrap_or(0);

        let tables_count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%';",
        )
        .fetch_one(&self.reader_pool)
        .await
        .unwrap_or(0);

        let mut db_size = (page_size * page_count) as u64;
        let mut wal_size = 0u64;
        let mut shm_size = 0u64;

        if let Some(path) = &self.db_path {
            if let Ok(meta) = tokio::fs::metadata(path).await {
                db_size = meta.len();
            }

            let wal_path = PathBuf::from(format!("{}-wal", path.display()));
            if let Ok(meta) = tokio::fs::metadata(&wal_path).await {
                wal_size = meta.len();
            }

            let shm_path = PathBuf::from(format!("{}-shm", path.display()));
            if let Ok(meta) = tokio::fs::metadata(&shm_path).await {
                shm_size = meta.len();
            }
        }

        let total_size = db_size + wal_size + shm_size;

        Ok(DbStorageStats {
            db_size_bytes: db_size,
            wal_size_bytes: wal_size,
            shm_size_bytes: shm_size,
            total_size_bytes: total_size,
            page_size: page_size as u32,
            page_count: page_count as u32,
            freelist_count: freelist_count as u32,
            tables_count: tables_count as u32,
            wal_mode: true,
        })
    }

    /// Выполнить создание и миграции схемы реляционных таблиц SQLite
    ///
    /// Создает таблицы `users`, `roles`, `permissions`, `role_permissions`, `user_roles`,
    /// `kv_store`, `audit_logs`, `event_journal` и заполняет системные роли/права по умолчанию.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое выполнения DDL/DML запросов.
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
                is_username_locked INTEGER NOT NULL DEFAULT 0,
                is_totp_enabled INTEGER NOT NULL DEFAULT 0,
                totp_secret TEXT,
                totp_backup_codes TEXT,
                login_count INTEGER NOT NULL DEFAULT 0,
                failed_login_attempts INTEGER NOT NULL DEFAULT 0,
                locked_until TEXT,
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
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN is_username_locked INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await;
        let _ = sqlx::query("UPDATE users SET is_username_locked = 1 WHERE username = 'root' OR last_login_at IS NOT NULL")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN is_totp_enabled INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN totp_secret TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN totp_backup_codes TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN department TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN login_count INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN failed_login_attempts INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN locked_until TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::query("ALTER TABLE users ADD COLUMN force_2fa INTEGER")
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

        // 6. Таблицы планировщика задач (Task Scheduler & Execution History)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS scheduled_tasks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                schedule_type TEXT NOT NULL,
                schedule_value TEXT NOT NULL,
                action_type TEXT NOT NULL,
                action_params TEXT,
                concurrency_policy TEXT NOT NULL DEFAULT 'skip',
                misfire_policy TEXT NOT NULL DEFAULT 'skip_to_next',
                timeout_secs INTEGER NOT NULL DEFAULT 300,
                is_enabled INTEGER NOT NULL DEFAULT 1,
                is_system INTEGER NOT NULL DEFAULT 0,
                next_run_at TEXT,
                last_run_at TEXT,
                last_status TEXT,
                last_error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_next_run ON scheduled_tasks(next_run_at, is_enabled);

            CREATE TABLE IF NOT EXISTS task_execution_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                task_name TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                status TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                error_message TEXT,
                triggered_by TEXT NOT NULL,
                FOREIGN KEY (task_id) REFERENCES scheduled_tasks(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_task_history_task_id ON task_execution_history(task_id);
            CREATE INDEX IF NOT EXISTS idx_task_history_started_at ON task_execution_history(started_at);
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::database(format!("Failed to create scheduler tables: {}", e)))?;

        // Сидирование стандартных ролей и прав
        self.seed_default_rbac().await?;
        self.seed_default_tasks().await?;

        Ok(())
    }

    /// Заполнить базу данных стандартными системными ролями и правами доступа (RBAC seeding)
    ///
    /// Регистрирует категории прав `System`, `Users`, `Modules`, `Events`,
    /// создает роли `admin`, `operator`, `viewer` и настраивает базовые связи `role_permissions`.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое вставки в таблицы прав и ролей.
    async fn seed_default_rbac(&self) -> Result<()> {
        let pool = &self.writer_pool;

        // Встроенные права (4 основных домена системы)
        let default_permissions = [
            ("modules.view", "Просмотр модулей", "Modules", "Просмотр установленных плагинов и телеметрии"),
            ("modules.manage", "Управление модулями", "Modules", "Установка, включение, отключение и настройка плагинов"),
            ("users.view", "Просмотр пользователей", "Users", "Просмотр списка учетных записей"),
            ("users.manage", "Управление пользователями", "Users", "Создание, редактирование и блокировка пользователей"),
            ("access.view", "Просмотр безопасности и доступа", "Access", "Просмотр политик 2FA, IP, матрицы прав и журнала аудита"),
            ("access.manage", "Управление безопасностью и доступом", "Access", "Настройка политик 2FA, IP, матрицы прав и ротация аудита"),
            ("system.view", "Просмотр системы", "System", "Просмотр системного статуса, логов, метрик и базы данных"),
            ("system.manage", "Управление системой", "System", "Резервное копирование, обслуживание, ротация логов и перезапуск"),
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
            "modules.view", "modules.manage",
            "users.view", "users.manage",
            "access.view", "access.manage",
            "system.view", "system.manage",
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

        let operator_perms = [
            "modules.view", "users.view", "access.view", "system.view",
        ];
        for perm in operator_perms {
            sqlx::query(
                "INSERT OR IGNORE INTO role_permissions (role_name, permission_id) VALUES ('operator', ?)",
            )
            .bind(perm)
            .execute(pool)
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        }

        let viewer_perms = ["modules.view", "system.view"];
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

    /// Сидировать стандартные системные задачи планировщика по умолчанию
    async fn seed_default_tasks(&self) -> Result<()> {
        let pool = &self.writer_pool;
        let now = chrono::Utc::now().to_rfc3339();

        // 1. Системная задача ротации аудита раз в сутки (в полночь: 0 0 * * *)
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO scheduled_tasks (
                id, name, description, schedule_type, schedule_value,
                action_type, action_params, concurrency_policy, misfire_policy,
                timeout_secs, is_enabled, is_system, next_run_at, created_at, updated_at
            ) VALUES (
                'sys-audit-retention',
                'Ротация и архивация журнала аудита',
                'Автоматическое архивирование и очистка записей аудита старше установленного срока',
                'cron',
                '0 0 * * *',
                'system_audit_rotation',
                NULL,
                'skip',
                'skip_to_next',
                600,
                1,
                1,
                ?,
                ?,
                ?
            )
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        // 2. Системная задача очистки старой истории планировщика раз в сутки (в 03:00)
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO scheduled_tasks (
                id, name, description, schedule_type, schedule_value,
                action_type, action_params, concurrency_policy, misfire_policy,
                timeout_secs, is_enabled, is_system, next_run_at, created_at, updated_at
            ) VALUES (
                'sys-history-cleanup',
                'Очистка журнала выполнения планировщика',
                'Автоматическое удаление записей истории выполнения задач старше 30 дней',
                'cron',
                '0 3 * * *',
                'system_history_cleanup',
                NULL,
                'skip',
                'skip_to_next',
                300,
                1,
                1,
                ?,
                ?,
                ?
            )
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        // 3. Системная задача автоматического бэкапа SQLite раз в сутки (в 04:00)
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO scheduled_tasks (
                id, name, description, schedule_type, schedule_value,
                action_type, action_params, concurrency_policy, misfire_policy,
                timeout_secs, is_enabled, is_system, next_run_at, created_at, updated_at
            ) VALUES (
                'sys-auto-backup',
                'Автоматическое резервное копирование БД',
                'Создание ежедневного снимка SQLite и ротация устаревших копий',
                'cron',
                '0 4 * * *',
                'system_db_backup',
                NULL,
                'skip',
                'skip_to_next',
                600,
                1,
                1,
                ?,
                ?,
                ?
            )
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        Ok(())
    }
}
