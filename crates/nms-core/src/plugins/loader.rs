//! # Загрузчик и упаковщик пакетов плагинов (.nms-plugin / ZIP)
//!
//! Обеспечивает Zero-Unpack загрузку манифеста, WASM-байткода, локалей и статических
//! ассетов интерфейса напрямую из архива в память без распаковки на диск сервера,
//! а также поддержку директорий для локальной разработки (--dev).

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use nms_common::error::{AppError, Result};
use nms_common::manifest::ModuleManifest;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// Представление загруженного в память пакета плагина
#[derive(Debug, Clone)]
pub struct PluginPackage {
    /// Манифест плагина
    pub manifest: ModuleManifest,
    /// Сырой YAML манифеста для верификации подписи
    pub manifest_raw: Vec<u8>,
    /// WASM-байткод (backend.wasm)
    pub backend_wasm: Option<Vec<u8>>,
    /// Цифровая подпись пакета (signature.bin)
    pub signature: Option<Vec<u8>>,
    /// Локали плагина: "ru" -> JSON строка, "en" -> JSON строка
    pub locales: HashMap<String, String>,
    /// Файлы фронтенда (относительный путь -> байты)
    pub frontend_assets: HashMap<String, Vec<u8>>,
}

impl PluginPackage {
    /// Загрузить пакет плагина из ZIP архива (.nms-plugin) напрямую в память
    pub fn from_zip_bytes(bytes: &[u8]) -> Result<Self> {
        let reader = Cursor::new(bytes);
        let mut zip = ZipArchive::new(reader).map_err(|e| AppError::Validation {
            field: "plugin_archive".into(),
            details: format!("Failed to read ZIP archive: {}", e),
        })?;

        let mut manifest_raw: Option<Vec<u8>> = None;
        let mut backend_wasm: Option<Vec<u8>> = None;
        let mut signature: Option<Vec<u8>> = None;
        let mut locales = HashMap::new();
        let mut frontend_assets = HashMap::new();

        for i in 0..zip.len() {
            let mut file = zip.by_index(i).map_err(|e| AppError::Validation {
                field: "zip_entry".into(),
                details: e.to_string(),
            })?;

            if file.is_dir() {
                continue;
            }

            let file_name = file.name().to_string();
            let mut buf = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buf).map_err(|e| AppError::Internal {
                details: format!("Failed to read ZIP entry '{}': {}", file_name, e),
            })?;

            if file_name == "manifest.yaml" || file_name == "manifest.yml" {
                manifest_raw = Some(buf);
            } else if file_name == "backend.wasm" {
                backend_wasm = Some(buf);
            } else if file_name == "signature.bin" {
                signature = Some(buf);
            } else if file_name.starts_with("locales/") && file_name.ends_with(".json") {
                if let Some(locale_name) = file_name
                    .strip_prefix("locales/")
                    .and_then(|s| s.strip_suffix(".json"))
                {
                    if let Ok(json_str) = String::from_utf8(buf) {
                        locales.insert(locale_name.to_string(), json_str);
                    }
                }
            } else if file_name.starts_with("frontend/") {
                frontend_assets.insert(file_name, buf);
            }
        }

        let raw_manifest = manifest_raw.ok_or_else(|| AppError::Validation {
            field: "manifest.yaml".into(),
            details: "Plugin package is missing manifest.yaml".into(),
        })?;

        let manifest_str = std::str::from_utf8(&raw_manifest).map_err(|e| AppError::Validation {
            field: "manifest.yaml".into(),
            details: format!("Manifest is not valid UTF-8: {}", e),
        })?;

        let manifest = ModuleManifest::from_yaml(manifest_str)?;

