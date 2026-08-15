// Глобальные ошибки и представление исключений для Axum в NMS
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

/// Детализированный объект ошибки системы NMS
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    pub details: Value,
}

/// Единый формат JSON-ответа при возникновении ошибки
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

/// Базовое перечисление ошибок ядра NMS
#[derive(Debug, Clone)]
pub enum NmsError {
    /// Внутренняя ошибка сервера (500)
    Internal { message: String, details: Value },
    /// Запрошенный модуль не найден (404)
    ModuleNotFound { module_id: String },
    /// Модуль отключен в системе (403)
    ModuleDisabled { module_id: String },
    /// Ошибка аутентификации (401)
    AuthRequired { message: String },
    /// Ошибка прав доступа (403)
    PermissionDenied { message: String },
    /// Ресурс не найден (404)
    NotFound { message: String },
    /// Ошибка валидации параметров (400)
    Validation { message: String, details: Value },
    /// Ошибка валидации структуры или манифеста модуля (400)
    ModuleValidationError { message: String, details: Value },
    /// Кастомное исключение с произвольным кодом и статусом
    Custom {
        status_code: u16,
        code: String,
        message: String,
        details: Value,
    },
}

impl NmsError {
    /// Возвращает HTTP-код ответа для данной ошибки
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ModuleNotFound { .. } | Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::ModuleDisabled { .. } | Self::PermissionDenied { .. } => StatusCode::FORBIDDEN,
            Self::AuthRequired { .. } => StatusCode::UNAUTHORIZED,
            Self::Validation { .. } | Self::ModuleValidationError { .. } => StatusCode::BAD_REQUEST,
            Self::Custom { status_code, .. } => {
                StatusCode::from_u16(*status_code).unwrap_or(StatusCode::BAD_REQUEST)
            }
        }
    }

    /// Возвращает символьный код ошибки
    pub fn code(&self) -> &str {
        match self {
            Self::Internal { .. } => "INTERNAL_ERROR",
            Self::ModuleNotFound { .. } => "MODULE_NOT_FOUND",
            Self::ModuleDisabled { .. } => "MODULE_DISABLED",
            Self::AuthRequired { .. } => "AUTH_REQUIRED",
            Self::PermissionDenied { .. } => "INSUFFICIENT_PERMISSIONS",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::Validation { .. } => "VALIDATION_ERROR",
            Self::ModuleValidationError { .. } => "MODULE_VALIDATION_ERROR",
            Self::Custom { code, .. } => code.as_str(),
        }
    }

    /// Возвращает текстовое сообщение ошибки
    pub fn message(&self) -> String {
        match self {
            Self::Internal { message, .. } => message.clone(),
            Self::ModuleNotFound { module_id } => format!("Module '{module_id}' not found"),
            Self::ModuleDisabled { module_id } => format!("Module '{module_id}' is disabled"),
            Self::AuthRequired { message } => message.clone(),
            Self::PermissionDenied { message } => message.clone(),
            Self::NotFound { message } => message.clone(),
            Self::Validation { message, .. } => message.clone(),
            Self::ModuleValidationError { message, .. } => message.clone(),
            Self::Custom { message, .. } => message.clone(),
        }
    }

    /// Возвращает доп. детали ошибки в формате JSON Value
    pub fn details(&self) -> Value {
        match self {
            Self::Internal { details, .. } => details.clone(),
            Self::ModuleNotFound { module_id } => json!({ "module_id": module_id }),
            Self::ModuleDisabled { module_id } => json!({ "module_id": module_id }),
            Self::AuthRequired { .. } | Self::PermissionDenied { .. } | Self::NotFound { .. } => {
                json!({})
            }
            Self::Validation { details, .. } | Self::ModuleValidationError { details, .. } => {
                details.clone()
            }
            Self::Custom { details, .. } => details.clone(),
        }
    }

    /// Преобразует исключение в DTO структуру ErrorResponse
    pub fn to_error_response(&self) -> ErrorResponse {
        ErrorResponse {
            error: ErrorDetail {
                code: self.code().to_string(),
                message: self.message(),
                details: self.details(),
            },
        }
    }
}

impl fmt::Display for NmsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]: {}", self.code(), self.message())
    }
}

impl std::error::Error for NmsError {}

/// Преобразование исключения NmsError в HTTP-ответ Axum
impl IntoResponse for NmsError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let payload = self.to_error_response();
        (status, Json(payload)).into_response()
    }
}
