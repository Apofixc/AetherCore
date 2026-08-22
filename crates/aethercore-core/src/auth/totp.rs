//! # Двухфакторная аутентификация (TOTP RFC 6238 и Backup Codes)
//!
//! Предоставляет криптографические функции для:
//! - Генерации Base32 секретов TOTP
//! - Формирования URL `otpauth://` и QR-кодов в формате Data URI PNG
//! - Валидации 6-значных кодов аутентификатора с окном допуска ±1 временной шаг (30 сек)
//! - Генерации, хэширования и одноразовой проверки резервных кодов восстановления (Backup Codes)

use aethercore_common::error::{AppError, Result};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use totp_rs::{Algorithm, Secret, TOTP};

/// Эмитент (название системы), отображаемый в приложениях-аутентификаторах
pub const TOTP_ISSUER: &str = "AetherCore";

/// Сгенерировать новый случайный Base32 секретный ключ для TOTP
pub fn generate_totp_secret() -> String {
    let secret = Secret::generate_secret();
    secret.to_encoded().to_string()
}

/// Создать экземпляр TOTP для указанного секрета и логина пользователя
pub fn create_totp_instance(secret_base32: &str, username: &str) -> Result<TOTP> {
    let secret = Secret::Encoded(secret_base32.trim().to_string());
    let secret_bytes = secret
        .to_bytes()
        .map_err(|e| AppError::validation("totp_secret", format!("Invalid TOTP secret: {}", e)))?;

    TOTP::new(
        Algorithm::SHA1,
        6,
        1, // Окно допуска: 1 шаг до и 1 шаг после (всего 90 секунд)
        30,
        secret_bytes,
        Some(TOTP_ISSUER.to_string()),
        username.to_string(),
    )
    .map_err(|e| AppError::internal(format!("Failed to initialize TOTP: {}", e)))
}

/// Сгенерировать Data URI QR-кода (PNG base64) для отображения в веб-интерфейсе
pub fn generate_qr_code_data_url(secret_base32: &str, username: &str) -> Result<String> {
    let totp = create_totp_instance(secret_base32, username)?;
    let qr_base64 = totp
        .get_qr_base64()
        .map_err(|e| AppError::internal(format!("Failed to generate QR code: {}", e)))?;
    Ok(format!("data:image/png;base64,{}", qr_base64))
}

/// Получить стандартный otpauth URL для добавления в приложение аутентификации
pub fn generate_otpauth_url(secret_base32: &str, username: &str) -> Result<String> {
    let totp = create_totp_instance(secret_base32, username)?;
    Ok(totp.get_url())
}

/// Проверить введенный пользователем 6-значный TOTP-код
pub fn verify_totp_code(secret_base32: &str, username: &str, code: &str) -> bool {
    let code_clean = code.trim().replace(' ', "").replace('-', "");
    if code_clean.len() != 6 || !code_clean.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    if let Ok(totp) = create_totp_instance(secret_base32, username) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        totp.check(&code_clean, now)
    } else {
        false
    }
}

/// Сгенерировать набор уникальных 8-значных резервных кодов восстановления (например, `A3B9-8K2D`)
pub fn generate_backup_codes(count: usize) -> Vec<String> {
    const CHARSET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ"; // Исключены визуально похожие 0/O, 1/I
    let mut rng = rand::thread_rng();
    let mut codes = Vec::with_capacity(count);

    for _ in 0..count {
        let part1: String = (0..4)
            .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
            .collect();
        let part2: String = (0..4)
            .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
            .collect();
        codes.push(format!("{}-{}", part1, part2));
    }

    codes
}

/// Нормализовать резервный код (удалить дефисы, пробелы и перевести в верхний регистр)
pub fn normalize_backup_code(raw: &str) -> String {
    raw.trim().replace('-', "").replace(' ', "").to_uppercase()
}

/// Захэшировать список резервных кодов и сериализовать в JSON строку
pub fn hash_and_serialize_backup_codes(raw_codes: &[String]) -> Result<String> {
    let mut hashed_codes = Vec::with_capacity(raw_codes.len());
    for code in raw_codes {
        let norm = normalize_backup_code(code);
        let hash = crate::auth::hash_password(&norm)?;
        hashed_codes.push(hash);
    }

    serde_json::to_string(&hashed_codes)
        .map_err(|e| AppError::internal(format!("Failed to serialize backup codes: {}", e)))
}

/// Проверить и однократно списать (удалить) использованный резервный код.
///
/// Если код валиден, возвращает `Some(updated_json_string)` со списком оставшихся хэшей.
/// Если код не найден или не совпал, возвращает `None`.
pub fn verify_and_consume_backup_code(
    backup_codes_json: &str,
    raw_code: &str,
) -> Option<String> {
    let hashed_codes: Vec<String> = serde_json::from_str(backup_codes_json).ok()?;
    let norm = normalize_backup_code(raw_code);
    if norm.is_empty() {
        return None;
    }

    let mut matched_index = None;
    for (idx, hash) in hashed_codes.iter().enumerate() {
        if crate::auth::verify_password(&norm, hash).unwrap_or(false) {
            matched_index = Some(idx);
            break;
        }
    }

    if let Some(idx) = matched_index {
        let mut remaining = hashed_codes;
        remaining.remove(idx);
        serde_json::to_string(&remaining).ok()
    } else {
        None
    }
}

/// Получить количество оставшихся резервных кодов
pub fn count_remaining_backup_codes(backup_codes_json: Option<&str>) -> usize {
    backup_codes_json
        .and_then(|j| serde_json::from_str::<Vec<String>>(j).ok())
        .map(|v| v.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_totp_secret_and_qr() {
        let secret = generate_totp_secret();
        assert!(!secret.is_empty());

        let qr_url = generate_qr_code_data_url(&secret, "admin").unwrap();
        assert!(qr_url.starts_with("data:image/png;base64,"));

        let otpauth = generate_otpauth_url(&secret, "admin").unwrap();
        assert!(otpauth.starts_with("otpauth://totp/AetherCore:admin"));
    }

    #[test]
    fn test_totp_verification() {
        let secret = generate_totp_secret();
        let totp = create_totp_instance(&secret, "tester").unwrap();
        let valid_code = totp.generate_current().unwrap();

        assert!(verify_totp_code(&secret, "tester", &valid_code));
        assert!(!verify_totp_code(&secret, "tester", "000000"));
        assert!(!verify_totp_code(&secret, "tester", "invalid"));
    }

    #[test]
    fn test_backup_codes_lifecycle() {
        let raw_codes = generate_backup_codes(8);
        assert_eq!(raw_codes.len(), 8);

        let json = hash_and_serialize_backup_codes(&raw_codes).unwrap();
        assert_eq!(count_remaining_backup_codes(Some(&json)), 8);

        // Успешное списание первого кода
        let used_code = &raw_codes[0];
        let updated_json = verify_and_consume_backup_code(&json, used_code).unwrap();
        assert_eq!(count_remaining_backup_codes(Some(&updated_json)), 7);

        // Повторное списание того же кода должно провалиться
        let fail = verify_and_consume_backup_code(&updated_json, used_code);
        assert!(fail.is_none());

        // Неверный код
        let invalid = verify_and_consume_backup_code(&updated_json, "XXXX-YYYY");
        assert!(invalid.is_none());
    }
}
