//! # Middleware аутентификации, авторизации и локализации

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use nms_common::error::AppError;
use nms_common::i18n::Locale;
use nms_common::models::user::JwtClaims;
use nms_core::auth::JwtManager;

/// Экстрактор локали из заголовка Accept-Language или query параметра
#[derive(Debug, Clone, Copy)]
pub struct RequestLocale(pub Locale);

impl<S> FromRequestParts<S> for RequestLocale
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let accept_language = parts
            .headers
            .get("accept-language")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("ru");

        Ok(RequestLocale(Locale::from_str_relaxed(accept_language)))
    }
}

/// Экстрактор аутентифицированного пользователя (JWT Claims)
#[derive(Debug, Clone)]
pub struct AuthUser(pub JwtClaims);

/// Структура состояния для верификации токена
pub trait HasJwtManager {
    fn jwt_manager(&self) -> &JwtManager;
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync + HasJwtManager,
{
    type Rejection = AuthErrorResponse;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok());

        let token = match auth_header {
            Some(header) if header.starts_with("Bearer ") => &header[7..],
            _ => {
                return Err(AuthErrorResponse(AppError::Unauthorized {
                    details: "Missing Bearer token in Authorization header".into(),
                }))
            }
        };

        let claims = state
            .jwt_manager()
            .verify_token(token)
            .map_err(AuthErrorResponse)?;

        Ok(AuthUser(claims))
    }
}

/// Обертка ошибки аутентификации для Axum ответа
pub struct AuthErrorResponse(pub AppError);

impl IntoResponse for AuthErrorResponse {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::UNAUTHORIZED);
        let body = self.0.to_api_response(Locale::Ru);
        (status, Json(body)).into_response()
    }
}
