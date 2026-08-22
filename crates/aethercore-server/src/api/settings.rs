//! # Эндпоинты настроек платформы и предпочтений (`/api/v1/settings`)
//!
//! Предоставляет HTTP эндпоинты для:
//! - Получения и сохранения персональных настроек профиля пользователя (`/api/v1/settings/user-preferences`).
//! - Получения и сохранения общесистемных политик безопасности (`/api/v1/settings/security`).
//! - Получения и сохранения матрицы прав доступа ролей RBAC (`/api/v1/settings/permissions`).
//! - Получения и сохранения настроек системного обслуживания и бэкапов (`/api/v1/settings/maintenance`).

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use aethercore_common::error::ErrorResponse;
use aethercore_common::AppError;
use aethercore_core::auth::check_permission;
use aethercore_core::db::kv::KvStore;
use serde::{Deserialize, Serialize};

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

/// Создать вложенный роутер настроек `/settings`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/user-preferences", get(get_user_preferences_handler).put(update_user_preferences_handler))
        .route("/security", get(get_security_policies_handler).put(update_security_policies_handler))
        .route("/permissions", get(get_permissions_matrix_handler).put(update_permissions_matrix_handler))
        .route("/maintenance", get(get_maintenance_settings_handler).put(update_maintenance_settings_handler))
}

// ---------------------------------------------------------------------------
// 1. Персональные предпочтения пользователя
// ---------------------------------------------------------------------------

/// DTO подписки пользователя на модуль
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSubscriptionDto {
    /// Идентификатор подписки
    pub id: String,
    /// Ключ локализации названия
    pub name_key: String,
    /// Символьный код модуля
    pub code: String,
    /// Ключ локализации описания
    pub desc_key: String,
    /// Флаг активности подписки
    pub enabled: bool,
    /// Режим временного мьюта
    pub mute: String,
    /// Звуковой сигнал оповещения
    pub sound: String,
    /// Порог фильтрации событий
    pub threshold: String,
}

/// DTO персональных настроек профиля пользователя
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferencesDto {
    /// Часовой пояс пользователя
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// Формат времени отображения
    #[serde(default = "default_time_format")]
    pub time_format: String,
    /// Цветовая тема интерфейса
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Язык интерфейса
    #[serde(default = "default_locale")]
    pub locale: String,
    /// Подразделение / отдел
    #[serde(default)]
    pub department: Option<String>,
    /// Длительность режима "Не беспокоить"
    #[serde(default = "default_mute_duration")]
    pub active_mute_duration: String,
    /// Флаг включения тихих часов
    #[serde(default)]
    pub quiet_hours_enabled: bool,
    /// Расписание тихих часов
    #[serde(default = "default_quiet_schedule")]
    pub quiet_schedule: String,
    /// Звуковой сигнал для информационных событий
    #[serde(default = "default_sound_info")]
    pub sound_info: String,
    /// Звуковой сигнал для успешных операций
    #[serde(default = "default_sound_success")]
    pub sound_success: String,
    /// Звуковой сигнал для предупреждений
    #[serde(default = "default_sound_warning")]
    pub sound_warning: String,
    /// Звуковой сигнал для ошибок и тревог
    #[serde(default = "default_sound_error")]
    pub sound_error: String,
    /// Список подписок на события модулей
    #[serde(default = "default_module_subscriptions")]
    pub module_subscriptions: Vec<ModuleSubscriptionDto>,
    /// Флаг свернутого состояния боковой панели
    #[serde(default)]
    pub sidebar_collapsed: bool,
    /// Аватарка пользователя (base64 data URL или ссылка на изображение)
    #[serde(default)]
    pub avatar: Option<String>,
}

fn default_timezone() -> String {
    "UTC".to_string()
}
fn default_time_format() -> String {
    "24h_sec".to_string()
}
fn default_theme() -> String {
    "dark".to_string()
}
fn default_locale() -> String {
    "ru".to_string()
}
fn default_mute_duration() -> String {
    "none".to_string()
}
fn default_quiet_schedule() -> String {
    "23:00 — 07:00 (GMT+3)".to_string()
}
fn default_sound_info() -> String {
    "Soft Chime".to_string()
}
fn default_sound_success() -> String {
    "Major Chord".to_string()
}
fn default_sound_warning() -> String {
    "Double Beep".to_string()
}
fn default_sound_error() -> String {
    "Alarm Tone".to_string()
}
fn default_module_subscriptions() -> Vec<ModuleSubscriptionDto> {
    Vec::new()
}

impl Default for UserPreferencesDto {
    fn default() -> Self {
        Self {
            timezone: default_timezone(),
            time_format: default_time_format(),
            theme: default_theme(),
            locale: default_locale(),
            department: Some("Network Operations".to_string()),
            active_mute_duration: default_mute_duration(),
            quiet_hours_enabled: false,
            quiet_schedule: default_quiet_schedule(),
            sound_info: default_sound_info(),
            sound_success: default_sound_success(),
            sound_warning: default_sound_warning(),
            sound_error: default_sound_error(),
            module_subscriptions: default_module_subscriptions(),
            sidebar_collapsed: false,
            avatar: None,
        }
    }
}

