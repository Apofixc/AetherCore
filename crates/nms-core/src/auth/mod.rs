//! # Подсистема аутентификации и авторизации (RBAC)

pub mod jwt;
pub mod password;

pub use jwt::JwtManager;
pub use password::{hash_password, verify_password};

use nms_common::error::{AppError, Result};
use nms_common::models::user::JwtClaims;

/// Проверить, обладает ли пользователь указанным системным правом
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
