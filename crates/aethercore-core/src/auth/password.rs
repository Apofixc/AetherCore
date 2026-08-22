//! # Хэширование и валидация паролей (Argon2id)
//!
//! Модуль использует алгоритм Argon2id (победитель Password Hashing Competition)
//! с автоматической генерацией криптографически стойкой соли через [`OsRng`].

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordVerifier, PasswordHasher};
use aethercore_common::error::{AppError, Result};

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
/// Возвращает [`AppError::Internal`](aethercore_common::error::AppError), если генерация хэша
/// завершилась ошибкой библиотеки Argon2.
///
/// # Примеры
/// ```rust,no_run
/// use aethercore_core::auth::hash_password;
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
/// Возвращает [`AppError::Internal`](aethercore_common::error::AppError), если строка
/// `password_hash` имеет неверный формат или повреждена.
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|e| {
        AppError::internal(format!("Invalid password hash format: {}", e))
    })?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Проверить соответствие пароля политике сложности
///
/// # Аргументы
/// * `password` — Пароль в открытом виде
/// * `min_length` — Минимальная длина пароля
/// * `require_uppercase` — Требовать ли заглавные буквы [A-Z, А-Я]
/// * `require_digits` — Требовать ли цифры [0-9]
/// * `require_special` — Требовать ли спецсимволы (!@#$%^&*...)
///
/// # Ошибки
/// Возвращает [`AppError::validation`], если пароль не удовлетворяет требованиям.
pub fn validate_password_complexity(
    password: &str,
    min_length: u32,
    require_uppercase: bool,
    require_digits: bool,
    require_special: bool,
) -> Result<()> {
    let char_count = password.chars().count();
    if char_count < min_length as usize {
        return Err(AppError::validation(
            "password",
            format!("Password length must be at least {} characters", min_length),
        ));
    }

    if require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
        return Err(AppError::validation(
            "password",
            "Password must contain at least one uppercase letter",
        ));
    }

    if require_digits && !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(AppError::validation(
            "password",
            "Password must contain at least one numeric digit",
        ));
    }

    if require_special && !password.chars().any(|c| !c.is_alphanumeric()) {
        return Err(AppError::validation(
            "password",
            "Password must contain at least one special character",
        ));
    }

    Ok(())
}