/// GET /api/v1/settings/user-preferences
async fn get_user_preferences_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> ApiResult<UserPreferencesDto> {
    let kv = KvStore::new(state.db.clone(), format!("user:{}", claims.sub));
    let prefs: Option<UserPreferencesDto> = kv.get("preferences").await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(prefs.unwrap_or_default()))
}

/// PUT /api/v1/settings/user-preferences
async fn update_user_preferences_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(updates): Json<serde_json::Value>,
) -> ApiResult<UserPreferencesDto> {
    let kv = KvStore::new(state.db.clone(), format!("user:{}", claims.sub));
    let current: UserPreferencesDto = kv
        .get("preferences")
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?
        .unwrap_or_default();

    let mut current_val = serde_json::to_value(&current).map_err(|e| {
        let err = AppError::internal(e.to_string());
        (StatusCode::INTERNAL_SERVER_ERROR, Json(err.to_api_response(locale)))
    })?;

    if let (Some(base), Some(patch)) = (current_val.as_object_mut(), updates.as_object()) {
        for (k, v) in patch {
            base.insert(k.clone(), v.clone());
        }
    }

    let merged: UserPreferencesDto = serde_json::from_value(current_val).map_err(|e| {
        let err = AppError::validation("preferences", &e.to_string());
        (StatusCode::BAD_REQUEST, Json(err.to_api_response(locale)))
    })?;

    kv.set("preferences", &merged).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(merged))
}

// ---------------------------------------------------------------------------
// 2. Политики безопасности и аутентификации
// ---------------------------------------------------------------------------

pub use aethercore_common::models::user::SecurityPoliciesDto;

/// GET /api/v1/settings/security
async fn get_security_policies_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> ApiResult<SecurityPoliciesDto> {
    if !claims.is_superuser {
        check_permission(&claims, "settings.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let kv = KvStore::system(state.db.clone());
    let policies: Option<SecurityPoliciesDto> = kv.get("security_policies").await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(policies.unwrap_or_default()))
}

/// PUT /api/v1/settings/security
async fn update_security_policies_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(dto): Json<SecurityPoliciesDto>,
) -> ApiResult<SecurityPoliciesDto> {
    if !claims.is_superuser {
        check_permission(&claims, "settings.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    if dto.min_password_length < 4 || dto.min_password_length > 64 {
        let err = aethercore_common::error::AppError::validation(
            "min_password_length",
            "Minimum password length must be between 4 and 64 characters",
        );
        return Err((StatusCode::BAD_REQUEST, Json(err.to_api_response(locale))));
    }

    if dto.max_login_attempts < 1 || dto.max_login_attempts > 100 {
        let err = aethercore_common::error::AppError::validation(
            "max_login_attempts",
            "Max login attempts must be between 1 and 100",
        );
        return Err((StatusCode::BAD_REQUEST, Json(err.to_api_response(locale))));
    }

    if dto.lockout_duration < 1 || dto.lockout_duration > 10080 {
        let err = aethercore_common::error::AppError::validation(
            "lockout_duration",
            "Lockout duration must be between 1 and 10080 minutes",
        );
        return Err((StatusCode::BAD_REQUEST, Json(err.to_api_response(locale))));
    }

    let kv = KvStore::system(state.db.clone());
    kv.set("security_policies", &dto).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "settings.security.update",
            "settings/security",
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(dto))
}

// ---------------------------------------------------------------------------
// 3. Матрица прав доступа ролей (RBAC Permissions Matrix)
// ---------------------------------------------------------------------------

