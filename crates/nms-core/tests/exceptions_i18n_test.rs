// Автоматические тесты для модулей exceptions и i18n
use axum::http::StatusCode;
use nms_core::{get_lang, ErrorResponse, I18nEngine, NmsError};
use serde_json::json;
use std::fs;

#[test]
fn test_exceptions_status_codes_and_json_format() {
    // Тест NmsError::ModuleNotFound
    let err = NmsError::ModuleNotFound {
        module_id: "ping-collector".to_string(),
    };
    assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
    assert_eq!(err.code(), "MODULE_NOT_FOUND");
    assert_eq!(err.message(), "Module 'ping-collector' not found");
    assert_eq!(err.details(), json!({ "module_id": "ping-collector" }));

    // Преобразование в ErrorResponse
    let resp: ErrorResponse = err.to_error_response();
    assert_eq!(resp.error.code, "MODULE_NOT_FOUND");
    assert_eq!(resp.error.message, "Module 'ping-collector' not found");
    assert_eq!(resp.error.details["module_id"], "ping-collector");

    // Тест NmsError::AuthRequired
    let auth_err = NmsError::AuthRequired {
        message: "Session expired".to_string(),
    };
    assert_eq!(auth_err.status_code(), StatusCode::UNAUTHORIZED);
    assert_eq!(auth_err.code(), "AUTH_REQUIRED");

    // Тест NmsError::PermissionDenied
    let perm_err = NmsError::PermissionDenied {
        message: "Forbidden".to_string(),
    };
    assert_eq!(perm_err.status_code(), StatusCode::FORBIDDEN);
    assert_eq!(perm_err.code(), "INSUFFICIENT_PERMISSIONS");

    // Тест NmsError::Validation
    let val_err = NmsError::Validation {
        message: "Invalid field".to_string(),
        details: json!({ "field": "username" }),
    };
    assert_eq!(val_err.status_code(), StatusCode::BAD_REQUEST);
    assert_eq!(val_err.code(), "VALIDATION_ERROR");

    // Тест NmsError::Custom
    let custom_err = NmsError::Custom {
        status_code: 422,
        code: "UNPROCESSABLE_ENTITY".to_string(),
        message: "Unprocessable payload".to_string(),
        details: json!({ "reason": "malformed JSON" }),
    };
    assert_eq!(custom_err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(custom_err.code(), "UNPROCESSABLE_ENTITY");
}

#[test]
fn test_i18n_get_lang() {
    assert_eq!(get_lang(Some("ru"), None), "ru");
    assert_eq!(get_lang(Some("EN"), None), "en");
    assert_eq!(get_lang(None, Some("ru-RU,ru;q=0.9,en;q=0.8")), "ru");
    assert_eq!(get_lang(None, Some("en-US,en;q=0.9")), "en");
    assert_eq!(get_lang(None, None), "en");
}

#[test]
fn test_i18n_tr_and_formatting() {
    let engine = I18nEngine::new();

    // Перевод встроенного ключа auth_required
    assert_eq!(
        engine.tr("ru", "auth_required", None, None),
        "Необходима авторизация"
    );
    assert_eq!(
        engine.tr("en", "auth_required", None, None),
        "Authentication required"
    );

    // Перевод с подстановкой параметров
    let params = [("deleted", "5")];
    assert_eq!(
        engine.tr("ru", "audit_logs_rotated", None, Some(&params)),
        "Удалено 5 устаревших записей аудита"
    );
    assert_eq!(
        engine.tr("en", "audit_logs_rotated", None, Some(&params)),
        "Deleted 5 old audit records"
    );

    // Фолбэк на неизвестный ключ
    assert_eq!(
        engine.tr("ru", "Неизвестная ошибка", Some("Unknown error"), None),
        "Неизвестная ошибка"
    );
    assert_eq!(
        engine.tr("en", "Неизвестная ошибка", Some("Unknown error"), None),
        "Unknown error"
    );
}

#[test]
fn test_i18n_load_module_locales() {
    let engine = I18nEngine::new();
    let temp_dir_path =
        std::env::temp_dir().join(format!("nms_i18n_test_{}", uuid::Uuid::new_v4()));
    let locales_dir = temp_dir_path.join("locales");
    fs::create_dir_all(&locales_dir).unwrap();

    let ru_json = json!({
        "custom_module_msg": "Сообщение плагина {name}"
    });
    let en_json = json!({
        "custom_module_msg": "Plugin message {name}"
    });

    fs::write(locales_dir.join("ru.json"), ru_json.to_string()).unwrap();
    fs::write(locales_dir.join("en.json"), en_json.to_string()).unwrap();

    let loaded = engine.load_module_locales(&temp_dir_path).unwrap();
    assert_eq!(loaded, 2);

    let params = [("name", "ping")];
    assert_eq!(
        engine.tr("ru", "custom_module_msg", None, Some(&params)),
        "Сообщение плагина ping"
    );
    assert_eq!(
        engine.tr("en", "custom_module_msg", None, Some(&params)),
        "Plugin message ping"
    );

    let _ = fs::remove_dir_all(&temp_dir_path);
}

#[test]
fn test_i18n_load_module_locales_yaml_toml() {
    let engine = I18nEngine::new();
    let temp_dir_path =
        std::env::temp_dir().join(format!("nms_i18n_yaml_test_{}", uuid::Uuid::new_v4()));
    let locales_dir = temp_dir_path.join("locales");
    fs::create_dir_all(&locales_dir).unwrap();

    let ru_yaml = "yaml_key: 'Сообщение YAML'\n";
    let en_toml = "toml_key = \"Message TOML\"\n";

    fs::write(locales_dir.join("ru.yaml"), ru_yaml).unwrap();
    fs::write(locales_dir.join("en.toml"), en_toml).unwrap();

    let loaded = engine.load_module_locales(&temp_dir_path).unwrap();
    assert_eq!(loaded, 2);

    assert_eq!(engine.tr("ru", "yaml_key", None, None), "Сообщение YAML");
    assert_eq!(engine.tr("en", "toml_key", None, None), "Message TOML");

    let _ = fs::remove_dir_all(&temp_dir_path);
}

#[test]
fn test_i18n_register_module_messages_alias() {
    let engine = I18nEngine::new();

    let mut custom_messages = std::collections::HashMap::new();
    let mut lang_map = std::collections::HashMap::new();
    lang_map.insert("ru".to_string(), "Тест".to_string());
    lang_map.insert("en".to_string(), "Test".to_string());
    custom_messages.insert("custom_module.test_key".to_string(), lang_map);

    engine.register_module_messages(custom_messages);

    assert_eq!(
        engine.tr("ru", "custom_module.test_key", None, None),
        "Тест"
    );
    assert_eq!(
        engine.tr("en", "custom_module.test_key", None, None),
        "Test"
    );
}

#[test]
fn test_i18n_builtin_messages_coverage() {
    let engine = I18nEngine::new();

    assert_eq!(
        engine.tr("ru", "user_already_exists", None, None),
        "Пользователь с таким именем или UID уже существует"
    );
    assert_eq!(
        engine.tr("en", "user_already_exists", None, None),
        "User with this username or UID already exists"
    );
}
