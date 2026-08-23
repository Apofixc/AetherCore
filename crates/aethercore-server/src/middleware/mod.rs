//! # HTTP Middleware и экстракторы для веб-сервера Axum
//!
//! Модуль содержит экстракторы Axum ([`FromRequestParts`]):
//! - [`RequestLocale`]: автоматическое извлечение и определение языка пользователя из HTTP-заголовка `Accept-Language`.
//! - [`AuthUser`]: проверка и валидация JWT токена из заголовка `Authorization: Bearer <token>` с извлечением прав доступа ([`JwtClaims`]).
//! - [`HasJwtManager`]: трейт состояния приложения для доступа к менеджеру JWT токенов.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use aethercore_common::error::{AppError, ErrorResponse};
use aethercore_common::i18n::Locale;
use aethercore_common::models::user::JwtClaims;
use aethercore_core::auth::JwtManager;

/// Трейт для извлечения менеджера JWT и БД из разделяемого состояния Axum ([`AppState`](crate::state::AppState))
pub trait HasJwtManager {
    /// Получить ссылку на [`JwtManager`]
    fn jwt_manager(&self) -> &JwtManager;
    /// Получить ссылку на базу данных SQLite (если доступна)
    fn db(&self) -> Option<&aethercore_core::db::Db> {
        None
    }
    /// Получить ссылку на сервис сессий (если доступен)
    fn session_service(&self) -> Option<&aethercore_core::services::SessionService> {
        None
    }
}

/// Extractor для определения локали клиента из заголовка `Accept-Language`
///
/// Всегда завершается успешно (fall-through в [`Locale::Ru`] при отсутствии заголовка).
pub struct RequestLocale(pub Locale);

impl<S> FromRequestParts<S> for RequestLocale
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let locale_str = parts
            .headers
            .get("Accept-Language")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        Ok(RequestLocale(Locale::from_str_relaxed(locale_str)))
    }
}

/// Extractor для извлечения аутентифицированного пользователя из заголовка `Authorization: Bearer <token>`
///
/// Валидирует подпись и срок действия токена. Если токен отсутствует, проверяет системную политику `web_ui_auth`.
/// Если авторизация веб-интерфейса отключена, автоматически предоставляет права Суперпользователя.
pub struct AuthUser(pub JwtClaims);

use std::net::IpAddr;

