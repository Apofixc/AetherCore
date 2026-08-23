//! # Маршруты и обработчики аутентификации API (`/api/v1/auth`)
//!
//! Предоставляет HTTP эндпоинты для:
//! - Входа в систему с поддержкой 2FA (`POST /login`, `POST /2fa/verify-login`)
//! - Управления двухфакторной аутентификацией (`POST /2fa/setup`, `POST /2fa/enable`, `POST /2fa/disable`, `POST /2fa/backup-codes/regenerate`)
//! - Получения профиля текущего пользователя (`GET /me`)
//! - Проверки публичной конфигурации политик авторизации (`GET /config`)

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use aethercore_common::error::AppError;
use aethercore_common::models::user::{SecurityPoliciesDto, SessionDto, UserResponseDto};
use aethercore_core::auth::totp::{
    generate_backup_codes, generate_otpauth_url, generate_qr_code_data_url, generate_totp_secret,
    verify_totp_code,
};
use aethercore_core::db::kv::KvStore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Создать вложенный роутер аутентификации `/auth`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login_handler))
        .route("/me", get(me_handler))
        .route("/config", get(auth_config_handler))
        .route("/2fa/verify-login", post(verify_login_2fa_handler))
        .route("/2fa/setup", post(setup_2fa_handler))
        .route("/2fa/enable", post(enable_2fa_handler))
        .route("/2fa/disable", post(disable_2fa_handler))
        .route("/2fa/backup-codes/regenerate", post(regenerate_backup_codes_handler))
        .route("/sessions", get(list_my_sessions_handler))
        .route("/sessions/{id}", axum::routing::delete(revoke_my_session_handler))
        .route("/sessions/terminate-others", post(terminate_my_other_sessions_handler))
        .route("/sessions/terminate-all", post(terminate_my_all_sessions_handler))
}

