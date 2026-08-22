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
use aethercore_common::models::user::UserResponseDto;
use serde::{Deserialize, Serialize};

use aethercore_core::db::kv::KvStore;

/// Создать вложенный роутер аутентификации `/auth` (`POST /login`, `GET /me`, `GET /config`)
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login_handler))
        .route("/me", get(me_handler))
        .route("/config", get(auth_config_handler))
}

use aethercore_common::models::user::SecurityPoliciesDto;

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
    /// Время жизни сессии в часах
    pub session_ttl: u32,
    /// Таймаут неактивности пользователя в минутах
    pub inactivity_timeout: u32,
    /// Максимальное число попыток входа
    pub max_login_attempts: u32,
    /// Длительность блокировки в минутах
    pub lockout_duration: u32,
}

/// GET /api/v1/auth/config
///
/// Публичный эндпоинт для проверки статуса авторизации и требований к паролю.
async fn auth_config_handler(
    State(state): State<AppState>,
) -> Json<AuthConfigResponse> {
    let kv = KvStore::system(state.db.clone());
    let policy: SecurityPoliciesDto = kv
        .get("security_policies")
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    Json(AuthConfigResponse {
        web_ui_auth: policy.web_ui_auth,
        force_2fa: policy.force_2fa,
        min_password_length: policy.min_password_length,
        require_uppercase: policy.require_uppercase,
        require_digits: policy.require_digits,
        require_special: policy.require_special,
        session_ttl: policy.session_ttl,
        inactivity_timeout: policy.inactivity_timeout,
        max_login_attempts: policy.max_login_attempts,
        lockout_duration: policy.lockout_duration,
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
/// Обработчик входа пользователя. Проверяет учетные данные через [`aethercore_core::users::UserService::authenticate`],
/// выпускает JWT токен через [`aethercore_core::auth::JwtManager`] и записывает действие в журнал аудита.
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
    headers: axum::http::HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let kv = KvStore::system(state.db.clone());
    let policy: SecurityPoliciesDto = kv
        .get("security_policies")
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    let client_ip = crate::middleware::extract_client_ip(&headers);
    if !crate::middleware::is_ip_allowed(&client_ip, &policy.ip_whitelist) {
        let err = aethercore_common::error::AppError::forbidden(
            "Client IP is not allowed by security policy whitelist",
        );
        return Err((StatusCode::FORBIDDEN, Json(err.to_api_response(locale))));
    }

    let user = state
        .user_service
        .authenticate(&req.username, &req.password)
        .await
        .map_err(|e| {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::UNAUTHORIZED);
            (status, Json(e.to_api_response(locale)))
        })?;

    let ttl_seconds = (policy.session_ttl.max(1) as i64) * 3600;

    let token = state
        .jwt_manager
        .generate_token_with_ttl(
            user.id,
            &user.username,
            user.is_superuser,
            user.permissions.clone(),
            ttl_seconds,
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
) -> Result<Json<UserResponseDto>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
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