        Ok(Self {
            manifest,
            manifest_raw: raw_manifest,
            backend_wasm,
            signature,
            locales,
            frontend_assets,
        })
    }

    /// Загрузить плагин из локальной директории (режим разработки --dev)
    pub fn from_directory(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join("manifest.yaml");
        if !manifest_path.exists() {
            return Err(AppError::NotFound {
                resource: format!("manifest.yaml in {:?}", dir),
            });
        }

        let manifest_raw = std::fs::read(&manifest_path).map_err(|e| AppError::Internal {
            details: format!("Failed to read {:?}: {}", manifest_path, e),
        })?;

        let manifest_str = std::str::from_utf8(&manifest_raw).map_err(|e| AppError::Validation {
            field: "manifest.yaml".into(),
            details: e.to_string(),
        })?;

        let manifest = ModuleManifest::from_yaml(manifest_str)?;

        let wasm_path = dir.join("backend.wasm");
        let backend_wasm = if wasm_path.exists() {
            Some(std::fs::read(&wasm_path).map_err(|e| AppError::Internal {
                details: format!("Failed to read {:?}: {}", wasm_path, e),
            })?)
        } else {
            None
        };

        let sig_path = dir.join("signature.bin");
        let signature = if sig_path.exists() {
            Some(std::fs::read(&sig_path).unwrap_or_default())
        } else {
            None
        };

        // Загрузка локалей
        let mut locales = HashMap::new();
        let locales_dir = dir.join("locales");
        if locales_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&locales_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                locales.insert(stem.to_string(), content);
                            }
                        }
                    }
                }
            }
        }

        // Загрузка фронтенд-ассетов
        let mut frontend_assets = HashMap::new();
        let frontend_dir = dir.join("frontend");
        if frontend_dir.is_dir() {
            load_directory_recursive(&frontend_dir, "frontend", &mut frontend_assets)?;
        }

        Ok(Self {
            manifest,
            manifest_raw,
            backend_wasm,
            signature,
            locales,
            frontend_assets,
        })
    }

    /// Проверить цифровую Ed25519 подпись пакета (манифест + байткод)
    pub fn verify_signature(&self, public_key_bytes: &[u8; 32]) -> Result<bool> {
        let sig_bytes = match &self.signature {
            Some(s) if s.len() == 64 => s,
            _ => return Ok(false),
        };

        let verifying_key = VerifyingKey::from_bytes(public_key_bytes).map_err(|e| {
            AppError::Validation {
                field: "public_key".into(),
                details: format!("Invalid public key: {}", e),
            }
        })?;

        let signature = Signature::from_slice(sig_bytes).map_err(|e| AppError::Validation {
            field: "signature".into(),
            details: format!("Invalid signature format: {}", e),
        })?;

        // Подписываемые данные: manifest_raw + backend_wasm (если есть)
        let mut signed_data = self.manifest_raw.clone();
        if let Some(wasm) = &self.backend_wasm {
            signed_data.extend_from_slice(wasm);
        }

        Ok(verifying_key.verify(&signed_data, &signature).is_ok())
    }

    /// Упаковать плагин из директории в .nms-plugin (ZIP) архив с опциональной подписью
    pub fn pack(dir: &Path, signing_key: Option<&SigningKey>) -> Result<Vec<u8>> {
        let package = Self::from_directory(dir)?;

        let mut zip_buf = Vec::new();
        let mut zip = ZipWriter::new(Cursor::new(&mut zip_buf));
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 1. Записываем manifest.yaml
        zip.start_file("manifest.yaml", options)
            .map_err(|e| AppError::Internal {
                details: e.to_string(),
            })?;
        zip.write_all(&package.manifest_raw)
            .map_err(|e| AppError::Internal {
                details: e.to_string(),
            })?;

        // 2. Записываем backend.wasm
        if let Some(wasm) = &package.backend_wasm {
            zip.start_file("backend.wasm", options)
                .map_err(|e| AppError::Internal {
                    details: e.to_string(),
                })?;
            zip.write_all(wasm).map_err(|e| AppError::Internal {
                details: e.to_string(),
            })?;
        }

        // 3. Записываем цифровую подпись (если передан ключ)
        if let Some(signer) = signing_key {
            let mut signed_data = package.manifest_raw.clone();
            if let Some(wasm) = &package.backend_wasm {
                signed_data.extend_from_slice(wasm);
            }
            let sig = signer.sign(&signed_data);
            zip.start_file("signature.bin", options)
                .map_err(|e| AppError::Internal {
                    details: e.to_string(),
                })?;
            zip.write_all(&sig.to_bytes())
                .map_err(|e| AppError::Internal {
                    details: e.to_string(),
                })?;
        }

        // 4. Записываем локали
        for (lang, json) in &package.locales {
            let entry_name = format!("locales/{}.json", lang);
            zip.start_file(entry_name, options)
                .map_err(|e| AppError::Internal {
                    details: e.to_string(),
                })?;
            zip.write_all(json.as_bytes())
                .map_err(|e| AppError::Internal {
                    details: e.to_string(),
                })?;
        }

        // 5. Записываем фронтенд файлы
        for (rel_path, data) in &package.frontend_assets {
            zip.start_file(rel_path, options)
                .map_err(|e| AppError::Internal {
                    details: e.to_string(),
                })?;
            zip.write_all(data).map_err(|e| AppError::Internal {
                details: e.to_string(),
            })?;
        }

        zip.finish().map_err(|e| AppError::Internal {
            details: format!("Failed to finish ZIP archive: {}", e),
        })?;

        Ok(zip_buf)
    }
}

/// Рекурсивное чтение директории для фронтенд-ассетов
fn load_directory_recursive(
    dir: &Path,
    prefix: &str,
    map: &mut HashMap<String, Vec<u8>>,
) -> Result<()> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let current_rel = format!("{}/{}", prefix, name);

            if path.is_dir() {
                load_directory_recursive(&path, &current_rel, map)?;
            } else if path.is_file() {
                if let Ok(bytes) = std::fs::read(&path) {
                    map.insert(current_rel, bytes);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;
    use tempfile::tempdir;

    #[test]
    fn test_plugin_pack_and_verify_signature() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        // Создаем manifest.yaml
        let manifest_content = r#"
id: "test-plugin"
name: "Test Plugin"
version: "1.0.0"
description: "Test plugin description"
"#;
        std::fs::write(dir_path.join("manifest.yaml"), manifest_content).unwrap();

        // Создаем тестовый backend.wasm (фиктивные байты)
        let dummy_wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        std::fs::write(dir_path.join("backend.wasm"), &dummy_wasm).unwrap();

        // Создаем локаль
        let locales_dir = dir_path.join("locales");
        std::fs::create_dir_all(&locales_dir).unwrap();
        std::fs::write(locales_dir.join("ru.json"), r#"{"hello": "Привет"}"#).unwrap();

        // Генерируем Ed25519 ключи для подписи
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        // Упаковываем в ZIP
        let zip_bytes = PluginPackage::pack(dir_path, Some(&signing_key)).unwrap();

        // Читаем из ZIP напрямую
        let package = PluginPackage::from_zip_bytes(&zip_bytes).unwrap();
        assert_eq!(package.manifest.id, "test-plugin");
        assert_eq!(package.backend_wasm.as_ref().unwrap(), &dummy_wasm);
        assert_eq!(
            package.locales.get("ru").unwrap(),
            r#"{"hello": "Привет"}"#
        );

        // Проверяем валидность подписи
        assert!(package.verify_signature(&verifying_key.to_bytes()).unwrap());

        // Проверяем с неверным ключом
        let another_key = SigningKey::generate(&mut csprng);
        assert!(!package
            .verify_signature(&another_key.verifying_key().to_bytes())
            .unwrap());
    }
}