/// Публичная конфигурация авторизации для фронтенда
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfigResponse {
    /// Требуется ли авторизация в веб-интерфейсе
    pub web_ui_auth: bool,
    /// Принудительное 2FA (для обратной совместимости)
    pub force_2fa: bool,
    /// Область действия политики 2FA ("disabled" | "admins_only" | "all")
    pub mfa_scope: String,
    /// Период доверия к устройствам в днях
    pub mfa_remember_device_days: u32,
    /// Льготный период на настройку в днях
    pub mfa_grace_period_days: u32,
    /// Количество резервных кодов
    pub mfa_backup_codes_count: u32,
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

/// Проверяет, является ли двухфакторная аутентификация обязательной для данного пользователя
pub fn is_mfa_enforced_for_user(policy: &SecurityPoliciesDto, user: &aethercore_common::models::user::User) -> bool {
    if let Some(enforced) = user.force_2fa {
        return enforced;
    }
    match policy.mfa_scope.as_str() {
        "all" => true,
        "admins_only" => user.is_superuser || user.roles.iter().any(|r| r == "admin" || r == "superuser"),
        "disabled" => policy.force_2fa,
        _ => policy.force_2fa,
    }
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

    let force_2fa_effective = policy.force_2fa || policy.mfa_scope != "disabled";

    Json(AuthConfigResponse {
        web_ui_auth: policy.web_ui_auth,
        force_2fa: force_2fa_effective,
        mfa_scope: policy.mfa_scope,
        mfa_remember_device_days: policy.mfa_remember_device_days,
        mfa_grace_period_days: policy.mfa_grace_period_days,
        mfa_backup_codes_count: policy.mfa_backup_codes_count,
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
    /// Опциональный 6-значный TOTP-код или резервный код при прямом входе
    #[serde(default)]
    pub totp_code: Option<String>,
    /// Является ли переданный код резервным (backup code)
    #[serde(default)]
    pub is_backup_code: Option<bool>,
}

/// Ответ аутентификации с выпуском токена доступа либо запросом 2FA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    /// Флаг успеха операции
    pub success: bool,
    /// Выпущенный JWT токен доступа (при успешном входе)
    #[serde(default)]
    pub token: String,
    /// Данные профиля авторизованного пользователя ([`UserResponseDto`])
    #[serde(default)]
    pub user: Option<UserResponseDto>,
    /// Требуется ли прохождение второго фактора аутентификации
    #[serde(default)]
    pub requires_2fa: bool,
    /// Временный токен для подтверждения второго фактора на шаге 2
    #[serde(default)]
    pub temp_token: Option<String>,
    /// Количество оставшихся резервных кодов (если использовался бэкап-код)
    #[serde(default)]
    pub backup_codes_left: Option<usize>,
}

/// Извлечь User-Agent клиента из заголовков HTTP
fn extract_user_agent(headers: &axum::http::HeaderMap) -> String {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Web Client")
        .to_string()
}

/// POST /api/v1/auth/login
///
/// Обработчик входа пользователя. Проверяет учетные данные через [`aethercore_core::users::UserService::authenticate`],
/// проверяет статус 2FA, выпускает JWT токен и записывает действие в журнал аудита.
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
        let err = AppError::forbidden(
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

    let user_agent = extract_user_agent(&headers);

    // Проверка необходимости 2FA
    if user.is_totp_enabled {
        let mut backup_left = None;

        if let Some(ref code) = req.totp_code {
            let is_backup = req.is_backup_code.unwrap_or(false);
            if is_backup {
                let remaining = state
                    .user_service
                    .consume_backup_code(user.id, code)
                    .await
                    .map_err(|e| {
                        (StatusCode::UNAUTHORIZED, Json(e.to_api_response(locale)))
                    })?;
                backup_left = Some(remaining);
            } else {
                let secret = user.totp_secret.as_deref().unwrap_or_default();
                if !verify_totp_code(secret, &user.username, code) {
                    let err = AppError::unauthorized("Invalid TOTP verification code")
                        .with_i18n_key("core.auth.invalid_totp");
                    return Err((StatusCode::UNAUTHORIZED, Json(err.to_api_response(locale))));
                }
            }
        } else {
            // Код не передан - возвращаем 2FA Challenge с временным токеном
            let temp_token = state
                .jwt_manager
                .generate_token_with_ttl(
                    user.id,
                    &user.username,
                    false,
                    vec!["2fa_pending".to_string()],
                    vec![],
                    300, // 5 минут
                )
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(e.to_api_response(locale)),
                    )
                })?;

            return Ok(Json(LoginResponse {
                success: false,
                token: String::new(),
                user: None,
                requires_2fa: true,
                temp_token: Some(temp_token),
                backup_codes_left: None,
            }));
        }

        let ttl_seconds = (policy.session_ttl.max(1) as i64) * 3600;

        let session = state
            .session_service
            .create_session(
                user.id,
                &user.username,
                &user.roles,
                &client_ip,
                &user_agent,
                ttl_seconds,
            )
            .await
            .map_err(|e| {
                let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                (status, Json(e.to_api_response(locale)))
            })?;

        let token = state
            .jwt_manager
            .generate_token_with_session(
                user.id,
                &user.username,
                user.is_superuser,
                user.roles.clone(),
                user.permissions.clone(),
                Some(session.id),
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
                "auth.login_2fa",
                "auth",
                "success",
                Some(&format!("session_id: {}", session.id)),
                Some(&client_ip),
            )
            .await;

        return Ok(Json(LoginResponse {
            success: true,
            token,
            user: Some(user.into()),
            requires_2fa: false,
            temp_token: None,
            backup_codes_left: backup_left,
        }));
    }

    let ttl_seconds = (policy.session_ttl.max(1) as i64) * 3600;

    let session = state
        .session_service
        .create_session(
            user.id,
            &user.username,
            &user.roles,
            &client_ip,
            &user_agent,
            ttl_seconds,
        )
        .await
        .map_err(|e| {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(e.to_api_response(locale)))
        })?;

    let token = state
        .jwt_manager
        .generate_token_with_session(
            user.id,
            &user.username,
            user.is_superuser,
            user.roles.clone(),
            user.permissions.clone(),
            Some(session.id),
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
            Some(&format!("session_id: {}", session.id)),
            Some(&client_ip),
        )
        .await;

    Ok(Json(LoginResponse {
        success: true,
        token,
        user: Some(user.into()),
        requires_2fa: false,
        temp_token: None,
        backup_codes_left: None,
    }))
}

