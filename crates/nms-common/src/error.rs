//! # Единая система ошибок платформы (AppError)
//!
//! Обеспечивает типизацию ошибок ядра, модулей и API с поддержкой
//! интернационализации (i18n), HTTP статус-кодов и сериализации в JSON.

use crate::i18n::{self, Locale};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Результат выполнения операций с типом ошибки `AppError`
pub type Result<T, E = AppError> = std::result::Result<T, E>;

/// Категории и типы системных ошибок
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
}

impl AppError {
    /// Получить HTTP статус-код для данной ошибки
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Unauthorized { .. } => 401,
            Self::Forbidden { .. } => 403,
            Self::NotFound { .. } => 404,
            Self::BadRequest { .. } => 400,
            Self::Validation { .. } => 422,
            Self::Conflict { .. } => 409,
            Self::RateLimited { .. } => 429,
            Self::PluginTimeout { .. } => 504,
            Self::Database { .. } | Self::Plugin { .. } | Self::Internal { .. } => 500,
        }
    }

    /// Получить i18n ключ для локализации ошибки
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::Unauthorized { .. } => "core.error.unauthorized",
            Self::Forbidden { .. } => "core.error.forbidden",
            Self::NotFound { .. } => "core.error.not_found",
            Self::BadRequest { .. } => "core.error.bad_request",
            Self::Validation { .. } => "core.error.validation",
            Self::Conflict { .. } => "core.error.conflict",
            Self::Database { .. } => "core.error.database",
            Self::Plugin { .. } => "core.error.plugin_failed",
            Self::PluginTimeout { .. } => "core.error.plugin_timeout",
            Self::RateLimited { .. } => "core.error.rate_limited",
            Self::Internal { .. } => "core.error.internal",
        }
    }

    /// Получить параметры ошибки для подстановки в шаблон перевода
    pub fn i18n_params(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::Unauthorized { details } => vec![("details", details.clone())],
            Self::Forbidden { permission } => vec![("permission", permission.clone())],
            Self::NotFound { resource } => vec![("resource", resource.clone())],
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
        }
    }

    /// Локализовать сообщение об ошибке для заданной локали
    pub fn localized_message(&self, locale: Locale) -> String {
        let key = self.i18n_key();
        let params = self.i18n_params();
        let str_params: Vec<(&str, &str)> =
            params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        i18n::tr(locale, key, &str_params)
    }

    /// Преобразовать ошибку в структурированный JSON ответ API для клиента
    pub fn to_api_response(&self, locale: Locale) -> ErrorResponse {
        ErrorResponse {
            success: false,
            error: self.localized_message(locale),
            code: self.status_code(),
            i18n_key: self.i18n_key().to_string(),
        }
    }
}

/// Стандартный формат тела ответа с ошибкой для REST API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    pub code: u16,
    pub i18n_key: String,
}
