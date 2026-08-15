use std::path::PathBuf;
use std::sync::Mutex;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Глобальное хранилище стража логирования для функции setup_logging() / stop_logging()
static GLOBAL_GUARD: Mutex<Option<LoggingGuard>> = Mutex::new(None);

/// Хранитель фонового потока неблокирующего логирования (защита от потери логов при выходе)
pub struct LoggingGuard {
    _guard: Option<WorkerGuard>,
}

impl LoggingGuard {
    /// Остановка работы неблокирующего аппендера и сброс логов
    pub fn stop(&mut self) {
        self._guard = None;
    }
}

/// Корректная остановка слушателя очереди логов и сброс оставшихся записей (1-в-1 stop_logging из Python)
pub fn stop_logging() {
    if let Ok(mut guard) = GLOBAL_GUARD.lock() {
        if let Some(mut g) = guard.take() {
            g.stop();
        }
    }
}

/// Настройка неблокирующего логгирования для приложения (1-в-1 setup_logging из Python)
pub fn setup_logging(log_file_path: Option<PathBuf>) -> anyhow::Result<()> {
    stop_logging();

    let path = log_file_path.unwrap_or_else(|| PathBuf::from("backend.log"));

    // Получение уровня логирования из NMS_LOG_LEVEL или RUST_LOG (по умолчанию info)
    let filter_str = std::env::var("NMS_LOG_LEVEL")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info,nms_core=info,nms=info".to_string());
    let env_filter = EnvFilter::try_new(&filter_str)
        .unwrap_or_else(|_| EnvFilter::new("info,nms_core=info,nms=info"));

    // Консольный слой вывода в stdout
    let stdout_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(false);

    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;

    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    // Неблокирующий аппендер лог-файла (аналог QueueHandler / QueueListener из Python)
    let file_appender = tracing_appender::rolling::never(parent, filename.as_ref());
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer().with_ansi(false).with_writer(non_blocking);

    let res = tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .try_init();

    if let Err(e) = res {
        // ponytail: subscriber already initialized globally by another test or component
        eprintln!("Tracing subscriber initialization skipped: {e}");
    }

    let logging_guard = LoggingGuard {
        _guard: Some(guard),
    };
    if let Ok(mut g) = GLOBAL_GUARD.lock() {
        *g = Some(logging_guard);
    }

    Ok(())
}
