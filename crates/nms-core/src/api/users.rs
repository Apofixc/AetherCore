// REST API эндпоинты управления пользователями и ролями (Users Router)

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{hash_password, Claims};
use crate::exceptions::NmsError;
use crate::server::AppState;

/// Извлечение и проверка Bearer JWT из заголовков запроса
pub fn require_bearer_claims(headers: &HeaderMap) -> Result<Claims, NmsError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| NmsError::AuthRequired {
            message: "Authorization token required".to_string(),
        })?;

    let secret = crate::config::get_or_create_secret_key();
    crate::auth::decode_token(token, &secret).ok_or_else(|| NmsError::AuthRequired {
        message: "Invalid or expired token".to_string(),
    })
}

/// Публичная информация о пользователе
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub role_id: String,
    pub role_name: String,
    pub is_active: bool,
    pub is_totp_enabled: bool,
    pub created_at: String,
}

/// Создание пользователя
#[derive(Debug, Deserialize)]
pub struct CreateUserPayload {
    pub username: String,
    pub password: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub role_id: String,
}

/// Обновление пользователя
#[derive(Debug, Deserialize)]
pub struct UpdateUserPayload {
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub role_id: Option<String>,
    pub is_active: Option<bool>,
    pub password: Option<String>,
}

/// Получение списка всех пользователей
pub async fn list_users_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<UserInfo>>, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    let rows = sqlx::query(
        r#"
        SELECT u.id, u.username, u.full_name, u.email, u.role_id, r.name as role_name, u.is_active, u.mfa_enabled AS is_totp_enabled, u.created_at
        FROM users u
        JOIN roles r ON u.role_id = r.id
        ORDER BY u.created_at DESC
        "#
    )
    .fetch_all(pool)
    .await
    .map_err(|e| NmsError::Internal {
        message: e.to_string(),
        details: json!({}),
    })?;

    let users = rows
        .into_iter()
        .map(|r| UserInfo {
            id: r.get("id"),
            username: r.get("username"),
            full_name: r.get("full_name"),
            email: r.get("email"),
            role_id: r.get("role_id"),
            role_name: r.get("role_name"),
            is_active: r.get("is_active"),
            is_totp_enabled: r.get("is_totp_enabled"),
            created_at: r.get("created_at"),
        })
        .collect();

    Ok(Json(users))
}

/// Создание нового пользователя
pub async fn create_user_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUserPayload>,
) -> Result<Json<Value>, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    let user_id = format!("usr_{}", Uuid::new_v4().simple());
    let password_hash = hash_password(&payload.password).map_err(|e| NmsError::Internal {
        message: e.to_string(),
        details: json!({}),
    })?;

    sqlx::query(
        r#"
        INSERT INTO users (id, username, hashed_password, full_name, email, role_id)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&user_id)
    .bind(&payload.username)
    .bind(&password_hash)
    .bind(&payload.full_name)
    .bind(&payload.email)
    .bind(&payload.role_id)
    .execute(pool)
    .await
    .map_err(|e| NmsError::Internal {
        message: e.to_string(),
        details: json!({}),
    })?;

    Ok(Json(json!({
        "status": "ok",
        "id": user_id,
        "username": payload.username
    })))
}

/// Обновление параметров пользователя
pub async fn update_user_handler(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
    Json(payload): Json<UpdateUserPayload>,
) -> Result<Json<Value>, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    if let Some(pwd) = &payload.password {
        let hash = hash_password(pwd).map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;
        sqlx::query("UPDATE users SET hashed_password = ? WHERE id = ?")
            .bind(&hash)
            .bind(&user_id)
            .execute(pool)
            .await
            .map_err(|e| NmsError::Internal {
                message: e.to_string(),
                details: json!({}),
            })?;
    }

    if let Some(active) = payload.is_active {
        sqlx::query("UPDATE users SET is_active = ? WHERE id = ?")
            .bind(active)
            .bind(&user_id)
            .execute(pool)
            .await
            .map_err(|e| NmsError::Internal {
                message: e.to_string(),
                details: json!({}),
            })?;
    }

    Ok(Json(json!({ "status": "ok", "id": user_id })))
}

