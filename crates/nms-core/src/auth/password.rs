//! # Хэширование и валидация паролей (Argon2id)
//!
//! Модуль использует алгоритм Argon2id (победитель Password Hashing Competition)
//! с автоматической генерацией криптографически стойкой соли через [`OsRng`].

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordVerifier, PasswordHasher};
use nms_common::error::{AppError, Result};

/// Захэшировать открытый пароль с использованием алгоритма Argon2id
///
/// Для каждого вызова генерируется уникальная криптографическая соль через системный CSPRNG.
///
/// # Аргументы
/// * `password` — Пароль в виде открытого текста UTF-8.
///
/// # Возвращаемое значение
/// Возвращает строку в стандартном формате PHC (Password Hashing Competition),
/// включающую параметры алгоритма, соль и хэш.
///
/// # Ошибки
/// Возвращает [`AppError::Internal`](nms_common::error::AppError), если генерация хэша
/// завершилась ошибкой библиотеки Argon2.
///
/// # Примеры
/// ```rust,no_run
/// use nms_core::auth::hash_password;
///
/// let hash = hash_password("my_secret_pass").unwrap();
/// assert!(hash.starts_with("$argon2id$"));
/// ```
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::internal(format!("Password hashing failed: {}", e)))
}

/// Проверить соответствие открытого пароля сохраненному Argon2 хэшу
///
/// # Аргументы
/// * `password` — Пароль в виде открытого текста для проверки.
/// * `password_hash` — Ранее сохраненная PHC-строка хэша.
///
/// # Возвращаемое значение
/// Возвращает `Ok(true)`, если пароль верен, и `Ok(false)` при несовпадении.
///
/// # Ошибки
/// Возвращает [`AppError::Internal`](nms_common::error::AppError), если строка
/// `password_hash` имеет неверный формат или повреждена.
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|e| {
        AppError::internal(format!("Invalid password hash format: {}", e))
    })?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}
