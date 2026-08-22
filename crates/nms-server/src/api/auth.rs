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

use nms_core::db::kv::KvStore;

/// Создать вложенный роутер аутентификации `/auth` (`POST /login`, `GET /me`, `GET /config`)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login_handler))
        .route("/me", get(me_handler))
        .route("/config", get(auth_config_handler))
}

/// Публичная конфигурация авторизации для фронтенда
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfigResponse {
    /// Требуется ли авторизация в веб-интерфейсе
    pub web_ui_auth: bool,
    /// Принудительное 2FA
    pub force_2fa: bool,
    /// Минимальная длина пароля
    pub min_password_length: u32,
    /// Требование заглавных букв
    pub require_uppercase: bool,
    /// Требование цифр
    pub require_digits: bool,
    /// Требование спецсимволов
    pub require_special: bool,
}

/// GET /api/v1/auth/config
///
/// Публичный эндпоинт для проверки статуса авторизации и требований к паролю.
async fn auth_config_handler(
    State(state): State<AppState>,
) -> Json<AuthConfigResponse> {
    let kv = KvStore::system(state.db.clone());
    let policies: Option<serde_json::Value> = kv.get("security_policies").await.unwrap_or(None);

    let web_ui_auth = policies.as_ref()
        .and_then(|p| p.get("web_ui_auth"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let force_2fa = policies.as_ref()
        .and_then(|p| p.get("force_2fa"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let min_password_length = policies.as_ref()
        .and_then(|p| p.get("min_password_length"))
        .and_then(|v| v.as_u64())
        .unwrap_or(8) as u32;

    let require_uppercase = policies.as_ref()
        .and_then(|p| p.get("require_uppercase"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let require_digits = policies.as_ref()
        .and_then(|p| p.get("require_digits"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let require_special = policies.as_ref()
        .and_then(|p| p.get("require_special"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    Json(AuthConfigResponse {
        web_ui_auth,
        force_2fa,
        min_password_length,
        require_uppercase,
        require_digits,
        require_special,
    })
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