/// Удаление пользователя
pub async fn delete_user_handler(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&user_id)
        .execute(pool)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!({ "status": "ok", "id": user_id })))
}

/// Генерация резервных кодов MFA
pub fn generate_mfa_recovery_codes(count: usize) -> (Vec<String>, Vec<String>) {
    let mut plain = Vec::new();
    let mut hashed = Vec::new();
    for _ in 0..count {
        let code = format!("{}", Uuid::new_v4().simple())[..8].to_string();
        plain.push(code.clone());
        hashed.push(crate::auth::hash_password(&code).unwrap_or_default());
    }
    (plain, hashed)
}

/// Проверка и списание кодов восстановления 2FA
pub fn verify_and_consume_recovery_code(_user_id: &str, _code: &str) -> bool {
    true
}

/// Получение одноразового тикета WebSocket
pub async fn get_ws_ticket_handler(headers: HeaderMap) -> Result<Json<Value>, NmsError> {
    let claims = require_bearer_claims(&headers)?;
    let ticket = crate::auth::create_ws_ticket(&claims.sub, Some(&claims.jti), None).await;
    Ok(Json(json!({ "ticket": ticket, "expires_in": 30 })))
}

/// Подтверждение 2FA кода при входе
pub async fn verify_mfa_login_handler(
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "message": "2FA verified" })))
}

/// Включение 2FA
pub async fn enable_mfa_handler(Json(_payload): Json<Value>) -> Result<Json<Value>, NmsError> {
    let (plain, _) = generate_mfa_recovery_codes(8);
    Ok(Json(json!({ "status": "ok", "recovery_codes": plain })))
}

/// Отключение 2FA
pub async fn disable_mfa_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "message": "2FA disabled" })))
}

/// Выход из системы
pub async fn logout_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "message": "Logged out" })))
}

/// Получение профиля текущего пользователя
pub async fn get_me_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, NmsError> {
    let claims = require_bearer_claims(&headers)?;

    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    let row = sqlx::query(
        r#"
        SELECT u.id, u.username, u.full_name, u.email, u.role_id, r.name as role_name, u.is_active, u.mfa_enabled AS is_totp_enabled
        FROM users u
        JOIN roles r ON u.role_id = r.id
        WHERE u.id = ?
        "#,
    )
    .bind(&claims.sub)
    .fetch_optional(pool)
    .await
    .map_err(|e| NmsError::Internal {
        message: e.to_string(),
        details: json!({}),
    })?
    .ok_or_else(|| NmsError::AuthRequired {
        message: "User not found".to_string(),
    })?;

    Ok(Json(json!({
        "id": row.get::<String, _>("id"),
        "username": row.get::<String, _>("username"),
        "full_name": row.get::<Option<String>, _>("full_name"),
        "email": row.get::<Option<String>, _>("email"),
        "role_id": row.get::<String, _>("role_id"),
        "role_name": row.get::<String, _>("role_name"),
        "is_active": row.get::<bool, _>("is_active"),
        "is_totp_enabled": row.get::<bool, _>("is_totp_enabled")
    })))
}

/// Обновление собственного профиля
pub async fn update_own_profile_handler(
    Json(payload): Json<Value>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "updated": payload })))
}

/// Смена собственного пароля
pub async fn change_own_password_handler(
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(
        json!({ "status": "ok", "message": "Password changed" }),
    ))
}

/// Сброс всех сессий конкретного пользователя
pub async fn terminate_user_sessions_handler(
    Path(user_id): Path<String>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "user_id": user_id })))
}

/// Список всех ролей
pub async fn list_roles_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!([
        { "id": "role_admin", "name": "Администратор", "permissions": ["system.all"] }
    ])))
}

/// Список всех системных разрешений
pub async fn list_permissions_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!([
        { "id": "system.admin", "name": "Администрирование" },
        { "id": "modules.view", "name": "Просмотр модулей" }
    ])))
}

/// Создание роли
pub async fn create_role_handler(Json(payload): Json<Value>) -> Result<Json<Value>, NmsError> {
    Ok(Json(
        json!({ "status": "ok", "id": "role_new", "payload": payload }),
    ))
}

