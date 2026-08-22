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
    /// Создать произвольную базовую ошибку с указанным кодом, сообщением и HTTP-статусом
    ///
    /// # Аргументы
    /// * `code` — Машиночитаемый код ошибки (например, `"CUSTOM_ERROR"`).
    /// * `message` — Человекочитаемое описание проблемы.
    /// * `status_code` — Соответствующий HTTP статус-код (например, 400, 500).
    ///
    /// # Примеры
    /// ```rust
    /// use aethercore_common::error::AppError;
    ///
    /// let err = AppError::new("SERVICE_UNAVAILABLE", "Сервис временно недоступен", 503);
    /// assert_eq!(err.status_code(), 503);
    /// ```
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

    /// Залогировать ошибку в трассировку (tracing) с указанием целевого компонента
    ///
    /// Ошибки со статусом 5xx логируются на уровне `error!`, ошибки 4xx — на уровне `warn!`.
    ///
    /// # Аргументы
    /// * `target` — Имя компонента/подсистемы (например, `"bus"`, `"db"`, `"auth"`).
    ///
    /// # Возвращаемое значение
    /// Возвращает ссылку на текущую ошибку (`&Self`) для удобного связывания вызовов.
    pub fn log(&self, target: &str) -> &Self {
        if self.status_code >= 500 {
            error!(component = target, code = %self.code, status = self.status_code, "{}", self.message);
        } else if self.status_code >= 400 {
            warn!(component = target, code = %self.code, status = self.status_code, "{}", self.message);
        }
        self
    }

    /// Залогировать ошибку и вернуть `self` по значению
    ///
    /// Удобно для использования в выражениях возврата: `return Err(err.logged("bus"))`.
    ///
    /// # Аргументы
    /// * `target` — Имя компонента/подсистемы.
    pub fn logged(self, target: &str) -> Self {
        self.log(target);
        self
    }

    /// Добавить структурированные метаданные (JSON) к объекту ошибки
    ///
    /// # Аргументы
    /// * `details` — JSON-значение с контекстом ошибки (параметры, идентификаторы).
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    /// Установить ключ словаря интернационализации (i18n) для локализации сообщения
    ///
    /// # Аргументы
    /// * `key` — Ключ перевода (например, `"core.error.not_found"`).
    pub fn with_i18n_key(mut self, key: impl Into<String>) -> Self {
        self.i18n_key = key.into();
        self
    }

    // --- Стандартные конструкторы ошибок ядра ---

    /// 401 Unauthorized — Ошибка аутентификации (требуется вход в систему или валидный токен)
    ///
    /// # Аргументы
    /// * `details` — Описание причины отказа в доступе (например, `"Invalid token"`).
    ///
    /// # Примеры
    /// ```rust
    /// use aethercore_common::error::AppError;
    ///
    /// let err = AppError::unauthorized("Token expired");
    /// assert_eq!(err.status_code(), 401);
    /// assert_eq!(err.code(), "AUTH_REQUIRED");
    /// ```
    pub fn unauthorized(details: impl Into<String>) -> Self {
        let d = details.into();
        let msg = if d.is_empty() {
            "Authentication required".to_string()
        } else {
            d.clone()
        };
        Self {
            code: "AUTH_REQUIRED".into(),
            message: msg.clone(),
            status_code: 401,
            i18n_key: "core.error.unauthorized".into(),
            details: serde_json::json!({ "details": msg }),
        }
    }

    /// 403 Forbidden — Недостаточно прав доступа (RBAC)
    ///
    /// # Аргументы
    /// * `permission` — Название отсутствующего права доступа (например, `"users.manage"`).
    ///
    /// # Примеры
    /// ```rust
    /// use aethercore_common::error::AppError;
    ///
    /// let err = AppError::forbidden("system.manage");
    /// assert_eq!(err.status_code(), 403);
    /// ```
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

    /// 404 Not Found — Запрашиваемый ресурс не найден
    ///
    /// # Аргументы
    /// * `resource` — Наименование или идентификатор отсутствующего ресурса.
    ///
    /// # Примеры
    /// ```rust
    /// use aethercore_common::error::AppError;
    ///
    /// let err = AppError::not_found("User with id '42'");
    /// assert_eq!(err.status_code(), 404);
    /// ```
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

    /// 404 Module Not Found — Запрашиваемый плагин/модуль не зарегистрирован в системе
    ///
    /// # Аргументы
    /// * `module_id` — Идентификатор модуля (например, `"snmp-collector"`).
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

    /// 403 Module Disabled — Модуль отключен в конфигурации платформы
    ///
    /// # Аргументы
    /// * `module_id` — Идентификатор отключенного модуля.
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

    /// 400 Bad Request — Некорректный запрос клиента
    ///
    /// # Аргументы
    /// * `details` — Описание некорректного параметра или условия запроса.
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

    /// 422 Unprocessable Entity — Ошибка валидации переданных данных
    ///
    /// # Аргументы
    /// * `field` — Имя некорректного поля или параметра (например, `"username"`, `"port"`).
    /// * `details` — Пояснение ошибки валидации.
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

    /// 409 Conflict — Конфликт состояния ресурсов (например, дублирование уникального имени)
    ///
    /// # Аргументы
    /// * `details` — Описание возникшего конфликта.
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

    /// 500 Database Error — Внутренняя ошибка выполнения запроса к базе данных SQLite
    ///
    /// # Аргументы
    /// * `details` — Сообщение об ошибке базы данных.
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

    /// 500 Plugin Error — Ошибка выполнения гостевого кода плагина
    ///
    /// # Аргументы
    /// * `plugin_id` — Идентификатор упавшего плагина.
    /// * `details` — Описание ошибки или текст паники.
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

    /// 504 Plugin Timeout — Превышен лимит времени выполнения операции в гостевом Wasm-модуле
    ///
    /// # Аргументы
    /// * `plugin_id` — Идентификатор плагина, превысившего таймаут.
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

    /// 429 Rate Limited — Превышен лимит частоты запросов
    ///
    /// # Аргументы
    /// * `retry_after` — Время в секундах, через которое разрешено повторить запрос.
    pub fn rate_limited(retry_after: u64) -> Self {
        Self {
            code: "RATE_LIMITED".into(),
            message: format!("Rate limit exceeded. Retry after {}s", retry_after),
            status_code: 429,
            i18n_key: "core.error.rate_limited".into(),
            details: serde_json::json!({ "retry_after": retry_after }),
        }
    }

    /// 500 Internal Server Error — Непредвиденная внутренняя ошибка платформы
    ///
    /// # Аргументы
    /// * `details` — Описание внутренней ошибки.
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

    /// Создать произвольную кастомную ошибку
    ///
    /// # Аргументы
    /// * `code` — Код ошибки.
    /// * `message` — Сообщение об ошибке.
    /// * `status_code` — HTTP статус-код.
    pub fn custom(
        code: impl Into<String>,
        message: impl Into<String>,
        status_code: u16,
    ) -> Self {
        Self::new(code, message, status_code)
    }

    /// Создать произвольную кастомную ошибку с метаданными и ключом локализации
    ///
    /// # Аргументы
    /// * `code` — Код ошибки.
    /// * `message` — Сообщение об ошибке.
    /// * `status_code` — HTTP статус-код.
    /// * `i18n_key` — Опциональный ключ локализации.
    /// * `details` — JSON-объект с деталями ошибки.
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

    /// Локализовать сообщение об ошибке для заданной языковой локали
    ///
    /// Если задан `i18n_key`, выполняет поиск в словаре переводов с подстановкой параметров из `details`.
    /// При отсутствии ключа возвращает исходное сообщение `message`.
    ///
    /// # Аргументы
    /// * `locale` — Целевая языковая локаль ([`Locale`]).
    ///
    /// # Возвращаемое значение
    /// Локализованная строка сообщения.
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

    /// Преобразовать ошибку в стандартизированный JSON ответ REST API ([`ErrorResponse`])
    ///
    /// # Аргументы
    /// * `locale` — Языковая локаль клиента для перевода текста ошибки.
    ///
    /// # Возвращаемое значение
    /// Структура [`ErrorResponse`], готовая для сериализации в HTTP-ответ.
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

/// Стандартизированная структура информации об ошибке для API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorDetail {
    /// Машиночитаемый код ошибки (например, `"VALIDATION_ERROR"`)
    pub code: String,
    /// Человекопонятный локализованный текст ошибки
    pub message: String,
    /// HTTP статус-код ошибки (например, 400, 404, 500)
    pub status_code: u16,
    /// Ключ словаря i18n (например, `"core.error.validation"`)
    pub i18n_key: String,
    /// Структурированные детали и контекст ошибки
    #[serde(default)]
    pub details: serde_json::Value,
}

/// Стандартный единый формат JSON ответа с ошибкой в REST API платформы
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    /// Флаг успешности выполнения запроса (всегда `false` для ответов с ошибками)
    pub success: bool,
    /// Вложенный объект с подробностями ошибки ([`ErrorDetail`])
    pub error: ErrorDetail,
}
