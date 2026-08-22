//! # Подсистема аутентификации и авторизации (RBAC)
//!
//! Обеспечивает:
//! - Выпуск и валидацию JWT токенов ([`JwtManager`])
//! - Безопасное хэширование паролей Argon2id ([`hash_password`], [`verify_password`])
//! - Ролевой контроль доступа (RBAC) через [`check_permission`]

pub mod jwt;
pub mod password;

pub use jwt::JwtManager;
pub use password::{hash_password, validate_password_complexity, verify_password};

use nms_common::error::{AppError, Result};
use nms_common::models::user::JwtClaims;

/// Проверить, обладает ли пользователь указанным системным правом доступа
///
/// Суперпользователи (`is_superuser == true`) автоматически обладают всеми правами.
/// Для остальных пользователей проверяется наличие права в векторе `claims.permissions`.
///
/// # Аргументы
/// * `claims` — Клеймы текущего аутентифицированного пользователя ([`JwtClaims`]).
/// * `required_permission` — Идентификатор требуемого права, например `"devices:write"`.
///
/// # Ошибки
/// Возвращает [`AppError::Forbidden`](nms_common::error::AppError), если у пользователя
/// отсутствует требуемое право доступа.
pub fn check_permission(claims: &JwtClaims, required_permission: &str) -> Result<()> {
    if claims.is_superuser {
        return Ok(());
    }

    if claims.permissions.iter().any(|p| p == required_permission) {
        Ok(())
    } else {
        Err(AppError::forbidden(required_permission))
    }
}
