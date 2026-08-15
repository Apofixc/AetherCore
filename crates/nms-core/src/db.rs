// Подсистема базы данных NMS на базе SQLx (SQLite / PostgreSQL)
// Управляет инициализацией таблиц, пулом соединений и миграциями схемы

pub use crate::auth::{hash_password, verify_password};
use anyhow::Result;
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::path::PathBuf;
use tracing::info;

/// Инициализация базы данных и пула соединений SQLite (alias 1-в-1 для `init_db_pool`)
pub async fn init_db(db_path: &PathBuf) -> Result<Pool<Sqlite>> {
    init_db_pool(db_path).await
}

/// Инициализация пула соединений SQLite и создание таблиц БД
pub async fn init_db_pool(db_path: &PathBuf) -> Result<Pool<Sqlite>> {
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let connection_str = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&connection_str)
        .await?;

    // Выполнение PRAGMA инструкций для обеспечения целостности и производительности SQLite WAL
    sqlx::query("PRAGMA journal_mode=WAL;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA busy_timeout=30000;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA synchronous=NORMAL;")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA foreign_keys=ON;")
        .execute(&pool)
        .await?;

    init_tables(&pool).await?;
    seed_initial_data(&pool).await?;

    info!("Database pool initialized successfully at {:?}", db_path);
    Ok(pool)
}

/// Асинхронное создание базовых таблиц системы
async fn init_tables(pool: &Pool<Sqlite>) -> Result<()> {
    // 1. Таблица ролей
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS roles (
            id TEXT PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            description TEXT,
            is_system BOOLEAN DEFAULT 0
        );",
    )
    .execute(pool)
    .await?;

    // 2. Таблица разрешений
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS permissions (
            id TEXT PRIMARY KEY,
            category TEXT NOT NULL,
            name TEXT NOT NULL,
            description TEXT,
            module_id TEXT DEFAULT NULL
        );",
    )
    .execute(pool)
    .await?;

    // 3. Связь ролей и разрешений
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS role_permissions (
            role_id TEXT NOT NULL,
            permission_id TEXT NOT NULL,
            PRIMARY KEY (role_id, permission_id),
            FOREIGN KEY (role_id) REFERENCES roles (id) ON DELETE CASCADE,
            FOREIGN KEY (permission_id) REFERENCES permissions (id) ON DELETE CASCADE
        );",
    )
    .execute(pool)
    .await?;

    // 4. Таблица пользователей
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            full_name TEXT NOT NULL DEFAULT '',
            email TEXT,
            uid TEXT UNIQUE NOT NULL DEFAULT '',
            hashed_password TEXT NOT NULL DEFAULT '',
            is_active BOOLEAN DEFAULT 1,
            role_id TEXT NOT NULL,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            last_login TIMESTAMP,
            avatar TEXT,
            token_valid_after INTEGER DEFAULT 0,
            must_change_password BOOLEAN DEFAULT 0,
            failed_login_attempts INTEGER DEFAULT 0,
            locked_until TIMESTAMP,
            title TEXT DEFAULT '',
            last_seen TIMESTAMP,
            mfa_enabled INTEGER DEFAULT 0,
            mfa_secret TEXT,
            mfa_recovery_codes TEXT,
            FOREIGN KEY (role_id) REFERENCES roles (id)
        );",
    )
    .execute(pool)
    .await?;

    // 5. Таблица логов аудита
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            user_id TEXT,
            username TEXT NOT NULL,
            action TEXT NOT NULL,
            resource TEXT NOT NULL,
            details TEXT,
            ip_address TEXT
        );",
    )
    .execute(pool)
    .await?;

    // 6. Таблица системных настроек (Key-Value)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS system_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .execute(pool)
    .await?;

    // 7. Таблица активных сессий пользователей
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS active_sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            token_jti TEXT NOT NULL,
            ip_address TEXT,
            user_agent TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            last_seen TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            is_revoked BOOLEAN DEFAULT 0,
            FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
        );",
    )
    .execute(pool)
    .await?;

    // 8. Таблица системных удаленных источников логов
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS remote_log_sources (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            url TEXT NOT NULL,
            api_token TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .execute(pool)
    .await?;

    // 9. Таблица журнала системных событий (для WebSocket replay/recovery)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS system_events_journal (
            seq_id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            payload TEXT NOT NULL,
            target_user_id TEXT,
            topic TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_seq_id ON system_events_journal(seq_id);")
        .execute(pool)
        .await?;

    // 10. Таблица уведомлений
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS notifications (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            module_id TEXT NOT NULL DEFAULT 'core',
            user_id TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT DEFAULT '',
            severity TEXT DEFAULT 'info',
            category TEXT DEFAULT 'system',
            entity_id TEXT DEFAULT NULL,
            target_url TEXT DEFAULT NULL,
            group_count INTEGER DEFAULT 1,
            actions TEXT DEFAULT NULL,
            acknowledged_at REAL DEFAULT NULL,
            acknowledged_by TEXT DEFAULT NULL,
            escalated_at REAL DEFAULT NULL,
            title_template TEXT DEFAULT NULL,
            created_at REAL NOT NULL,
            read_at REAL DEFAULT NULL
        );",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_notifications_user_read ON notifications(user_id, read_at);",
    )
    .execute(pool)
    .await?;

    // 11. Таблица предпочтений уведомлений пользователей
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS notification_preferences (
            user_id TEXT PRIMARY KEY,
            push_enabled BOOLEAN DEFAULT 1,
            sound_enabled BOOLEAN DEFAULT 1,
            muted_categories TEXT DEFAULT '[]',
            subscribed_modules TEXT DEFAULT NULL,
            module_rules TEXT DEFAULT '{}',
            sound_signals TEXT DEFAULT '{}',
            muted_until REAL DEFAULT NULL,
            quiet_hours TEXT DEFAULT '{}'
        );",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Инициализация начальных данных (роли, права, пользователь root)
