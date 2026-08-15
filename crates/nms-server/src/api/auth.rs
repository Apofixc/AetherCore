//! # Маршруты и обработчики аутентификации API (`/api/v1/auth`)
//!
//! Предоставляет HTTP эндпоинты для входа в систему (`POST /login`)
//! и получения профиля текущего аутентифицированного пользователя (`GET /me`).

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use nms_common::models::user::UserResponseDto;
use serde::{Deserialize, Serialize};

/// Создать вложенный роутер аутентификации `/auth` (`POST /login`, `GET /me`)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login_handler))
        .route("/me", get(me_handler))
}

/// Запрос на аутентификацию по логину и паролю
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    /// Имя пользователя (логин)
    pub username: String,
    /// Пароль в открытом виде
    pub password: String,
}

/// Ответ успешной аутентификации с выпуском токена доступа
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    /// Флаг успеха операции
    pub success: bool,
    /// Выпущенный JWT токен доступа
    pub token: String,
    /// Данные профиля авторизованного пользователя ([`UserResponseDto`])
    pub user: UserResponseDto,
}

/// POST /api/v1/auth/login
///
/// Обработчик входа пользователя. Проверяет учетные данные через [`nms_core::users::UserService::authenticate`],
/// выпускает JWT токен через [`nms_core::auth::JwtManager`] и записывает действие в журнал аудита.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `req` — JSON тело с логином и паролем [`LoginRequest`].
///
/// # Возвращаемое значение
/// Структура [`LoginResponse`] с выпущенным JWT токеном и профилем пользователя.
///
/// # Ошибки
/// * [`StatusCode::UNAUTHORIZED`] — неверный логин или пароль, либо аккаунт заблокирован.
/// * [`StatusCode::INTERNAL_SERVER_ERROR`] — сбой генерации токена.
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
///
/// Обработчик получения профиля текущего пользователя по JWT токену из заголовка `Authorization: Bearer <token>`.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `claims` — Данные авторизованного пользователя [`AuthUser`].
///
/// # Возвращаемое значение
/// Профиль текущего пользователя [`UserResponseDto`].
///
/// # Ошибки
/// * [`StatusCode::UNAUTHORIZED`] — токен отсутствует, просрочен или поврежден.
/// * [`StatusCode::NOT_FOUND`] — пользователь не найден в базе данных.
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
