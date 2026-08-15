// Unit-тесты для модуля конфигурации (config.rs)

use nms_core::{get_or_create_secret_key, AppConfig};
use std::env;
use std::fs;
use std::path::Path;

#[test]
fn test_default_app_config() {
    let config = AppConfig::default();
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 8080);
    assert!(!config.is_server_mode);
    assert!(!config.secret_key.is_empty());
    assert!(config
        .cors_origins
        .contains(&"http://localhost:5173".to_string()));
    assert!(!config.enable_hsts);
}

#[test]
fn test_config_from_env() {
    // Временно очищаем и задаем env
    env::set_var("NMS_HOST", "0.0.0.0");
    env::set_var("NMS_PORT", "9090");
    env::set_var("NMS_SERVER_MODE", "true");
    env::set_var("NMS_SECRET_KEY", "custom-env-secret-key-123");
    env::set_var("NMS_CORS_ORIGINS", "http://example.com, https://app.local");
    env::set_var("NMS_ENABLE_HSTS", "true");

    let config = AppConfig::from_env();
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 9090);
    assert!(config.is_server_mode);
    assert_eq!(config.secret_key, "custom-env-secret-key-123");
    assert_eq!(
        config.cors_origins,
        vec!["http://example.com", "https://app.local"]
    );
    assert!(config.enable_hsts);

    // Очистка
    env::remove_var("NMS_HOST");
    env::remove_var("NMS_PORT");
    env::remove_var("NMS_SERVER_MODE");
    env::remove_var("NMS_SECRET_KEY");
    env::remove_var("NMS_CORS_ORIGINS");
    env::remove_var("NMS_ENABLE_HSTS");
}

#[test]
fn test_secret_key_file_creation() {
    env::remove_var("NMS_SECRET_KEY");
    let secret = get_or_create_secret_key();
    assert!(!secret.is_empty());

    let secret_file = Path::new("data/.secret_key");
    if secret_file.exists() {
        let content = fs::read_to_string(secret_file).unwrap();
        assert!(!content.trim().is_empty());
    }
}