async fn seed_initial_data(pool: &Pool<Sqlite>) -> Result<()> {
    // 1. Системные роли (Superuser, Admin, Operator, Viewer + legacy role-admin)
    let default_roles = [
        (
            "1",
            "Superuser",
            "Полный доступ к системе и ее конфигурации",
            1,
        ),
        (
            "2",
            "Admin",
            "Административный контроль, ограничение на удаление",
            1,
        ),
        (
            "3",
            "Operator",
            "Управление конфигурациями и мониторингом",
            1,
        ),
        ("4", "Viewer", "Только чтение параметров и логов", 1),
        ("role-admin", "Administrator", "Full system access", 1),
    ];

    for (r_id, r_name, r_desc, r_sys) in default_roles {
        sqlx::query(
            "INSERT OR IGNORE INTO roles (id, name, description, is_system) VALUES (?, ?, ?, ?);",
        )
        .bind(r_id)
        .bind(r_name)
        .bind(r_desc)
        .bind(r_sys)
        .execute(pool)
        .await?;
    }

    // 2. Системные разрешения (Permissions)
    let default_permissions = [
        (
            "system.all",
            "Система",
            "Полный доступ",
            "Полные права суперпользователя",
        ),
        (
            "system.admin",
            "Система",
            "Администрирование",
            "Просмотр логов, бэкапы, управление сессиями",
        ),
        (
            "users.view",
            "Пользователи",
            "Просмотр пользователей",
            "Просмотр списка пользователей и их данных",
        ),
        (
            "users.manage",
            "Пользователи",
            "Управление пользователями",
            "Создание, редактирование и удаление пользователей",
        ),
        (
            "roles.view",
            "Доступ",
            "Просмотр ролей",
            "Просмотр списка ролей и прав",
        ),
        (
            "roles.manage",
            "Доступ",
            "Управление ролями",
            "Изменение матрицы прав доступа и создание ролей",
        ),
        (
            "settings.view",
            "Настройки",
            "Просмотр настроек",
            "Просмотр системных настроек и конфигурации",
        ),
        (
            "settings.edit",
            "Настройки",
            "Изменение настроек",
            "Редактирование параметров системы и модулей",
        ),
        (
            "modules.view",
            "Модули",
            "Просмотр модулей",
            "Просмотр списка доступных модулей и статусов",
        ),
        (
            "modules.manage",
            "Модули",
            "Управление модулями",
            "Включение и выключение плагинов",
        ),
        (
            "audit.view",
            "Аудит",
            "Просмотр журнала аудита",
            "Доступ к событиям безопасности и журналам",
        ),
        (
            "audit.export",
            "Аудит",
            "Экспорт аудита",
            "Экспорт журнала аудита безопасности",
        ),
    ];

    for (p_id, p_cat, p_name, p_desc) in default_permissions {
        sqlx::query(
            "INSERT OR IGNORE INTO permissions (id, category, name, description) VALUES (?, ?, ?, ?);",
        )
        .bind(p_id)
        .bind(p_cat)
        .bind(p_name)
        .bind(p_desc)
        .execute(pool)
        .await?;
    }

    // 3. Матрица прав для системных ролей
    sqlx::query("DELETE FROM role_permissions WHERE role_id IN ('1', '2', '3', '4');")
        .execute(pool)
        .await?;

    // Роль 1 (Superuser) — все разрешения
    for (p_id, _, _, _) in default_permissions {
        sqlx::query(
            "INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES ('1', ?);",
        )
        .bind(p_id)
        .execute(pool)
        .await?;
    }

    // Роль 2 (Admin) — администрирование системы и пользователей
    let admin_perms = [
        "system.admin",
        "users.view",
        "users.manage",
        "roles.view",
        "roles.manage",
        "settings.view",
        "settings.edit",
        "modules.view",
        "modules.manage",
        "audit.view",
        "audit.export",
    ];
    for p_id in admin_perms {
        sqlx::query(
            "INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES ('2', ?);",
        )
        .bind(p_id)
        .execute(pool)
        .await?;
    }

    // Роль 3 (Operator) — просмотр и изменение настроек/модулей
    let operator_perms = [
        "settings.view",
        "settings.edit",
        "modules.view",
        "audit.view",
    ];
    for p_id in operator_perms {
        sqlx::query(
            "INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES ('3', ?);",
        )
        .bind(p_id)
        .execute(pool)
        .await?;
    }

    // Роль 4 (Viewer) — только чтение аудита
    let viewer_perms = ["audit.view"];
    for p_id in viewer_perms {
        sqlx::query(
            "INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES ('4', ?);",
        )
        .bind(p_id)
        .execute(pool)
        .await?;
    }

    // 4. Миграция admin -> root
    sqlx::query(
        "UPDATE users SET username = 'root', full_name = 'Главный администратор (Root)', uid = 'ROOT-001' WHERE username = 'admin';",
    )
    .execute(pool)
    .await?;

    // 5. Инициализация системного пользователя root (пароль по умолчанию: admin)
    let root_user: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE username = 'root'")
            .fetch_optional(pool)
            .await?;

    if root_user.is_none() {
        let pass_hash = hash_password("admin")?;
        sqlx::query(
            "INSERT INTO users (id, username, full_name, email, uid, hashed_password, is_active, role_id)
             VALUES (?, ?, ?, ?, ?, ?, 1, '1');",
        )
        .bind("usr-root-01")
        .bind("root")
        .bind("Главный администратор (Root)")
        .bind("root@nms.local")
        .bind("ROOT-001")
        .bind(pass_hash)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Получить системную настройку из БД по ключу
pub async fn get_system_setting(pool: &Pool<Sqlite>, key: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM system_settings WHERE key = ?;")
        .bind(key)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| r.0))
}