/// Запрос подтверждения входа по 2FA
#[derive(Debug, Deserialize)]
pub struct VerifyLogin2faRequest {
    /// Временный токен проверки
    pub temp_token: String,
    /// 6-значный TOTP-код или резервный код
    pub code: String,
    /// Является ли переданный код резервным (backup code)
    #[serde(default)]
    pub is_backup_code: Option<bool>,
}

/// POST /api/v1/auth/2fa/verify-login
///
/// Обработчик второго этапа авторизации с кодом из аутентификатора
async fn verify_login_2fa_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    headers: axum::http::HeaderMap,
    Json(req): Json<VerifyLogin2faRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let claims = state
        .jwt_manager
        .verify_token(&req.temp_token)
        .map_err(|e| {
            (StatusCode::UNAUTHORIZED, Json(e.to_api_response(locale)))
        })?;

    let user = state
        .user_service
        .get_user_by_id(claims.sub)
        .await
        .map_err(|e| {
            (StatusCode::UNAUTHORIZED, Json(e.to_api_response(locale)))
        })?;

    if !user.is_active {
        let err = AppError::unauthorized("Account is disabled")
            .with_i18n_key("core.auth.account_disabled");
        return Err((StatusCode::UNAUTHORIZED, Json(err.to_api_response(locale))));
    }

    let is_backup = req.is_backup_code.unwrap_or(false);
    let mut backup_left = None;

    if is_backup {
        let remaining = state
            .user_service
            .consume_backup_code(user.id, &req.code)
            .await
            .map_err(|e| {
                (StatusCode::UNAUTHORIZED, Json(e.to_api_response(locale)))
            })?;
        backup_left = Some(remaining);
    } else {
        let secret = user.totp_secret.as_deref().unwrap_or_default();
        if !verify_totp_code(secret, &user.username, &req.code) {
            let err = AppError::unauthorized("Invalid TOTP verification code")
                .with_i18n_key("core.auth.invalid_totp");
            return Err((StatusCode::UNAUTHORIZED, Json(err.to_api_response(locale))));
        }
    }

    let kv = KvStore::system(state.db.clone());
    let policy: SecurityPoliciesDto = kv
        .get("security_policies")
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    let client_ip = crate::middleware::extract_client_ip(&headers);
    let user_agent = extract_user_agent(&headers);
    let ttl_seconds = (policy.session_ttl.max(1) as i64) * 3600;

    let session = state
        .session_service
        .create_session(
            user.id,
            &user.username,
            &user.roles,
            &client_ip,
            &user_agent,
            ttl_seconds,
        )
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_api_response(locale)))
        })?;

    let token = state
        .jwt_manager
        .generate_token_with_session(
            user.id,
            &user.username,
            user.is_superuser,
            user.roles.clone(),
            user.permissions.clone(),
            Some(session.id),
            ttl_seconds,
        )
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(e.to_api_response(locale)),
            )
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&user.id.to_string()),
            Some(&user.username),
            "auth.login_2fa",
            "auth",
            "success",
            Some(&format!("session_id: {}", session.id)),
            Some(&client_ip),
        )
        .await;

    Ok(Json(LoginResponse {
        success: true,
        token,
        user: Some(user.into()),
        requires_2fa: false,
        temp_token: None,
        backup_codes_left: backup_left,
    }))
}

/// Ответ с параметрами для первичной настройки 2FA
#[derive(Debug, Serialize, Deserialize)]
pub struct TotpSetupResponse {
    /// Секретный ключ Base32 для ручного ввода
    pub secret: String,
    /// Data URL QR-кода формата PNG base64
    pub qr_code_url: String,
    /// Стандартный otpauth URL
    pub otpauth_url: String,
    /// Набор из 8 одноразовых резервных кодов восстановления
    pub backup_codes: Vec<String>,
}

