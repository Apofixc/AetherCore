//! # Единая система ошибок платформы (AppError)
//!
//! Обеспечивает типизацию ошибок ядра, модулей и API со стандартизированной
//! структурой ответа (code, message, status_code, details), поддержкой
//! интернационализации (i18n) и созданием кастомных ошибок плагинами.

use crate::i18n::{self, Locale};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Результат выполнения операций с типом ошибки `AppError`
pub type Result<T, E = AppError> = std::result::Result<T, E>;

/// Категории и типы системных и пользовательских ошибок
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppError {
    /// Ошибка аутентификации (отсутствует или недействителен токен)
    #[error("Authentication required: {details}")]
    Unauthorized { details: String },

    /// Ошибка авторизации (недостаточно прав доступа)
    #[error("Forbidden: missing permission '{permission}'")]
    Forbidden { permission: String },

    /// Запрошенный ресурс не найден
    #[error("Resource not found: '{resource}'")]
    NotFound { resource: String },

    /// Модуль не найден
    #[error("Module '{module_id}' not found")]
    ModuleNotFound { module_id: String },

    /// Модуль отключен
    #[error("Module '{module_id}' is disabled")]
    ModuleDisabled { module_id: String },

    /// Некорректные параметры запроса или данных
    #[error("Bad request: {details}")]
    BadRequest { details: String },

    /// Ошибка валидации схемы или полей
    #[error("Validation error for '{field}': {details}")]
    Validation { field: String, details: String },

    /// Конфликт состояния (например, пользователь уже существует)
    #[error("Conflict: {details}")]
    Conflict { details: String },

    /// Ошибка базы данных
    #[error("Database error: {details}")]
    Database { details: String },

    /// Ошибка в работе WASM-модуля / плагина
    #[error("Plugin error in '{plugin_id}': {details}")]
    Plugin {
        plugin_id: String,
        details: String,
    },

    /// Превышение времени ожидания вызова плагина
    #[error("Plugin '{plugin_id}' timed out")]
    PluginTimeout { plugin_id: String },

    /// Ограничение частоты запросов
    #[error("Rate limit exceeded. Retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    /// Внутренняя непредвиденная ошибка сервера
    #[error("Internal server error: {details}")]
    Internal { details: String },

    /// Произвольная кастомная ошибка (создаваемая модулями или расширениями)
    #[error("{message}")]
    Custom {
        code: String,
        message: String,
        status_code: u16,
        i18n_key: Option<String>,
        details: serde_json::Value,
    },
}

impl AppError {
    /// Создать произвольную кастомную ошибку
    pub fn custom(
        code: impl Into<String>,
        message: impl Into<String>,
        status_code: u16,
    ) -> Self {
        Self::Custom {
            code: code.into(),
            message: message.into(),
            status_code,
            i18n_key: None,
            details: serde_json::json!({}),
        }
    }