/// Сохранить или обновить системную настройку в БД (UPSERT)
pub async fn set_system_setting(pool: &Pool<Sqlite>, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO system_settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;

    Ok(())
}

/// Сохранение события в персистентный журнал SQLite (system_events_journal)
pub async fn record_event_in_db(
    pool: &Pool<Sqlite>,
    event_type: &str,
    payload: &serde_json::Value,
    target_user_id: Option<&str>,
    topic: Option<&str>,
) -> Result<i64> {
    let payload_json = serde_json::to_string(payload)?;
    let result = sqlx::query(
        "INSERT INTO system_events_journal (event_type, payload, target_user_id, topic)
         VALUES (?, ?, ?, ?);",
    )
    .bind(event_type)
    .bind(payload_json)
    .bind(target_user_id)
    .bind(topic)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Выборка пропущенных событий из журнала БД по seq_id для повтора (replay)
pub async fn get_missed_events_from_db(
    pool: &Pool<Sqlite>,
    from_seq_id: i64,
    target_user_id: Option<&str>,
    topic_filter: Option<&str>,
    limit: i64,
) -> Result<Vec<crate::bus::SystemEvent>> {
    let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>, String)>(
        "SELECT seq_id, event_type, payload, target_user_id, topic, created_at
         FROM system_events_journal
         WHERE seq_id > ?
           AND (target_user_id IS NULL OR ? IS NULL OR target_user_id = ?)
           AND (topic IS NULL OR ? IS NULL OR topic = ? OR ? = '*' OR ? = '#')
         ORDER BY seq_id ASC
         LIMIT ?;",
    )
    .bind(from_seq_id)
    .bind(target_user_id)
    .bind(target_user_id)
    .bind(topic_filter)
    .bind(topic_filter)
    .bind(topic_filter.unwrap_or(""))
    .bind(topic_filter.unwrap_or(""))
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut events = Vec::new();
    for (seq_id, event_type, payload_str, target_uid, topic, created_at) in rows {
        let payload: serde_json::Value =
            serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null);
        events.push(crate::bus::SystemEvent {
            seq_id: Some(seq_id),
            topic: topic.unwrap_or(event_type),
            payload,
            sender: "journal_replay".to_string(),
            target_user_id: target_uid,
            created_at: Some(created_at),
        });
    }

    Ok(events)
}

