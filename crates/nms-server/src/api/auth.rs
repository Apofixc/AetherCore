//! # Маршруты и обработчики аутентификации API (`/api/v1/auth`)

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use nms_common::models::user::UserResponseDto;
use serde::{Deserialize, Serialize};

/// Создать вложенный роутер аутентификации `/auth` (`/login`, `/me`)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login_handler))
        .route("/me", get(me_handler))
}

/// Запрос на аутентификацию по логину и паролю
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Логин пользователя
    pub username: String,
    /// Пароль пользователя
    pub password: String,
}

/// Ответ успешной аутентификации с выпуском токена
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    /// Флаг успеха
    pub success: bool,
    /// Выпущенный JWT токен
    pub token: String,
    /// Данные профиля авторизованного пользователя
    pub user: UserResponseDto,
}

/// POST /api/v1/auth/login
async fn login_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<nms_common::error::ErrorResponse>)> {
    let user = state
        .user_service
        .authenticate(&req.username, &req.password)
        .await
        .map_err(|e| {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::UNAUTHORIZED);
            (status, Json(e.to_api_response(locale)))
        })?;

    let token = state
        .jwt_manager
        .generate_token(
            user.id,
            &user.username,
            user.is_superuser,
            user.permissions.clone(),
        )
        .map_err(|e| {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(e.to_api_response(locale)))
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&user.id.to_string()),
            Some(&user.username),
            "auth.login",
            "auth",
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(LoginResponse {
        success: true,
        token,
        user: user.into(),
    }))
}

/// GET /api/v1/auth/me
async fn me_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<UserResponseDto>, (StatusCode, Json<nms_common::error::ErrorResponse>)> {
    let user = state
        .user_service
        .get_user_by_id(claims.sub)
        .await
        .map_err(|e| {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::NOT_FOUND);
            (status, Json(e.to_api_response(locale)))
        })?;

    Ok(Json(user.into()))
}
