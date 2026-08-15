// REST API эндпоинты аутентификации пользователей и управления 2FA (Auth Router)

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::{
    create_access_token, create_refresh_token, decode_token, generate_qr_svg, generate_totp_secret,
    get_totp_uri, verify_password, verify_totp_code,
};
use crate::config::get_or_create_secret_key;
use crate::exceptions::NmsError;
use crate::server::AppState;

/// Модель входящих данных авторизации
#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
    pub totp_code: Option<String>,
}

/// Модель ответа с JWT токенами
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
}

/// Модель входящего токена обновления
#[derive(Debug, Deserialize)]
pub struct RefreshPayload {
    pub refresh_token: String,
}

/// Обработчик входа в систему по логину и паролю
pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<AuthResponse>, NmsError> {
    let limiter_key = format!("login:{}", payload.username);
    if state.rate_limiter.is_rate_limited(&limiter_key, 5, 60) {
        return Err(NmsError::PermissionDenied {
            message: "Too many login attempts. Please try again later.".to_string(),
        });
    }

    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    let row = sqlx::query(
        r#"
        SELECT u.id, u.username, u.hashed_password AS password_hash, u.is_active, u.mfa_secret AS totp_secret, u.mfa_enabled AS is_totp_enabled
        FROM users u
        WHERE u.username = ?
        "#,
    )
    .bind(&payload.username)
    .fetch_optional(pool)
    .await
    .map_err(|e| NmsError::Internal {
        message: e.to_string(),
        details: json!({}),
    })?;

    let user = row.ok_or_else(|| NmsError::AuthRequired {
        message: "Invalid username or password".to_string(),
    })?;

    use sqlx::Row;
    let user_id: String = user.get("id");
    let username: String = user.get("username");
    let password_hash: String = user.get("password_hash");
    let is_active: bool = user.get("is_active");
    let totp_secret: Option<String> = user.get("totp_secret");
    let is_totp_enabled: bool = user.get("is_totp_enabled");

    if !is_active {
        return Err(NmsError::PermissionDenied {
            message: "User account is disabled".to_string(),
        });
    }

    if !verify_password(&payload.password, &password_hash) {
        return Err(NmsError::AuthRequired {
            message: "Invalid username or password".to_string(),
        });
    }

    if is_totp_enabled {
        let secret = totp_secret.as_deref().unwrap_or_default();
        let code = payload.totp_code.as_deref().unwrap_or_default();
        if !verify_totp_code(secret, code) {
            return Err(NmsError::AuthRequired {
                message: "Invalid 2FA TOTP code".to_string(),
            });
        }
    }

    let secret_key = get_or_create_secret_key();
    let access_token = create_access_token(&user_id, &username, &secret_key, 24).map_err(|e| {
        NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        }
    })?;
    let jti = format!("jti-{}", Uuid::new_v4().simple());
    let refresh_token =
        create_refresh_token(&user_id, &username, &jti, &secret_key, 168).map_err(|e| {
            NmsError::Internal {
                message: e.to_string(),
                details: json!({}),
            }
        })?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        token_type: "bearer".to_string(),
    }))
}

/// Обработчик обновления JWT доступа по refresh_token
pub async fn refresh_handler(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<RefreshPayload>,
) -> Result<Json<AuthResponse>, NmsError> {
    let secret_key = get_or_create_secret_key();
    let claims = decode_token(&payload.refresh_token, &secret_key).ok_or_else(|| {
        NmsError::AuthRequired {
            message: "Invalid or expired refresh token".to_string(),
        }
    })?;

    let access_token = create_access_token(&claims.sub, &claims.username, &secret_key, 24)
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;
    let new_jti = format!("jti-{}", Uuid::new_v4().simple());
    let new_refresh_token =
        create_refresh_token(&claims.sub, &claims.username, &new_jti, &secret_key, 168).map_err(
            |e| NmsError::Internal {
                message: e.to_string(),
                details: json!({}),
            },
        )?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token: new_refresh_token,
        token_type: "bearer".to_string(),
    }))
}

/// Генерация 2FA TOTP ключа и QR кода для пользователя
pub async fn setup_mfa_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    let secret = generate_totp_secret();
    let uri = get_totp_uri(&secret, "NMS Administrator", "NMS Core");
    let qr_svg = generate_qr_svg(&uri);

    sqlx::query("UPDATE users SET totp_secret = ? WHERE id = 'root'")
        .bind(&secret)
        .execute(pool)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!({
        "secret": secret,
        "qr_svg": qr_svg,
        "uri": uri
    })))
}
