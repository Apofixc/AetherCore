//! # Исполняемый файл и CLI утилита платформы (aethercore-cli)
//!
//! Точка входа для запуска ядра в режимах Server, Dev и Safe-Mode,
//! а также сборки и упаковки плагинов (`aethercore plugin pack <dir> -o <output>`).

use clap::{Parser, Subcommand};
use aethercore_common::config::AppConfig;
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::loader::PluginPackage;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, LoggerService, LoggerServiceLayer, NotifyService};
use aethercore_core::users::UserService;
use aethercore_server::state::AppState;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::info;

/// Параметры командной строки ядра AetherCore Platform
#[derive(Parser, Debug)]
#[command(name = "aethercore", version, about = "AetherCore Universal Modular Platform")]
struct Cli {
    /// Запуск в режиме Headless сервера (по умолчанию true)
    #[arg(long)]
    server: bool,

    /// Хост привязки HTTP/WebSocket сервера (например, 127.0.0.1 или 0.0.0.0)
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// TCP порт HTTP/WebSocket сервера
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Режим разработки (разрешение неподписанных плагинов и детальный лог)
    #[arg(long)]
    dev: bool,

    /// Аварийный режим Safe-Mode (старт ядра без загрузки сторонних модулей)
    #[arg(long)]
    safe_mode: bool,

    /// Путь к файлу базы данных SQLite
    #[arg(long, default_value = "data/aethercore.db")]
    db: PathBuf,

    /// Каталог хранения и сканирования плагинов (.aether-plugin)
    #[arg(long, default_value = "modules")]
    modules_dir: PathBuf,

    /// Опциональная подкоманда CLI
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Набор поддерживаемых подкоманд CLI
#[derive(Subcommand, Debug)]
enum Commands {
    /// Управление модулями и плагинами (.aether-plugin)
    Plugin {
        /// Действие с плагином
        #[command(subcommand)]
        action: PluginCommands,
    },
}

/// Подкоманды управления пакетами плагинов
#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// Упаковка каталога разработки в ZIP архив .aether-plugin
    Pack {
        /// Путь к каталогу исходных файлов плагина
        dir: String,
        /// Путь к выходному файлу .aether-plugin
        #[arg(short, long)]
        output: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let logger_service = LoggerService::with_log_file("data/aethercore.log");

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cli.dev {
            tracing_subscriber::EnvFilter::new("info,tower_http=debug,aethercore_server=debug,aethercore_core=debug")
        } else {
            tracing_subscriber::EnvFilter::new("info")
        }
    });

    let fmt_layer = tracing_subscriber::fmt::layer();
    let logger_layer = LoggerServiceLayer::new(logger_service.clone());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(logger_layer)
        .init();

    // Обработка подкоманд CLI
    if let Some(Commands::Plugin { action }) = cli.command {
        match action {
            PluginCommands::Pack { dir, output } => {
                info!("Packing plugin from directory: {}", dir);
                let zip_bytes = PluginPackage::pack(Path::new(&dir), None)?;
                tokio::fs::write(&output, zip_bytes).await?;
                info!("Plugin successfully packed to {}", output);
                return Ok(());
            }
        }
    }

    info!("Starting Next-Gen Universal Core Platform v{}", env!("CARGO_PKG_VERSION"));

    // 1. Формируем конфигурацию
    let mut config = AppConfig::default();
    config.server.host = cli.host;
    config.server.port = cli.port;
    config.server.dev_mode = cli.dev;
    config.server.safe_mode = cli.safe_mode;
    config.database.path = cli.db;
    config.plugins.dir = cli.modules_dir;

    // 2. Инициализируем базу данных (SQLite WAL)
    let db = Db::init(
        &config.database.path,
        config.database.max_read_connections,
        config.database.busy_timeout_ms,
    )
    .await?;

    // 3. Инициализируем сервисы ядра
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new(&config.security.jwt_secret, config.security.jwt_ttl_seconds);
    let user_service = UserService::new(db.clone());
    let session_service = aethercore_core::services::SessionService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());
    let backup_dir = config
        .database
        .path
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| PathBuf::from("data/backups"));
    let backup_service = aethercore_core::services::BackupService::new(db.clone(), backup_dir);
    let backup_service_arc = std::sync::Arc::new(backup_service.clone());

    let scheduler_service = std::sync::Arc::new(aethercore_core::services::SchedulerService::new(db.clone()));

    // Регистрируем обработчики системных действий в планировщике (Handler Registry)
    scheduler_service
        .register_handler(
            "system_db_backup",
            std::sync::Arc::new(aethercore_core::services::handlers::BackupTaskHandler::new(
                backup_service_arc,
                db.clone(),
            )),
        )
        .await;

    let archive_dir = config
        .database
        .path
        .parent()
        .map(|p| p.join("archives"))
        .unwrap_or_else(|| PathBuf::from("data/archives"));
    scheduler_service
        .register_handler(
            "system_audit_rotation",
            std::sync::Arc::new(aethercore_core::services::handlers::AuditTaskHandler::new(
                audit_service.clone(),
                db.clone(),
                archive_dir,
            )),
        )
        .await;

    scheduler_service
        .register_handler(
            "system_history_cleanup",
            std::sync::Arc::new(aethercore_core::services::handlers::HistoryCleanupTaskHandler::new(
                db.clone(),
            )),
        )
        .await;

    scheduler_service
        .register_handler(
            "plugin_timer",
            std::sync::Arc::new(aethercore_core::services::handlers::PluginTimerHandler::new(
                plugin_manager.clone(),
                bus.clone(),
            )),
        )
        .await;

    scheduler_service
        .register_handler(
            "event_publish",
            std::sync::Arc::new(aethercore_core::services::handlers::EventPublishHandler::new(
                bus.clone(),
            )),
        )
        .await;

    scheduler_service.seed_default_tasks().await?;

    // 4. Проверяем наличие дефолтного администратора
    user_service.ensure_default_admin().await?;

    // 5. Загружаем плагины (если не safe-mode)
    if !config.server.safe_mode {
        info!("Scanning for plugins in {:?}", config.plugins.dir);
        let _ = plugin_manager.load_plugins_from_dir(&config.plugins.dir).await;
    } else {
        info!("Running in SAFE-MODE: skipping user plugins loading");
    }

    // 6. Собираем глобальное состояние AppState
    let state = AppState {
        config: config.clone(),
        db,
        bus,
        jwt_manager,
        user_service,
        session_service,
        audit_service,
        logger_service,
        notify_service,
        plugin_manager,
        scheduler_service,
        backup_service,
        start_time: Instant::now(),
    };

    // 7. Запускаем HTTP веб-сервер Axum
    aethercore_server::run_server(state).await?;

    Ok(())
}
