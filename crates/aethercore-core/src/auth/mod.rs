//! # Подсистема аутентификации и авторизации (RBAC)
//!
//! Обеспечивает:
//! - Выпуск и валидацию JWT токенов ([`JwtManager`])
//! - Безопасное хэширование паролей Argon2id ([`hash_password`], [`verify_password`])
//! - Ролевой контроль доступа (RBAC) через [`check_permission`]

pub mod jwt;
pub mod password;
pub mod totp;

pub use jwt::JwtManager;
pub use password::{hash_password, validate_password_complexity, verify_password};
pub use totp::*;

use aethercore_common::error::{AppError, Result};
use aethercore_common::models::user::JwtClaims;

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
/// Возвращает [`AppError::Forbidden`](aethercore_common::error::AppError), если у пользователя
/// отсутствует требуемое право доступа.
pub fn check_permission(claims: &JwtClaims, required_permission: &str) -> Result<()> {
    if claims.is_superuser {
        return Ok(());
    }

    // 1. Прямое совпадение или wildcard
    if claims.permissions.iter().any(|p| p == "*" || p == required_permission) {
        return Ok(());
    }

    // 2. Иерархическое наследование (manage в том же домене дает доступ к view)
    if let Some(domain) = required_permission.strip_suffix(".view") {
        let manage_perm = format!("{}.manage", domain);
        if claims.permissions.iter().any(|p| p == &manage_perm) {
            return Ok(());
        }
    }

    // 3. Обратная совместимость для составных прав
    if (required_permission == "access.roles.view" || required_permission == "settings.view" || required_permission == "audit.view")
        && claims.permissions.iter().any(|p| p == "access.view" || p == "access.manage" || p == "system.view" || p == "system.manage")
    {
        return Ok(());
    }

    if (required_permission == "access.roles.manage" || required_permission == "settings.manage" || required_permission == "settings.security.manage" || required_permission == "audit.export")
        && claims.permissions.iter().any(|p| p == "access.manage" || p == "system.manage")
    {
        return Ok(());
    }

    Err(AppError::forbidden(required_permission))
}
