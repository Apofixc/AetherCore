//! # Исполняемый файл nms и CLI утилита платформы (nms-cli)
//!
//! Точка входа для запуска ядра в режимах Server, Dev и Safe-Mode,
//! а также сборки и упаковки плагинов (`nms plugin pack <dir> -o <output>`).

use clap::{Parser, Subcommand};
use nms_common::config::AppConfig;
use nms_core::auth::JwtManager;
use nms_core::bus::EventBus;
use nms_core::db::Db;
use nms_core::plugins::loader::PluginPackage;
use nms_core::plugins::PluginManager;
use nms_core::services::{AuditService, LoggerService, NotifyService};
use nms_core::users::UserService;
use nms_server::state::AppState;

use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::info;

/// Параметры командной строки ядра NMSNext-Gen
#[derive(Parser, Debug)]
#[command(name = "nms", version, about = "Next-Gen Universal Core Platform")]
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
    #[arg(long, default_value = "data/nms.db")]
    db: PathBuf,

    /// Каталог хранения и сканирования плагинов (.nms-plugin)
    #[arg(long, default_value = "modules")]
    modules_dir: PathBuf,

    /// Опциональная подкоманда CLI
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Набор поддерживаемых подкоманд CLI
#[derive(Subcommand, Debug)]
enum Commands {
    /// Управление модулями и плагинами (.nms-plugin)
    Plugin {
        /// Действие с плагином
        #[command(subcommand)]
        action: PluginCommands,
    },
}

/// Подкоманды управления пакетами плагинов
#[derive(Subcommand, Debug)]
enum PluginCommands {
    /// Упаковка каталога разработки в ZIP архив .nms-plugin
    Pack {
        /// Путь к каталогу исходных файлов плагина
        dir: String,
        /// Путь к выходному файлу .nms-plugin
        #[arg(short, long)]
        output: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

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
    let audit_service = AuditService::new(db.clone());
    let logger_service = LoggerService::with_log_file("data/nms.log");
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());

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
        audit_service,
        logger_service,
        notify_service,
        plugin_manager,
        start_time: Instant::now(),
    };

    // 7. Запускаем HTTP веб-сервер Axum
    nms_server::run_server(state).await?;

    Ok(())
}