/// Извлечь IP-адрес клиента из HTTP-заголовков (X-Forwarded-For, X-Real-IP)
pub fn extract_client_ip(headers: &axum::http::HeaderMap) -> String {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok()) {
        if let Some(first_ip) = xff.split(',').next() {
            let trimmed = first_ip.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(real_ip) = headers.get("x-real-ip").and_then(|h| h.to_str().ok()) {
        let trimmed = real_ip.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "127.0.0.1".to_string()
}

/// Проверить, разрешен ли IP-адрес согласно белому списку
pub fn is_ip_allowed(client_ip: &str, whitelist: &str) -> bool {
    let trimmed = whitelist.trim();
    if trimmed.is_empty() {
        return true;
    }

    let client_ip_clean = client_ip.trim();
    if client_ip_clean.is_empty() {
        return true;
    }

    // Localhost всегда разрешен (Anti-Lockout)
    if client_ip_clean == "127.0.0.1" || client_ip_clean == "::1" || client_ip_clean == "localhost" {
        return true;
    }

    let client_parsed: IpAddr = match client_ip_clean.parse() {
        Ok(ip) => ip,
        Err(_) => return false,
    };

    // Разделители: запятая, точка с запятой или пробел
    for entry in trimmed.split([',', ';', ' ']) {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }

        // Поддержка точного IP
        if let Ok(ip) = entry.parse::<IpAddr>() {
            if ip == client_parsed {
                return true;
            }
        }

        // Поддержка CIDR подсетей (например 192.168.1.0/24)
        if let Some((net_str, mask_str)) = entry.split_once('/') {
            if let (Ok(net_ip), Ok(prefix_len)) = (net_str.parse::<IpAddr>(), mask_str.parse::<u8>()) {
                match (client_parsed, net_ip) {
                    (IpAddr::V4(c), IpAddr::V4(n)) if prefix_len <= 32 => {
                        let mask = if prefix_len == 0 { 0u32 } else { !0u32 << (32 - prefix_len) };
                        let c_u32 = u32::from(c);
                        let n_u32 = u32::from(n);
                        if (c_u32 & mask) == (n_u32 & mask) {
                            return true;
                        }
                    }
                    (IpAddr::V6(c), IpAddr::V6(n)) if prefix_len <= 128 => {
                        let mask = if prefix_len == 0 { 0u128 } else { !0u128 << (128 - prefix_len) };
                        let c_u128 = u128::from(c);
                        let n_u128 = u128::from(n);
                        if (c_u128 & mask) == (n_u128 & mask) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    false
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: HasJwtManager + Send + Sync,
{
    type Rejection = AuthErrorResponse;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Проверка IP Whitelist при наличии подключения к БД
        if let Some(db) = state.db() {
            let kv = aethercore_core::db::kv::KvStore::system(db.clone());
            if let Ok(Some(policies)) = kv.get::<aethercore_common::models::user::SecurityPoliciesDto>("security_policies").await {
                let client_ip = extract_client_ip(&parts.headers);
                if !is_ip_allowed(&client_ip, &policies.ip_whitelist) {
                    return Err(AuthErrorResponse(AppError::forbidden(
                        "Client IP is not allowed by security policy whitelist",
                    )));
                }

                let auth_header = parts
                    .headers
                    .get(AUTHORIZATION)
                    .and_then(|h| h.to_str().ok());

                let token = match auth_header {
                    Some(header_val) if header_val.starts_with("Bearer ") => {
                        Some(&header_val["Bearer ".len()..])
                    }
                    _ => None,
                };

                if let Some(t) = token {
                    let claims = state
                        .jwt_manager()
                        .verify_token(t)
                        .map_err(AuthErrorResponse)?;

                    if let Some(session_id) = claims.session_id {
                        if let Some(session_srv) = state.session_service() {
                            match session_srv.is_session_valid(session_id).await {
                                Ok(true) => {
                                    let _ = session_srv.touch_session(session_id).await;
                                }
                                _ => {
                                    return Err(AuthErrorResponse(
                                        AppError::unauthorized("Session has been revoked or expired")
                                            .with_i18n_key("core.auth.session_revoked"),
                                    ));
                                }
                            }
                        }
                    }

                    return Ok(AuthUser(claims));
                }

                if !policies.web_ui_auth {
                    return Ok(AuthUser(JwtClaims {
                        sub: uuid::Uuid::nil(),
                        username: "anonymous_admin".to_string(),
                        is_superuser: true,
                        roles: vec!["superuser".to_string()],
                        permissions: vec![
                            "events.view".to_string(),
                            "modules.manage".to_string(),
                            "modules.view".to_string(),
                            "system.manage".to_string(),
                            "system.view".to_string(),
                            "users.manage".to_string(),
                            "users.view".to_string(),
                            "settings.view".to_string(),
                            "settings.manage".to_string(),
                        ],
                        exp: 0,
                        iat: 0,
                        session_id: None,
                    }));
                }
            }
        }

        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok());

        let token = match auth_header {
            Some(header_val) if header_val.starts_with("Bearer ") => {
                Some(&header_val["Bearer ".len()..])
            }
            _ => None,
        };

        if let Some(t) = token {
            let claims = state
                .jwt_manager()
                .verify_token(t)
                .map_err(AuthErrorResponse)?;

            if let Some(session_id) = claims.session_id {
                if let Some(session_srv) = state.session_service() {
                    match session_srv.is_session_valid(session_id).await {
                        Ok(true) => {
                            let _ = session_srv.touch_session(session_id).await;
                        }
                        _ => {
                            return Err(AuthErrorResponse(
                                AppError::unauthorized("Session has been revoked or expired")
                                    .with_i18n_key("core.auth.session_revoked"),
                            ));
                        }
                    }
                }
            }

            return Ok(AuthUser(claims));
        }

        Err(AuthErrorResponse(AppError::unauthorized(
            "Missing Bearer authorization header",
        )))
    }
}

/// Обертка ошибки аутентификации для конвертации в типизированный JSON HTTP-ответ
pub struct AuthErrorResponse(pub AppError);

impl IntoResponse for AuthErrorResponse {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::UNAUTHORIZED);
        let error_response: ErrorResponse = self.0.to_api_response(Locale::Ru);
        (status, Json(error_response)).into_response()
    }
}
