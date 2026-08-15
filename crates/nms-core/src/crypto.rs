// Модуль шифрования чувствительных данных at-rest (AES-256-GCM с HKDF-SHA256)
// Соответствует алгоритму и формату enc:v1: из Python-версии NMS

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hkdf::Hkdf;
use sha2::Sha256;

const PREFIX: &str = "enc:v1:";
const SALT: &[u8] = b"nms-webui-at-rest-salt";
const INFO: &[u8] = b"secret-encryption-v1";

/// Вывести 256-битный ключ шифрования из secret_key с помощью HKDF-SHA256 (соответствует _get_aes_key в Python)
fn _get_aes_key(secret_key: &str) -> Result<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(Some(SALT), secret_key.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(INFO, &mut okm)
        .map_err(|_| anyhow!("HKDF key derivation failed"))?;
    Ok(okm)
}

/// Зашифровать чувствительную строку в формат enc:v1:<base64(12-byte nonce + ciphertext)>
pub fn encrypt_secret(plain_text: Option<&str>, secret_key: &str) -> Result<Option<String>> {
    let text = match plain_text {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(plain_text.map(String::from)),
    };

    if text.starts_with(PREFIX) {
        return Ok(Some(text.to_string()));
    }

    let key_bytes = _get_aes_key(secret_key)?;
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .map_err(|e| anyhow!("Failed to initialize AES-GCM: {}", e))?;

    // Генерация 12-байтного случайно криптографического nonce
    let mut nonce_bytes = [0u8; 12];
    getrandom::getrandom(&mut nonce_bytes)
        .map_err(|e| anyhow!("Failed to generate random nonce: {}", e))?;

    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, text.as_bytes())
        .map_err(|e| anyhow!("AES-GCM encryption failed: {}", e))?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);

    let encoded = BASE64.encode(combined);
    Ok(Some(format!("{}{}", PREFIX, encoded)))
}

/// Расшифровать данные формата enc:v1:... С открытым фолбэком для старых строк
pub fn decrypt_secret(cipher_text: Option<&str>, secret_key: &str) -> Result<Option<String>> {
    let text = match cipher_text {
        Some(t) if !t.is_empty() => t,
        _ => return Ok(cipher_text.map(String::from)),
    };

    if !text.starts_with(PREFIX) {
        return Ok(Some(text.to_string()));
    }

    let raw_b64 = &text[PREFIX.len()..];
    let decoded: Vec<u8> = match BASE64.decode(raw_b64) {
        Ok(d) => d,
        Err(_) => return Ok(Some(text.to_string())),
    };

    if decoded.len() < 12 {
        return Ok(Some(text.to_string()));
    }

    let (nonce_bytes, ciphertext) = decoded.split_at(12);
    let key_bytes = _get_aes_key(secret_key)?;

    let cipher = match Aes256Gcm::new_from_slice(&key_bytes) {
        Ok(c) => c,
        Err(_) => return Ok(Some(text.to_string())),
    };

    let nonce = Nonce::from_slice(nonce_bytes);
    match cipher.decrypt(nonce, ciphertext) {
        Ok(plain_bytes) => Ok(Some(String::from_utf8_lossy(&plain_bytes).to_string())),
        Err(_) => Ok(Some(text.to_string())),
    }
}

/// Маскирование секрета для отдачи через REST API (замена на ***)
pub fn mask_secret(secret_val: Option<&str>) -> Option<String> {
    secret_val.and_then(|val| {
        if val.is_empty() {
            None
        } else {
            Some("***".to_string())
        }
    })
}
