//! # Тесты единой системы ошибок AppError и стандартного формата ответа

use nms_common::error::AppError;
use nms_common::i18n::Locale;

#[test]
fn test_error_localization_ru() {
    let err = AppError::NotFound {
        resource: "users/admin".into(),
    };
    assert_eq!(err.status_code(), 404);
    assert_eq!(err.error_code(), "NOT_FOUND");
    assert_eq!(
        err.localized_message(Locale::Ru),
        "Запрошенный ресурс 'users/admin' не найден"
    );
}

#[test]
fn test_error_localization_en() {
    let err = AppError::Forbidden {
        permission: "users.manage".into(),
    };
    assert_eq!(err.status_code(), 403);
    assert_eq!(err.error_code(), "INSUFFICIENT_PERMISSIONS");
    assert_eq!(
        err.localized_message(Locale::En),
        "Forbidden: missing required permission: users.manage"
    );
}

#[test]
fn test_standard_api_response_structure() {
    let err = AppError::Validation {
        field: "email".into(),
        details: "invalid format".into(),
    };
    let res = err.to_api_response(Locale::Ru);
    assert_eq!(res.success, false);
    assert_eq!(res.error.code, "VALIDATION_ERROR");
    assert_eq!(res.error.status_code, 422);
    assert_eq!(res.error.i18n_key, "core.error.validation");
    assert_eq!(res.error.details["field"], "email");
}

#[test]
fn test_custom_errors_support() {
    // 1. Простая кастомная ошибка
    let custom_err = AppError::custom("EXTERNAL_SERVICE_UNAVAILABLE", "Upstream device timed out", 503);
    assert_eq!(custom_err.error_code(), "EXTERNAL_SERVICE_UNAVAILABLE");
    assert_eq!(custom_err.status_code(), 503);
    assert_eq!(custom_err.localized_message(Locale::Ru), "Upstream device timed out");

    // 2. Кастомная ошибка с деталями
    let custom_with_details = AppError::custom_with_details(
        "DEVICE_COMMUNICATION_ERROR",
        "SNMP connection failed",
        502,
        None,
        serde_json::json!({
            "ip": "192.168.1.100",
            "port": 161,
            "timeout_ms": 3000
        }),
    );
    let res = custom_with_details.to_api_response(Locale::Ru);
    assert_eq!(res.error.code, "DEVICE_COMMUNICATION_ERROR");
    assert_eq!(res.error.status_code, 502);
    assert_eq!(res.error.details["ip"], "192.168.1.100");

    // 3. Модульные ошибки
    let mod_not_found = AppError::module_not_found("tuya");
    assert_eq!(mod_not_found.error_code(), "MODULE_NOT_FOUND");
    assert_eq!(mod_not_found.status_code(), 404);
}
