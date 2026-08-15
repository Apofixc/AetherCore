// Unit-тесты для модуля лог-провайдеров и фильтрации системных логов

use nms_core::{
    clean_ansi_codes, matches_log_level, LocalFileLogProvider, LogProvider, LogProviderRegistry,
};
use std::sync::Arc;
use tokio::fs;

#[test]
fn test_clean_ansi_codes() {
    let colored = "\x1B[31mError message\x1B[0m";
    let cleaned = clean_ansi_codes(colored);
    assert_eq!(cleaned, "Error message");
}

#[test]
fn test_matches_log_level() {
    assert!(matches_log_level(
        "2026-08-12 | INFO     | server | Started",
        "INFO"
    ));
    assert!(matches_log_level(
        "2026-08-12 | ERROR    | server | Crash",
        "ERROR"
    ));
    assert!(!matches_log_level(
        "2026-08-12 | DEBUG    | server | Trace",
        "ERROR"
    ));
    assert!(matches_log_level(
        "2026-08-12 | WARN     | server | Warning text",
        "WARN"
    ));
    assert!(matches_log_level(
        "2026-08-12 | WARNING  | server | Warning text",
        "WARN"
    ));
    assert!(matches_log_level(
        "2026-08-12 | CRITICAL | server | Critical failure",
        "ERROR"
    ));
    assert!(matches_log_level(
        "2026-08-12 | FATAL    | server | Fatal failure",
        "ERROR"
    ));
}

#[tokio::test]
async fn test_local_file_log_provider() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("nms_test_backend.log");

    let sample_logs = "2026-08-12 | INFO     | main | Application started\n\
                       2026-08-12 | WARN     | bus  | Topic queue slow\n\
                       2026-08-12 | ERROR    | db   | Failed connection\n";

    fs::write(&test_file, sample_logs).await.unwrap();

    let provider = LocalFileLogProvider::new("test.log", "test.log", test_file.clone());
    assert!(provider.is_available().await);

    // Проверка фильтрации по ERROR
    let res = provider.get_logs(10, "ERROR", "").await.unwrap();
    assert_eq!(res.matched_lines, 1);
    assert!(res.content[0].contains("Failed connection"));

    // Проверка скачивания файла
    let dl = provider.download_log().await.unwrap();
    assert_eq!(dl.content, sample_logs.as_bytes());

    let _ = fs::remove_file(test_file).await;
}

#[tokio::test]
async fn test_log_provider_registry() {
    let registry = LogProviderRegistry::new();
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("nms_registry_test.log");
    let _ = fs::write(&test_file, "dummy content").await;

    let provider = Arc::new(LocalFileLogProvider::new(
        "backend.log",
        "backend.log",
        test_file.clone(),
    ));
    registry.register(provider.clone()).await;

    let list = registry.list_all().await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "backend.log");

    let retrieved = registry.get("backend.log").await;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id(), "backend.log");

    let _ = fs::remove_file(test_file).await;
}

#[tokio::test]
async fn test_non_utf8_log_handling() {
    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("nms_non_utf8.log");

    // Формируем байты с битыми UTF-8 последовательностями (например, \xFF\xFE)
    let invalid_utf8_bytes = b"2026-08-12 | INFO | main | Valid text\n2026-08-12 | ERROR | main | Invalid \xFF\xFE bytes\n";
    fs::write(&test_file, invalid_utf8_bytes).await.unwrap();

    let provider = LocalFileLogProvider::new("non_utf8.log", "non_utf8.log", test_file.clone());
    let res = provider.get_logs(10, "ALL", "").await;

    assert!(
        res.is_ok(),
        "Reading log with invalid UTF-8 bytes must not panic or error out"
    );
    let data = res.unwrap();
    assert_eq!(data.total_lines, 2);

    let _ = fs::remove_file(test_file).await;
}

#[tokio::test]
async fn test_clean_ansi_alias() {
    let colored = "\x1B[32mSuccess message\x1B[0m";
    assert_eq!(nms_core::clean_ansi(colored), "Success message");
}

#[tokio::test]
async fn test_shared_log_stream_manager() {
    use nms_core::SharedLogStreamManager;

    let manager = SharedLogStreamManager::new();
    let registry = LogProviderRegistry::new();

    let temp_dir = std::env::temp_dir();
    let test_file = temp_dir.join("nms_stream_test.log");
    let _ = fs::write(&test_file, "2026-08-12 | INFO | stream | Stream line 1\n").await;

    let provider = Arc::new(LocalFileLogProvider::new(
        "stream.log",
        "stream.log",
        test_file.clone(),
    ));
    registry.register(provider).await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    manager
        .subscribe(
            "sub_1".to_string(),
            "stream.log".to_string(),
            "ALL".to_string(),
            "".to_string(),
            registry,
            tx,
        )
        .await;

    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await;
    assert!(msg.is_ok() && msg.unwrap().is_some());

    manager.unsubscribe("sub_1", "stream.log", "ALL", "").await;
    manager.close_all().await;

    let _ = fs::remove_file(test_file).await;
}

#[tokio::test]
async fn test_load_remote_sources_from_db() {
    use nms_core::{init_db_pool, load_remote_sources_from_db, LogProviderRegistry};

    let temp_dir = std::env::temp_dir();
    let db_file = temp_dir.join("nms_test_remote_sources.db");
    let _ = fs::remove_file(&db_file).await;

    let pool = init_db_pool(&db_file).await.unwrap();

    // Создаем запись удаленного источника
    sqlx::query(
        "INSERT INTO remote_log_sources (id, name, url, api_token) VALUES ('remote1', 'Remote 1', 'http://127.0.0.1:8080/log', NULL)"
    )
    .execute(&pool)
    .await
    .unwrap();

    let registry = LogProviderRegistry::new();
    load_remote_sources_from_db(&pool, &registry, "test_secret_key")
        .await
        .unwrap();

    let list = registry.list_all().await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "remote1");
    assert_eq!(list[0].name, "Remote 1");

    let _ = fs::remove_file(&db_file).await;
}
