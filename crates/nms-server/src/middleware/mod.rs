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
use nms_common::error::{AppError, ErrorResponse};
use nms_common::i18n::Locale;
use nms_common::models::user::JwtClaims;
use nms_core::auth::JwtManager;

/// Трейт для извлечения менеджера JWT из разделяемого состояния Axum ([`AppState`](crate::state::AppState))
pub trait HasJwtManager {
    /// Получить ссылку на [`JwtManager`]
    fn jwt_manager(&self) -> &JwtManager;
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
/// Валидирует подпись и срок действия токена. В случае неудачи возвращает [`AuthErrorResponse`] со статусом 401 Unauthorized.
pub struct AuthUser(pub JwtClaims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: HasJwtManager + Send + Sync,
{
    type Rejection = AuthErrorResponse;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok());

        let token = match auth_header {
            Some(header_val) if header_val.starts_with("Bearer ") => {
                &header_val["Bearer ".len()..]
            }
            _ => {
                return Err(AuthErrorResponse(AppError::unauthorized(
                    "Missing Bearer authorization header",
                )))
            }
        };

        let claims = state
            .jwt_manager()
            .verify_token(token)
            .map_err(AuthErrorResponse)?;

        Ok(AuthUser(claims))
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