    /// Создать кастомную ошибку с расширенными метаданными и i18n
    pub fn custom_with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        status_code: u16,
        i18n_key: Option<String>,
        details: serde_json::Value,
    ) -> Self {
        Self::Custom {
            code: code.into(),
            message: message.into(),
            status_code,
            i18n_key,
            details,
        }
    }

    /// Конструктор ошибки: Модуль не найден
    pub fn module_not_found(module_id: impl Into<String>) -> Self {
        Self::ModuleNotFound {
            module_id: module_id.into(),
        }
    }

    /// Конструктор ошибки: Модуль отключен
    pub fn module_disabled(module_id: impl Into<String>) -> Self {
        Self::ModuleDisabled {
            module_id: module_id.into(),
        }
    }

    /// Получить строковый машиночитаемый код ошибки (code)
    pub fn error_code(&self) -> &str {
        match self {
            Self::Unauthorized { .. } => "AUTH_REQUIRED",
            Self::Forbidden { .. } => "INSUFFICIENT_PERMISSIONS",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::ModuleNotFound { .. } => "MODULE_NOT_FOUND",
            Self::ModuleDisabled { .. } => "MODULE_DISABLED",
            Self::BadRequest { .. } => "BAD_REQUEST",
            Self::Validation { .. } => "VALIDATION_ERROR",
            Self::Conflict { .. } => "CONFLICT",
            Self::Database { .. } => "DATABASE_ERROR",
            Self::Plugin { .. } => "PLUGIN_ERROR",
            Self::PluginTimeout { .. } => "PLUGIN_TIMEOUT",
            Self::RateLimited { .. } => "RATE_LIMITED",
            Self::Internal { .. } => "INTERNAL_SERVER_ERROR",
            Self::Custom { code, .. } => code.as_str(),
        }
    }

    /// Получить HTTP статус-код для данной ошибки
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Unauthorized { .. } => 401,
            Self::Forbidden { .. } | Self::ModuleDisabled { .. } => 403,
            Self::NotFound { .. } | Self::ModuleNotFound { .. } => 404,
            Self::BadRequest { .. } => 400,
            Self::Validation { .. } => 422,
            Self::Conflict { .. } => 409,
            Self::RateLimited { .. } => 429,
            Self::PluginTimeout { .. } => 504,
            Self::Database { .. } | Self::Plugin { .. } | Self::Internal { .. } => 500,
            Self::Custom { status_code, .. } => *status_code,
        }
    }

    /// Получить i18n ключ для локализации ошибки
    pub fn i18n_key(&self) -> &str {
        match self {
            Self::Unauthorized { .. } => "core.error.unauthorized",
            Self::Forbidden { .. } => "core.error.forbidden",
            Self::NotFound { .. } => "core.error.not_found",
            Self::ModuleNotFound { .. } => "core.error.not_found",
            Self::ModuleDisabled { .. } => "core.error.forbidden",
            Self::BadRequest { .. } => "core.error.bad_request",
            Self::Validation { .. } => "core.error.validation",
            Self::Conflict { .. } => "core.error.conflict",
            Self::Database { .. } => "core.error.database",
            Self::Plugin { .. } => "core.error.plugin_failed",
            Self::PluginTimeout { .. } => "core.error.plugin_timeout",
            Self::RateLimited { .. } => "core.error.rate_limited",
            Self::Internal { .. } => "core.error.internal",
            Self::Custom { i18n_key, .. } => {
                i18n_key.as_deref().unwrap_or("core.error.custom")
            }
        }
    }

    /// Получить структурированные детали ошибки (details) в формате JSON
    pub fn details(&self) -> serde_json::Value {
        match self {
            Self::Unauthorized { details } => serde_json::json!({ "details": details }),
            Self::Forbidden { permission } => serde_json::json!({ "permission": permission }),
            Self::NotFound { resource } => serde_json::json!({ "resource": resource }),
            Self::ModuleNotFound { module_id } => serde_json::json!({ "module_id": module_id }),
            Self::ModuleDisabled { module_id } => serde_json::json!({ "module_id": module_id }),
            Self::BadRequest { details } => serde_json::json!({ "details": details }),
            Self::Validation { field, details } => {
                serde_json::json!({ "field": field, "details": details })
            }
            Self::Conflict { details } => serde_json::json!({ "details": details }),
            Self::Database { details } => serde_json::json!({ "details": details }),
            Self::Plugin { plugin_id, details } => {
                serde_json::json!({ "plugin_id": plugin_id, "details": details })
            }
            Self::PluginTimeout { plugin_id } => {
                serde_json::json!({ "plugin_id": plugin_id })
            }
            Self::RateLimited { retry_after } => {
                serde_json::json!({ "retry_after": retry_after })
            }
            Self::Internal { details } => serde_json::json!({ "details": details }),
            Self::Custom { details, .. } => details.clone(),
        }
    }

    /// Получить параметры ошибки для подстановки в шаблон перевода
    pub fn i18n_params(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Unauthorized { details } => vec![("details", details.clone())],
            Self::Forbidden { permission } => vec![("permission", permission.clone())],
            Self::NotFound { resource } => vec![("resource", resource.clone())],
            Self::ModuleNotFound { module_id } => {
                vec![("resource", format!("Module '{}'", module_id))]
            }
            Self::ModuleDisabled { module_id } => {
                vec![("permission", format!("Module '{}' is disabled", module_id))]
            }
            Self::BadRequest { details } => vec![("details", details.clone())],
            Self::Validation { field, details } => {
                vec![("field", field.clone()), ("details", details.clone())]
            }
            Self::Conflict { details } => vec![("details", details.clone())],
            Self::Database { details } => vec![("details", details.clone())],
            Self::Plugin { plugin_id, details } => {
                vec![("plugin_id", plugin_id.clone()), ("details", details.clone())]
            }
            Self::PluginTimeout { plugin_id } => vec![("plugin_id", plugin_id.clone())],
            Self::RateLimited { retry_after } => vec![("retry_after", retry_after.to_string())],
            Self::Internal { details } => vec![("details", details.clone())],
            Self::Custom { message, .. } => vec![("details", message.clone())],
        }
    }

    /// Локализовать сообщение об ошибке для заданной локали
    pub fn localized_message(&self, locale: Locale) -> String {
        if let Self::Custom { message, i18n_key, .. } = self {
            if let Some(key) = i18n_key {
                let params = self.i18n_params();
                let str_params: Vec<(&str, &str)> =
                    params.iter().map(|(k, v)| (*k, v.as_str())).collect();
                return i18n::tr(locale, key, &str_params);
            }
            return message.clone();
        }

        let key = self.i18n_key();
        let params = self.i18n_params();
        let str_params: Vec<(&str, &str)> =
            params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        i18n::tr(locale, key, &str_params)
    }

    /// Преобразовать ошибку в стандартизированный JSON ответ API
    pub fn to_api_response(&self, locale: Locale) -> ErrorResponse {
        ErrorResponse {
            success: false,
            error: ErrorDetail {
                code: self.error_code().to_string(),
                message: self.localized_message(locale),
                status_code: self.status_code(),
                i18n_key: self.i18n_key().to_string(),
                details: self.details(),
            },
        }
    }
}

/// Стандартизированная структура информации об ошибке
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorDetail {
    /// Машиночитаемый код ошибки (например, MODULE_NOT_FOUND, AUTH_REQUIRED)
    pub code: String,
    /// Человекочитаемое локализованное сообщение
    pub message: String,
    /// HTTP статус-код
    pub status_code: u16,
    /// Ключ локализации
    pub i18n_key: String,
    /// Дополнительные структурированные детали ошибки
    #[serde(default)]
    pub details: serde_json::Value,
}

/// Стандартный единый формат JSON ответа с ошибкой в REST API платформы
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorDetail,
}
