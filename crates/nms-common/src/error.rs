//! # Единая система ошибок платформы (AppError)
//!
//! Архитектурный аналог NMSError: единая структура ошибки с кодом (code),
//! сообщением (message), HTTP-статусом (status_code), ключом локализации (i18n_key)
//! и произвольными метаданными (details).

use crate::i18n::{self, Locale};
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{error, warn};

/// Результат выполнения операций с типом ошибки `AppError`
pub type Result<T, E = AppError> = std::result::Result<T, E>;

/// Базовая структура ошибки платформы
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppError {
    /// Машиночитаемый код ошибки (например, AUTH_REQUIRED, MODULE_NOT_FOUND)
    pub code: String,
    /// Человекочитаемое сообщение об ошибке
    pub message: String,
    /// HTTP статус-код
    pub status_code: u16,
    /// Ключ локализации для перевода на фронтенде или бэкенде
    pub i18n_key: String,
    /// Дополнительные структурированные параметры ошибки
    pub details: serde_json::Value,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl AppError {
    /// Создать произвольную базовую ошибку
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        status_code: u16,
    ) -> Self {
        let code_str = code.into();
        let msg_str = message.into();
        Self {
            code: code_str,
            message: msg_str,
            status_code,
            i18n_key: String::new(),
            details: serde_json::json!({}),
        }
    }

    /// Залогировать ошибку в трассировку с указанием целевого модуля/компонента
    pub fn log(&self, target: &str) -> &Self {
        if self.status_code >= 500 {
            error!(component = target, code = %self.code, status = self.status_code, "{}", self.message);
        } else if self.status_code >= 400 {
            warn!(component = target, code = %self.code, status = self.status_code, "{}", self.message);
        }
        self
    }

    /// Залогировать ошибку и вернуть self (для удобного чейнинга `return Err(err.logged("bus"))`)
    pub fn logged(self, target: &str) -> Self {
        self.log(target);
        self
    }

    /// Добавить структурированные детали к ошибке
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    /// Задать ключ локализации
    pub fn with_i18n_key(mut self, key: impl Into<String>) -> Self {
        self.i18n_key = key.into();
        self
    }

    // --- Стандартные конструкторы ошибок ядра ---

    /// 401 Unauthorized
    pub fn unauthorized(details: impl Into<String>) -> Self {
        let d = details.into();
        Self {
            code: "AUTH_REQUIRED".into(),
            message: format!("Authentication required: {}", d),
            status_code: 401,
            i18n_key: "core.error.unauthorized".into(),
            details: serde_json::json!({ "details": d }),
        }
    }

    /// 403 Forbidden
    pub fn forbidden(permission: impl Into<String>) -> Self {
        let p = permission.into();
        Self {
            code: "INSUFFICIENT_PERMISSIONS".into(),
            message: format!("Forbidden: missing permission '{}'", p),
            status_code: 403,
            i18n_key: "core.error.forbidden".into(),
            details: serde_json::json!({ "permission": p }),
        }
    }

    /// 404 Not Found
    pub fn not_found(resource: impl Into<String>) -> Self {
        let r = resource.into();
        Self {
            code: "NOT_FOUND".into(),
            message: format!("Resource not found: '{}'", r),
            status_code: 404,
            i18n_key: "core.error.not_found".into(),
            details: serde_json::json!({ "resource": r }),
        }
    }

    /// 404 Module Not Found
    pub fn module_not_found(module_id: impl Into<String>) -> Self {
        let id = module_id.into();
        Self {
            code: "MODULE_NOT_FOUND".into(),
            message: format!("Module '{}' not found", id),
            status_code: 404,
            i18n_key: "core.error.not_found".into(),
            details: serde_json::json!({ "module_id": id, "resource": format!("Module '{}'", id) }),
        }
    }

    /// 403 Module Disabled
    pub fn module_disabled(module_id: impl Into<String>) -> Self {
        let id = module_id.into();
        Self {
            code: "MODULE_DISABLED".into(),
            message: format!("Module '{}' is disabled", id),
            status_code: 403,
            i18n_key: "core.error.forbidden".into(),
            details: serde_json::json!({ "module_id": id, "permission": format!("Module '{}' is disabled", id) }),
        }
    }

    /// 400 Bad Request
    pub fn bad_request(details: impl Into<String>) -> Self {
        let d = details.into();
        Self {
            code: "BAD_REQUEST".into(),
            message: format!("Bad request: {}", d),
            status_code: 400,
            i18n_key: "core.error.bad_request".into(),
            details: serde_json::json!({ "details": d }),
        }
    }

    /// 422 Validation Error
    pub fn validation(field: impl Into<String>, details: impl Into<String>) -> Self {
        let f = field.into();
        let d = details.into();
        Self {
            code: "VALIDATION_ERROR".into(),
            message: format!("Validation error for '{}': {}", f, d),
            status_code: 422,
            i18n_key: "core.error.validation".into(),
            details: serde_json::json!({ "field": f, "details": d }),
        }
    }

    /// 409 Conflict
    pub fn conflict(details: impl Into<String>) -> Self {
        let d = details.into();
        Self {
            code: "CONFLICT".into(),
            message: format!("Conflict: {}", d),
            status_code: 409,
            i18n_key: "core.error.conflict".into(),
            details: serde_json::json!({ "details": d }),
        }
    }

    /// 500 Database Error
    pub fn database(details: impl Into<String>) -> Self {
        let d = details.into();
        Self {
            code: "DATABASE_ERROR".into(),
            message: format!("Database error: {}", d),
            status_code: 500,
            i18n_key: "core.error.database".into(),
            details: serde_json::json!({ "details": d }),
        }
    }

    /// 500 Plugin Error
    pub fn plugin(plugin_id: impl Into<String>, details: impl Into<String>) -> Self {
        let id = plugin_id.into();
        let d = details.into();
        Self {
            code: "PLUGIN_ERROR".into(),
            message: format!("Plugin error in '{}': {}", id, d),
            status_code: 500,
            i18n_key: "core.error.plugin_failed".into(),
            details: serde_json::json!({ "plugin_id": id, "details": d }),
        }
    }

    /// 504 Plugin Timeout
    pub fn plugin_timeout(plugin_id: impl Into<String>) -> Self {
        let id = plugin_id.into();
        Self {
            code: "PLUGIN_TIMEOUT".into(),
            message: format!("Plugin '{}' timed out", id),
            status_code: 504,
            i18n_key: "core.error.plugin_timeout".into(),
            details: serde_json::json!({ "plugin_id": id }),
        }
    }

    /// 429 Rate Limited
    pub fn rate_limited(retry_after: u64) -> Self {
        Self {
            code: "RATE_LIMITED".into(),
            message: format!("Rate limit exceeded. Retry after {}s", retry_after),
            status_code: 429,
            i18n_key: "core.error.rate_limited".into(),
            details: serde_json::json!({ "retry_after": retry_after }),
        }
    }

    /// 500 Internal Server Error
    pub fn internal(details: impl Into<String>) -> Self {
        let d = details.into();
        Self {
            code: "INTERNAL_SERVER_ERROR".into(),
            message: format!("Internal server error: {}", d),
            status_code: 500,
            i18n_key: "core.error.internal".into(),
            details: serde_json::json!({ "details": d }),
        }
    }

    /// Произвольная кастомная ошибка
    pub fn custom(
        code: impl Into<String>,
        message: impl Into<String>,
        status_code: u16,
    ) -> Self {
        Self::new(code, message, status_code)
    }

    /// Произвольная кастомная ошибка с метаданными и i18n
    pub fn custom_with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        status_code: u16,
        i18n_key: Option<String>,
        details: serde_json::Value,
    ) -> Self {
        let mut err = Self::new(code, message, status_code).with_details(details);
        if let Some(key) = i18n_key {
            err = err.with_i18n_key(key);
        }
        err
    }

    // --- Методы доступа к свойствам ---

    /// Получить машиночитаемый код ошибки (например, `"AUTH_REQUIRED"`)
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Получить ассоциированный HTTP статус-код ошибки
    pub fn status_code(&self) -> u16 {
        self.status_code
    }

    /// Получить ключ интернационализации ошибки
    pub fn i18n_key(&self) -> &str {
        &self.i18n_key
    }

    /// Локализовать сообщение об ошибке для заданной локали
    pub fn localized_message(&self, locale: Locale) -> String {
        if self.i18n_key.is_empty() {
            return self.message.clone();
        }

        let mut params = Vec::new();
        if let Some(obj) = self.details.as_object() {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    params.push((k.as_str(), s.to_string()));
                } else {
                    params.push((k.as_str(), v.to_string()));
                }
            }
        }

        let str_params: Vec<(&str, &str)> =
            params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let translated = i18n::tr(locale, &self.i18n_key, &str_params);

        if translated == self.i18n_key {
            self.message.clone()
        } else {
            translated
        }
    }

    /// Преобразовать ошибку в стандартизированный JSON ответ API
    pub fn to_api_response(&self, locale: Locale) -> ErrorResponse {
        ErrorResponse {
            success: false,
            error: ErrorDetail {
                code: self.code.clone(),
                message: self.localized_message(locale),
                status_code: self.status_code,
                i18n_key: self.i18n_key.clone(),
                details: self.details.clone(),
            },
        }
    }
}

/// Стандартизированная структура информации об ошибке
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorDetail {
    /// Машиночитаемый код ошибки
    pub code: String,
    /// Человекопонятный локализованный текст ошибки
    pub message: String,
    /// HTTP статус-код
    pub status_code: u16,
    /// Ключ словаря i18n
    pub i18n_key: String,
    /// Структурированные детали и контекст ошибки
    #[serde(default)]
    pub details: serde_json::Value,
}

/// Стандартный единый формат JSON ответа с ошибкой в REST API платформы
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    /// Флаг успешности выполнения запроса (всегда `false` для ошибок)
    pub success: bool,
    /// Вложенный объект с подробностями ошибки ([`ErrorDetail`])
    pub error: ErrorDetail,
}
