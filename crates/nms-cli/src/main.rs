// Точка входа в исполняемое приложение NMS (nms)
// Поддерживает запуск в режиме серверного демона (--server) и локального интерфейса

use clap::Parser;
use nms_core::{setup_logging, start_server, AppConfig};
use tracing::info;

/// Разбор параметров командной строки NMS
#[derive(Parser, Debug)]
#[command(author, version, about = "NMS Next-Gen Server Daemon & Desktop Core", long_about = None)]
struct CliArgs {
    /// Запустить приложение в режиме бессерверного демона (Headless Server Daemon)
    #[arg(long, default_value_t = false)]
    server: bool,

    /// Порт для веб-сервера HTTP/WS
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Сетевой адрес хоста для прослушивания
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();

    // Инициализация подсистемы системного логирования tracing (stdout + backend.log)
    setup_logging(Some(std::path::PathBuf::from("backend.log")))?;

    info!(
        "Starting NMS application core (version {})...",
        env!("CARGO_PKG_VERSION")
    );

    let config = AppConfig {
        host: if args.server && args.host == "127.0.0.1" {
            // Если указан флаг --server, по умолчанию принимаем соединения со всех интерфейсов
            "0.0.0.0".to_string()
        } else {
            args.host
        },
        port: args.port,
        modules_dir: "./modules".into(),
        is_server_mode: args.server,
        ..AppConfig::default()
    };

    if args.server {
        info!("Running in headless server daemon mode");
    } else {
        info!("Running in standard mode");
    }

    // Запуск веб-сервера Axum
    start_server(config).await?;

    Ok(())
}