/// POST /api/v1/auth/2fa/setup
///
/// Генерация временного секрета, QR-кода и резервных кодов для настройки 2FA в профиле
async fn setup_2fa_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<TotpSetupResponse>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let user = state
        .user_service
        .get_user_by_id(claims.sub)
        .await
        .map_err(|e| {
            (StatusCode::NOT_FOUND, Json(e.to_api_response(locale)))
        })?;

    let kv = KvStore::system(state.db.clone());
    let policy: SecurityPoliciesDto = kv
        .get("security_policies")
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    let secret = generate_totp_secret();
    let qr_code_url = generate_qr_code_data_url(&secret, &user.username)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_api_response(locale))))?;
    let otpauth_url = generate_otpauth_url(&secret, &user.username)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_api_response(locale))))?;
    let count = policy.mfa_backup_codes_count.clamp(8, 16) as usize;
    let backup_codes = generate_backup_codes(count);

    Ok(Json(TotpSetupResponse {
        secret,
        qr_code_url,
        otpauth_url,
        backup_codes,
    }))
}

/// Запрос активации 2FA
#[derive(Debug, Deserialize)]
pub struct Enable2faRequest {
    /// Секретный ключ Base32
    pub secret: String,
    /// Проверочный 6-значный код из приложения
    pub code: String,
    /// Список 8 резервных кодов
    pub backup_codes: Vec<String>,
}

/// POST /api/v1/auth/2fa/enable
///
/// Подтверждение и включение 2FA для текущего пользователя
async fn enable_2fa_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(req): Json<Enable2faRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let user = state
        .user_service
        .get_user_by_id(claims.sub)
        .await
        .map_err(|e| {
            (StatusCode::NOT_FOUND, Json(e.to_api_response(locale)))
        })?;

    if !verify_totp_code(&req.secret, &user.username, &req.code) {
        let err = AppError::validation("code", "Invalid verification code")
            .with_i18n_key("core.auth.invalid_totp");
        return Err((StatusCode::BAD_REQUEST, Json(err.to_api_response(locale))));
    }

    state
        .user_service
        .enable_totp(user.id, &req.secret, &req.backup_codes)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_api_response(locale)))
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&user.id.to_string()),
            Some(&user.username),
            "security.2fa_enabled",
            "security",
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Two-factor authentication successfully enabled"
    })))
}

/// Запрос отключения 2FA
#[derive(Debug, Deserialize)]
pub struct Disable2faRequest {
    /// Текущий пароль для подтверждения
    pub password: Option<String>,
    /// Или текущий TOTP-код
    pub code: Option<String>,
}

/// POST /api/v1/auth/2fa/disable
///
/// Отключение 2FA для текущего пользователя (с проверкой политики `force_2fa`)
async fn disable_2fa_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(req): Json<Disable2faRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let kv = KvStore::system(state.db.clone());
    let policy: SecurityPoliciesDto = kv
        .get("security_policies")
        .await
        .unwrap_or_default()
        .unwrap_or_default();

    let user = state
        .user_service
        .get_user_by_id(claims.sub)
        .await
        .map_err(|e| {
            (StatusCode::NOT_FOUND, Json(e.to_api_response(locale)))
        })?;

    if is_mfa_enforced_for_user(&policy, &user) {
        let err = AppError::forbidden("Cannot disable 2FA when mandatory 2FA policy is enforced by administrator")
            .with_i18n_key("core.auth.force_2fa_active");
        return Err((StatusCode::FORBIDDEN, Json(err.to_api_response(locale))));
    }

    let mut is_authorized = false;
    if let Some(ref pwd) = req.password {
        if aethercore_core::auth::verify_password(pwd, &user.password_hash).unwrap_or(false) {
            is_authorized = true;
        }
    }

    if !is_authorized {
        if let Some(ref code) = req.code {
            if let Some(ref secret) = user.totp_secret {
                if verify_totp_code(secret, &user.username, code) {
                    is_authorized = true;
                }
            }
        }
    }

    if !is_authorized {
        let err = AppError::unauthorized("Invalid password or TOTP code")
            .with_i18n_key("core.auth.invalid_credentials");
        return Err((StatusCode::UNAUTHORIZED, Json(err.to_api_response(locale))));
    }

    state
        .user_service
        .disable_totp(user.id)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_api_response(locale)))
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&user.id.to_string()),
            Some(&user.username),
            "security.2fa_disabled",
            "security",
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Two-factor authentication disabled"
    })))
}

/// Запрос перевыпуска резервных кодов
#[derive(Debug, Deserialize)]
pub struct RegenerateBackupCodesRequest {
    /// Текущий пароль для подтверждения
    pub password: Option<String>,
}

