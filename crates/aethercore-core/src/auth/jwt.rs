//! # Управление JWT токенами аутентификации
//!
//! Модуль предоставляет [`JwtManager`] для выпуска (sign) и валидации (verify)
//! токенов JSON Web Token (JWT) с использованием алгоритма HMAC-SHA256 (HS256).

use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use aethercore_common::error::{AppError, Result};
use aethercore_common::models::user::JwtClaims;
use uuid::Uuid;

/// Менеджер JWT токенов для аутентификации пользователей
///
/// Отвечает за генерацию подписанных токенов доступа с заданным временем жизни (TTL)
/// и проверку входящих токенов при авторизации запросов.
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
    /// Создать новый менеджер токенов с указанным секретом и временем жизни
    ///
    /// # Аргументы
    /// * `secret` — Секретный ключ для подписи HMAC-SHA256.
    /// * `ttl_seconds` — Время жизни токена в секундах с момента выпуска.
    ///
    /// # Примеры
    /// ```rust,no_run
    /// use aethercore_core::auth::JwtManager;
    ///
    /// let jwt = JwtManager::new("super-secret-key-32-chars-long!", 3600);
    /// ```
    pub fn new(secret: &str, ttl_seconds: i64) -> Self {
        Self {
            secret: secret.to_string(),
            ttl_seconds,
        }
    }

    /// Сгенерировать новый JWT токен для пользователя
    ///
    /// Формирует payload [`JwtClaims`] со временем создания `iat` и истечения `exp`,
    /// после чего подписывает его секретным ключом менеджера.
    ///
    /// # Аргументы
    /// * `user_id` — Уникальный идентификатор пользователя ([`Uuid`]).
    /// * `username` — Имя пользователя.
    /// * `is_superuser` — Флаг суперпользователя (обходит проверки прав).
    /// * `roles` — Список назначенных ролей.
    /// * `permissions` — Список строковых прав доступа (RBAC), например `["devices:read", "users:write"]`.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Internal`](aethercore_common::error::AppError), если сериализация
    /// или криптографическая подпись токена завершилась сбоем.
    pub fn generate_token(
        &self,
        user_id: Uuid,
        username: &str,
        is_superuser: bool,
        roles: Vec<String>,
        permissions: Vec<String>,
    ) -> Result<String> {
        self.generate_token_with_ttl(user_id, username, is_superuser, roles, permissions, self.ttl_seconds)
    }

    /// Сгенерировать новый JWT токен для пользователя с явным указанием TTL
    ///
    /// # Аргументы
    /// * `user_id` — Уникальный идентификатор пользователя ([`Uuid`]).
    /// * `username` — Имя пользователя.
    /// * `is_superuser` — Флаг суперпользователя.
    /// * `roles` — Список назначенных ролей.
    /// * `permissions` — Список строковых прав доступа.
    /// * `ttl_seconds` — Время жизни токена в секундах.
    pub fn generate_token_with_ttl(
        &self,
        user_id: Uuid,
        username: &str,
        is_superuser: bool,
        roles: Vec<String>,
        permissions: Vec<String>,
        ttl_seconds: i64,
    ) -> Result<String> {
        let now = Utc::now().timestamp();
        let exp = now + ttl_seconds;

        let claims = JwtClaims {
            sub: user_id,
            username: username.to_string(),
            is_superuser,
            roles,
            permissions,
            iat: now,
            exp,
        };

        let encoding_key = EncodingKey::from_secret(self.secret.as_bytes());
        encode(&Header::default(), &claims, &encoding_key).map_err(|e| {
            AppError::internal(format!("JWT encoding error: {}", e))
        })
    }

    /// Проверить и декодировать JWT токен
    ///
    /// Выполняет криптографическую валидацию подписи токена и проверку срока действия (`exp`).
    ///
    /// # Аргументы
    /// * `token` — Строка JWT токена (обычно из заголовка `Authorization: Bearer <token>`).
    ///
    /// # Возвращаемое значение
    /// Возвращает раскодированные клеймы [`JwtClaims`] при успешной валидации.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Unauthorized`](aethercore_common::error::AppError), если токен
    /// невалиден, поврежден, подписан другим ключом или срок его действия истек.
    pub fn verify_token(&self, token: &str) -> Result<JwtClaims> {
        let mut validation = Validation::default();
        validation.validate_exp = true;

        let decoding_key = DecodingKey::from_secret(self.secret.as_bytes());
        let token_data = decode::<JwtClaims>(token, &decoding_key, &validation).map_err(
            |e| AppError::unauthorized(format!("Invalid or expired token: {}", e)),
        )?;

        Ok(token_data.claims)
    }
}
