//! # Тесты единой системы ошибок AppError

use nms_common::error::AppError;
use nms_common::i18n::Locale;

#[test]
fn test_error_localization_ru() {
    let err = AppError::NotFound {
        resource: "users/admin".into(),
    };
    assert_eq!(err.status_code(), 404);
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
    assert_eq!(
        err.localized_message(Locale::En),
        "Forbidden: missing required permission: users.manage"
    );
}

#[test]
fn test_api_response_generation() {
    let err = AppError::Validation {
        field: "email".into(),
        details: "invalid format".into(),
    };
    let res = err.to_api_response(Locale::Ru);
    assert_eq!(res.code, 422);
    assert_eq!(res.success, false);
    assert_eq!(res.i18n_key, "core.error.validation");
}