/// POST /api/v1/auth/2fa/backup-codes/regenerate
///
/// Перевыпуск набора из 8 резервных кодов
async fn regenerate_backup_codes_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(req): Json<RegenerateBackupCodesRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let user = state
        .user_service
        .get_user_by_id(claims.sub)
        .await
        .map_err(|e| {
            (StatusCode::NOT_FOUND, Json(e.to_api_response(locale)))
        })?;

    if let Some(ref pwd) = req.password {
        if !aethercore_core::auth::verify_password(pwd, &user.password_hash).unwrap_or(false) {
            let err = AppError::unauthorized("Invalid password")
                .with_i18n_key("core.auth.invalid_credentials");
            return Err((StatusCode::UNAUTHORIZED, Json(err.to_api_response(locale))));
        }
    }

    let kv = KvStore::system(state.db.clone());
    let policy: SecurityPoliciesDto = kv
        .get("security_policies")
        .await
        .unwrap_or_default()
        .unwrap_or_default();
    let count = policy.mfa_backup_codes_count.clamp(8, 16) as usize;

    let raw_codes = state
        .user_service
        .regenerate_backup_codes(user.id, count)
        .await
        .map_err(|e| {
            (StatusCode::BAD_REQUEST, Json(e.to_api_response(locale)))
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&user.id.to_string()),
            Some(&user.username),
            "security.2fa_backup_codes_regenerated",
            "security",
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "backup_codes": raw_codes
    })))
}

/// GET /api/v1/auth/me
///
/// Обработчик получения профиля текущего пользователя по JWT токену из заголовка `Authorization: Bearer <token>`.
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

/// GET /api/v1/auth/sessions
///
/// Получить список всех активных устройств/сессий текущего пользователя.
async fn list_my_sessions_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<SessionDto>>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let raw_sessions = state
        .session_service
        .list_user_sessions(claims.sub)
        .await
        .map_err(|e| {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(e.to_api_response(locale)))
        })?;

    let dtos: Vec<SessionDto> = raw_sessions
        .into_iter()
        .map(|s| s.into_dto(claims.session_id))
        .collect();

    Ok(Json(dtos))
}

/// DELETE /api/v1/auth/sessions/{id}
///
/// Завершить конкретный сеанс текущего пользователя.
async fn revoke_my_session_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(session_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    // Проверяем, что сессия принадлежит текущему пользователю
    if let Ok(Some(sess)) = state.session_service.get_session(session_id).await {
        if sess.user_id != claims.sub {
            return Err((
                StatusCode::FORBIDDEN,
                Json(AppError::forbidden("Cannot terminate another user's session").to_api_response(locale)),
            ));
        }
    }

    state
        .session_service
        .revoke_session(session_id)
        .await
        .map_err(|e| {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(e.to_api_response(locale)))
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "auth.session_revoked",
            "auth.sessions",
            "success",
            Some(&format!("User revoked own session {}", session_id)),
            None,
        )
        .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "session_id": session_id
    })))
}

/// POST /api/v1/auth/sessions/terminate-others
///
/// Завершить сеансы на всех других устройствах текущего пользователя.
async fn terminate_my_other_sessions_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let terminated_count = if let Some(current_sid) = claims.session_id {
        state
            .session_service
            .revoke_user_sessions_except(claims.sub, current_sid)
            .await
            .map_err(|e| {
                let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                (status, Json(e.to_api_response(locale)))
            })?
    } else {
        0
    };

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "auth.other_sessions_terminated",
            "auth.sessions",
            "success",
            Some(&format!("Terminated {} other sessions for user", terminated_count)),
            None,
        )
        .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "terminated_count": terminated_count
    })))
}

/// POST /api/v1/auth/sessions/terminate-all
///
/// Завершить абсолютно все сеансы текущего пользователя (включая текущий).
async fn terminate_my_all_sessions_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<aethercore_common::error::ErrorResponse>)> {
    let terminated_count = state
        .session_service
        .revoke_user_sessions(claims.sub)
        .await
        .map_err(|e| {
            let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            (status, Json(e.to_api_response(locale)))
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "auth.all_sessions_terminated",
            "auth.sessions",
            "success",
            Some(&format!("Terminated all {} sessions for user", terminated_count)),
            None,
        )
        .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "terminated_count": terminated_count
    })))
}
