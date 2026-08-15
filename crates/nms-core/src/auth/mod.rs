//! # Подсистема аутентификации и RBAC авторизации

pub mod jwt;
pub mod password;

pub use jwt::JwtManager;
pub use password::{hash_password, verify_password};

use nms_common::error::{AppError, Result};
use nms_common::models::user::JwtClaims;

/// Проверить наличие требуемого права доступа у пользователя
pub fn check_permission(claims: &JwtClaims, required_permission: &str) -> Result<()> {
    // Суперпользователь имеет неограниченный доступ ко всем операциям
    if claims.is_superuser {
        return Ok(());
    }

    if claims.permissions.iter().any(|p| p == required_permission) {
        Ok(())
    } else {
        Err(AppError::Forbidden {
            permission: required_permission.to_string(),
        })
    }
}