/// GET /api/v1/settings/permissions
async fn get_permissions_matrix_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> ApiResult<serde_json::Value> {
    if !claims.is_superuser {
        check_permission(&claims, "access.roles.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let kv = KvStore::system(state.db.clone());
    let matrix: Option<serde_json::Value> = kv.get("permissions_matrix").await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    match matrix {
        Some(m) => Ok(Json(m)),
        None => Ok(Json(default_permissions_matrix())),
    }
}

/// PUT /api/v1/settings/permissions
async fn update_permissions_matrix_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(dto): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    if !claims.is_superuser {
        check_permission(&claims, "access.roles.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;

        // Пользователи ниже уровня admin (operator, viewer) не могут изменять матрицу прав
        let has_admin_or_super = claims.roles.iter().any(|r| r == "admin" || r == "superuser");
        if !has_admin_or_super {
            return Err((
                StatusCode::FORBIDDEN,
                Json(
                    AppError::forbidden("Only administrators and superusers can modify role permissions matrix")
                        .to_api_response(locale),
                ),
            ));
        }
    }

    let kv = KvStore::system(state.db.clone());
    kv.set("permissions_matrix", &dto).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "settings.permissions.update",
            "settings/permissions",
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(dto))
}

fn default_permissions_matrix() -> serde_json::Value {
    serde_json::json!([
        {
            "id": "system",
            "name": "System",
            "icon": "terminal",
            "items": [
                { "id": "system_view", "name": "View System", "code": "system.view", "description": "View system status, health, and logs", "admin": true, "operator": true, "viewer": true },
                { "id": "system_manage", "name": "Manage System", "code": "system.manage", "description": "Modify system parameters, maintenance and backups", "admin": true, "operator": false, "viewer": false }
            ]
        },
        {
            "id": "users",
            "name": "Users",
            "icon": "group",
            "items": [
                { "id": "users_view", "name": "View Users", "code": "users.view", "description": "View users directory and account details", "admin": true, "operator": true, "viewer": true },
                { "id": "users_manage", "name": "Manage Users", "code": "users.manage", "description": "Create, edit, block, and delete user accounts", "admin": true, "operator": false, "viewer": false }
            ]
        },
        {
            "id": "modules",
            "name": "Modules",
            "icon": "view_in_ar",
            "items": [
                { "id": "modules_view", "name": "View Modules", "code": "modules.view", "description": "View installed plugins and module runtime state", "admin": true, "operator": true, "viewer": true },
                { "id": "modules_manage", "name": "Manage Modules", "code": "modules.manage", "description": "Install, update, enable/disable dynamic WASM modules", "admin": true, "operator": false, "viewer": false }
            ]
        },
        {
            "id": "events",
            "name": "Events & Telemetry",
            "icon": "sensors",
            "items": [
                { "id": "events_view", "name": "View Events", "code": "events.view", "description": "View real-time and historical event journal", "admin": true, "operator": true, "viewer": true }
            ]
        },
        {
            "id": "audit_logs",
            "name": "Audit Logs",
            "icon": "history_edu",
            "items": [
                { "id": "audit_view", "name": "View Audit Logs", "code": "audit.view", "description": "View security audit log history", "admin": true, "operator": true, "viewer": true },
                { "id": "audit_export", "name": "Export Audit Logs", "code": "audit.export", "description": "Export security audit log history", "admin": true, "operator": false, "viewer": false }
            ]
        },
        {
            "id": "access_control",
            "name": "Access Control",
            "icon": "vpn_key",
            "items": [
                { "id": "access_roles_view", "name": "View Roles & Permissions", "code": "access.roles.view", "description": "View access roles and permissions matrix", "admin": true, "operator": true, "viewer": true },
                { "id": "access_roles_manage", "name": "Manage Roles & Permissions", "code": "access.roles.manage", "description": "Create, edit, delete access roles and assign permissions", "admin": true, "operator": false, "viewer": false }
            ]
        },
        {
            "id": "settings",
            "name": "Settings",
            "icon": "settings",
            "items": [
                { "id": "settings_view", "name": "View System Settings", "code": "settings.view", "description": "View global application settings and configuration", "admin": true, "operator": true, "viewer": true },
                { "id": "settings_manage", "name": "Manage System Settings", "code": "settings.manage", "description": "Modify global application settings and configuration", "admin": true, "operator": false, "viewer": false }
            ]
        }
    ])
}

// ---------------------------------------------------------------------------
// 4. Системное обслуживание и администрирование
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

/// DTO настроек системного обслуживания и резервного копирования
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSettingsDto {
    /// Флаг автобэкапа базы данных
    #[serde(default = "default_true")]
    pub auto_backup: bool,
    /// Интервал автобэкапа в часах
    #[serde(default = "default_backup_interval")]
    pub backup_interval_hours: u32,
    /// Срок хранения бэкапов в днях
    #[serde(default = "default_backup_retention")]
    pub backup_retention_days: u32,
    /// Срок хранения журнала аудита в днях
    #[serde(default = "default_audit_retention")]
    pub audit_retention_days: u32,
    /// Уровень системного логирования по умолчанию
    #[serde(default = "default_log_level")]
    pub default_log_level: String,
}

fn default_backup_interval() -> u32 {
    24
}
fn default_backup_retention() -> u32 {
    30
}
fn default_audit_retention() -> u32 {
    90
}
fn default_log_level() -> String {
    "INFO".to_string()
}

impl Default for MaintenanceSettingsDto {
    fn default() -> Self {
        Self {
            auto_backup: true,
            backup_interval_hours: default_backup_interval(),
            backup_retention_days: default_backup_retention(),
            audit_retention_days: default_audit_retention(),
            default_log_level: default_log_level(),
        }
    }
}

/// GET /api/v1/settings/maintenance
async fn get_maintenance_settings_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> ApiResult<MaintenanceSettingsDto> {
    if !claims.is_superuser {
        check_permission(&claims, "system.admin").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let kv = KvStore::system(state.db.clone());
    let maintenance: Option<MaintenanceSettingsDto> = kv.get("maintenance_settings").await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(maintenance.unwrap_or_default()))
}

/// PUT /api/v1/settings/maintenance
async fn update_maintenance_settings_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(dto): Json<MaintenanceSettingsDto>,
) -> ApiResult<MaintenanceSettingsDto> {
    if !claims.is_superuser {
        check_permission(&claims, "system.admin").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let kv = KvStore::system(state.db.clone());
    kv.set("maintenance_settings", &dto).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "settings.maintenance.update",
            "settings/maintenance",
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(dto))
}
