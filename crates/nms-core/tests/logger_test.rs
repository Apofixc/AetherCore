use nms_core::{setup_logging, stop_logging};
use std::fs;
use tracing::info;

#[test]
fn test_setup_logging_file_creation_and_reinit() {
    let log_dir = std::env::temp_dir().join("nms_test_logs");
    let log_path = log_dir.join("test_backend.log");

    // Инициализация логирования с файлом
    let res = setup_logging(Some(log_path.clone()));
    assert!(res.is_ok(), "setup_logging should succeed");

    info!("Test log message for logger verification");

    // Повторный вызов не должен падать с ошибкой (без паники при переинициализации)
    let reinit_res = setup_logging(None);
    assert!(
        reinit_res.is_ok(),
        "Re-initializing logging must handle existing global subscriber gracefully"
    );

    // Проверяем создание файла
    assert!(log_path.exists(), "Log file should be created");

    // Останов логгирования
    stop_logging();

    // Очистка временного файла
    let _ = fs::remove_dir_all(log_dir);
}