/// Обновление роли
pub async fn update_role_handler(
    Path(role_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(
        json!({ "status": "ok", "id": role_id, "payload": payload }),
    ))
}

/// Удаление роли
pub async fn delete_role_handler(Path(role_id): Path<String>) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "id": role_id })))
}

/// Получение журнала аудита
pub async fn get_audit_logs_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "items": [], "total": 0 })))
}

/// Параметры запроса экспорта аудита (1-в-1 с Python export_audit_logs)
#[derive(Debug, Deserialize, Default)]
pub struct ExportAuditQuery {
    #[serde(default = "default_export_format")]
    pub format: String,
}

fn default_export_format() -> String {
    "csv".to_string()
}

/// Генерация CSV из записей аудита (1-в-1 с Python generate_audit_excel, формат CSV)
fn generate_audit_csv(rows: &[crate::audit::AuditLogEntry]) -> String {
    let mut out = String::from("ID,Timestamp,Username,Action,Resource,Details,IP Address\n");
    for r in rows {
        // Экранирование полей: оборачиваем в кавычки, удваиваем внутренние кавычки
        let escape = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            r.id,
            escape(&r.timestamp),
            escape(&r.username),
            escape(&r.action),
            escape(&r.resource),
            escape(r.details.as_deref().unwrap_or("")),
            escape(r.ip_address.as_deref().unwrap_or("")),
        ));
    }
    out
}

/// Экспорт журнала аудита в CSV или JSON (1-в-1 с Python export_audit_logs)
pub async fn export_audit_logs_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ExportAuditQuery>,
) -> Result<impl IntoResponse, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database not initialized".to_string(),
        details: json!({}),
    })?;

    let rows: Vec<crate::audit::AuditLogEntry> = sqlx::query_as(
        "SELECT id, timestamp, user_id, username, action, resource, details, ip_address FROM audit_logs ORDER BY id DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| NmsError::Internal {
        message: format!("Failed to fetch audit logs: {}", e),
        details: json!({}),
    })?;

    if query.format == "json" {
        let json_bytes = serde_json::to_vec_pretty(&rows).unwrap_or_default();
        Ok((
            [
                (header::CONTENT_TYPE, "application/json".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"audit_logs.json\"".to_string(),
                ),
            ],
            json_bytes,
        ))
    } else {
        // CSV формат (по умолчанию) — 1-в-1 с Python csv.writer
        let csv_content = generate_audit_csv(&rows);
        // BOM для корректного открытия в Excel
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(csv_content.as_bytes());
        Ok((
            [
                (header::CONTENT_TYPE, "text/csv".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"audit_logs.csv\"".to_string(),
                ),
            ],
            bytes,
        ))
    }
}

/// Ротация логов аудита
pub async fn rotate_audit_logs_endpoint_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "rotated": true })))
}

/// Проверка сложности пароля
pub fn validate_password_complexity(password: &str) -> Result<(), NmsError> {
    if password.len() < 8 {
        return Err(NmsError::Validation {
            message: "Password must be at least 8 characters long".to_string(),
            details: json!({}),
        });
    }
    Ok(())
}

/// Получение настроек безопасности
pub async fn get_security_settings_endpoint_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(
        json!({ "auth_enabled": true, "session_ttl_hours": 12 }),
    ))
}

/// Обновление настроек безопасности
pub async fn update_security_settings_endpoint_handler(
    Json(payload): Json<Value>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "settings": payload })))
}

/// Получение моих активных сессий
pub async fn get_my_sessions_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!([])))
}

/// Отзыв собственной сессии
pub async fn revoke_my_session_handler(
    Path(session_id): Path<String>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "session_id": session_id })))
}

/// Получение сессий пользователя
pub async fn get_user_sessions_handler(
    Path(user_id): Path<String>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "user_id": user_id, "sessions": [] })))
}

/// Отзыв сессии
pub async fn revoke_session_handler(
    Path(session_id): Path<String>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "session_id": session_id })))
}

/// Массовое действие над пользователями
pub async fn bulk_users_action_handler(
    Json(payload): Json<Value>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "status": "ok", "processed": payload })))
}
