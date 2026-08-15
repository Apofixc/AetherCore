//! # Управление JWT токенами аутентификации

use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use nms_common::error::{AppError, Result};
use nms_common::models::user::JwtClaims;
use uuid::Uuid;

/// Менеджер JWT токенов
#[derive(Clone)]
pub struct JwtManager {
    secret: String,
    ttl_seconds: i64,
}

impl std::fmt::Debug for JwtManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtManager")
            .field("ttl_seconds", &self.ttl_seconds)
            .finish()
    }
}

impl JwtManager {
    /// Создать новый менеджер токенов с указанным секретом
    pub fn new(secret: &str, ttl_seconds: i64) -> Self {
        Self {
            secret: secret.to_string(),
            ttl_seconds,
        }
    }

    /// Сгенерировать новый JWT токен для пользователя
    pub fn generate_token(
        &self,
        user_id: Uuid,
        username: &str,
        is_superuser: bool,
        permissions: Vec<String>,
    ) -> Result<String> {
        let now = Utc::now().timestamp();
        let exp = now + self.ttl_seconds;

        let claims = JwtClaims {
            sub: user_id,
            username: username.to_string(),
            is_superuser,
            permissions,
            iat: now,
            exp,
        };

        let encoding_key = EncodingKey::from_secret(self.secret.as_bytes());
        encode(&Header::default(), &claims, &encoding_key).map_err(|e| AppError::Internal {
            details: format!("JWT encoding error: {}", e),
        })
    }

    /// Проверить и декодировать JWT токен
    pub fn verify_token(&self, token: &str) -> Result<JwtClaims> {
        let mut validation = Validation::default();
        validation.validate_exp = true;

        let decoding_key = DecodingKey::from_secret(self.secret.as_bytes());
        let token_data = decode::<JwtClaims>(token, &decoding_key, &validation).map_err(
            |e| AppError::Unauthorized {
                details: format!("Invalid or expired token: {}", e),
            },
        )?;

        Ok(token_data.claims)
    }
}