/// Асинхронная очередь пакетной записи событий в SQLite без блокировок
#[derive(Clone)]
pub struct EventJournalQueue {
    sender: tokio::sync::mpsc::Sender<(String, serde_json::Value, Option<String>, Option<String>)>,
}

impl EventJournalQueue {
    /// Создание и запуск фонового флаш-цикла очереди пакетной записи
    pub fn new(pool: Pool<Sqlite>, flush_interval_ms: u64) -> Self {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<(
            String,
            serde_json::Value,
            Option<String>,
            Option<String>,
        )>(10000);

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(flush_interval_ms));
            let mut batch = Vec::new();

            loop {
                tokio::select! {
                    Some(item) = receiver.recv() => {
                        batch.push(item);
                        if batch.len() >= 500 {
                            Self::flush_batch(&pool, &mut batch).await;
                        }
                    }
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            Self::flush_batch(&pool, &mut batch).await;
                        }
                    }
                }
            }
        });

        Self { sender }
    }

    async fn flush_batch(
        pool: &Pool<Sqlite>,
        batch: &mut Vec<(String, serde_json::Value, Option<String>, Option<String>)>,
    ) {
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(err) => {
                tracing::error!("Failed to begin transaction for event batch: {}", err);
                batch.clear();
                return;
            }
        };

        for (event_type, payload, target_uid, topic) in batch.drain(..) {
            let payload_str = serde_json::to_string(&payload).unwrap_or_default();
            let _ = sqlx::query(
                "INSERT INTO system_events_journal (event_type, payload, target_user_id, topic)
                 VALUES (?, ?, ?, ?);",
            )
            .bind(event_type)
            .bind(payload_str)
            .bind(target_uid)
            .bind(topic)
            .execute(&mut *tx)
            .await;
        }

        if let Err(err) = tx.commit().await {
            tracing::error!("Failed to commit event journal batch: {}", err);
        }
    }

    /// Асинхронно отправить событие в пакетную очередь записи
    pub async fn enqueue(
        &self,
        event_type: String,
        payload: serde_json::Value,
        target_user_id: Option<String>,
        topic: Option<String>,
    ) {
        let _ = self
            .sender
            .send((event_type, payload, target_user_id, topic))
            .await;
    }
}
