//! # Загрузчик и упаковщик пакетов плагинов (.aether-plugin / ZIP)
//!
//! Обеспечивает Zero-Unpack загрузку манифеста, WASM-байткода, локалей и статических
//! ассетов интерфейса напрямую из архива в память без распаковки на диск сервера,
//! а также поддержку директорий для локальной разработки (--dev).

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use aethercore_common::error::{AppError, Result};
use aethercore_common::manifest::ModuleManifest;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// Пакет плагина, загруженный в оперативную память
///
/// Содержит все компоненты модуля:
/// - [`ModuleManifest`] — метаданные, зависимости, точки интеграции и JSON-схему настроек.
/// - Байткод WebAssembly для бэкенда (`backend.wasm`).
/// - Цифровую подпись Ed25519 (`signature.bin`).
/// - Карту словарей локализации (`locales/<lang>.json`).
/// - Встроенные статические файлы веб-интерфейса (`frontend/*`).
#[derive(Debug, Clone)]
pub struct PluginPackage {
    /// Разобранный манифест плагина
    pub manifest: ModuleManifest,
    /// Сырой YAML манифеста для криптографической верификации подписи
    pub manifest_raw: Vec<u8>,
    /// Опциональный WASM-байткод (backend.wasm)
    pub backend_wasm: Option<Vec<u8>>,
    /// Опциональная цифровая подпись пакета (signature.bin, 64 байта)
    pub signature: Option<Vec<u8>>,
    /// Локали плагина: "ru" -> JSON строка, "en" -> JSON строка
    pub locales: HashMap<String, String>,
    /// Файлы фронтенда (относительный путь -> бинарное содержимое)
    pub frontend_assets: HashMap<String, Vec<u8>>,
}

impl PluginPackage {
    /// Загрузить пакет плагина из ZIP архива (`.nms-plugin`) напрямую в оперативную память (Zero-Unpack)
    ///
    /// # Аргументы
    /// * `bytes` — Байтовый срез содержимого ZIP-файла.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Validation`](aethercore_common::error::AppError), если архив поврежден,
    /// отсутствует обязательный файл `manifest.yaml` или манифест содержит синтаксические ошибки.
    pub fn from_zip_bytes(bytes: &[u8]) -> Result<Self> {
        let reader = Cursor::new(bytes);
        let mut zip = ZipArchive::new(reader).map_err(|e| {
            AppError::validation("plugin_archive", format!("Failed to read ZIP archive: {}", e))
        })?;

        let mut manifest_raw: Option<Vec<u8>> = None;
        let mut backend_wasm: Option<Vec<u8>> = None;
        let mut signature: Option<Vec<u8>> = None;
        let mut locales = HashMap::new();
        let mut frontend_assets = HashMap::new();

        for i in 0..zip.len() {
            let mut file = zip.by_index(i).map_err(|e| {
                AppError::validation("zip_entry", e.to_string())
            })?;

            if file.is_dir() {
                continue;
            }

            let file_name = file.name().to_string();
            let mut buf = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut buf).map_err(|e| {
                AppError::internal(format!("Failed to read ZIP entry '{}': {}", file_name, e))
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

        let raw_manifest = manifest_raw.ok_or_else(|| {
            AppError::validation("manifest.yaml", "Plugin package is missing manifest.yaml")
        })?;

        let manifest_str = std::str::from_utf8(&raw_manifest).map_err(|e| {
            AppError::validation("manifest.yaml", format!("Manifest is not valid UTF-8: {}", e))
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

    /// Загрузить плагин из локальной распакованной директории (режим локальной разработки `--dev`)
    ///
    /// # Аргументы
    /// * `dir` — Путь к директории плагина на диске.
    ///
    /// # Возвращаемое значение
    /// Экземпляр [`PluginPackage`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::NotFound`](aethercore_common::error::AppError), если файл `manifest.yaml` отсутствует,
    /// или [`AppError::Validation`](aethercore_common::error::AppError) при синтаксических ошибках в манифесте.
    pub fn from_directory(dir: &Path) -> Result<Self> {
        let manifest_path = dir.join("manifest.yaml");
        if !manifest_path.exists() {
            return Err(AppError::not_found(format!("manifest.yaml in {:?}", dir)));
        }

        let manifest_raw = std::fs::read(&manifest_path).map_err(|e| {
            AppError::internal(format!("Failed to read {:?}: {}", manifest_path, e))
        })?;

        let manifest_str = std::str::from_utf8(&manifest_raw).map_err(|e| {
            AppError::validation("manifest.yaml", e.to_string())
        })?;

        let manifest = ModuleManifest::from_yaml(manifest_str)?;

        let wasm_path = dir.join("backend.wasm");
        let backend_wasm = if wasm_path.exists() {
            Some(std::fs::read(&wasm_path).map_err(|e| {
                AppError::internal(format!("Failed to read {:?}: {}", wasm_path, e))
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

    /// Проверить криптографическую цифровую подпись Ed25519 для манифеста и Wasm-байткода
    ///
    /// Подпись рассчитывается от конкатенации сырого манифеста и байткода `backend.wasm`.
    ///
    /// # Аргументы
    /// * `public_key_bytes` — 32-байтный открытый ключ доверенного издателя.
    ///
    /// # Возвращаемое значение
    /// `Ok(true)`, если цифровая подпись корректна, `Ok(false)` если подпись отсутствует или не совпала.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Validation`](aethercore_common::error::AppError) при неверном формате ключа или подписи.
    pub fn verify_signature(&self, public_key_bytes: &[u8; 32]) -> Result<bool> {
        let sig_bytes = match &self.signature {
            Some(s) if s.len() == 64 => s,
            _ => return Ok(false),
        };

        let verifying_key = VerifyingKey::from_bytes(public_key_bytes).map_err(|e| {
            AppError::validation("public_key", format!("Invalid public key: {}", e))
        })?;

        let signature = Signature::from_slice(sig_bytes).map_err(|e| {
            AppError::validation("signature", format!("Invalid signature format: {}", e))
        })?;

        // Подписываемые данные: manifest_raw + backend_wasm (если есть)
        let mut signed_data = self.manifest_raw.clone();
        if let Some(wasm) = &self.backend_wasm {
            signed_data.extend_from_slice(wasm);
        }

        Ok(verifying_key.verify(&signed_data, &signature).is_ok())
    }

    /// Упаковать плагин из директории в `.aether-plugin` (ZIP) архив с опциональной цифровой подписью Ed25519
    ///
    /// # Аргументы
    /// * `dir` — Путь к исходной папке плагина.
    /// * `signing_key` — Опциональный секретный ключ подписи [`SigningKey`].
    ///
    /// # Возвращаемое значение
    /// Байтовый буфер упакованного архива.
    ///
    /// # Ошибки
    /// Возвращает [`AppError`] при сбое чтения исходных файлов или формирования архива.
    pub fn pack(dir: &Path, signing_key: Option<&SigningKey>) -> Result<Vec<u8>> {
        let package = Self::from_directory(dir)?;

        let mut zip_buf = Vec::new();
        let mut zip = ZipWriter::new(Cursor::new(&mut zip_buf));
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 1. Записываем manifest.yaml
        zip.start_file("manifest.yaml", options)
            .map_err(|e| AppError::internal(e.to_string()))?;
        zip.write_all(&package.manifest_raw)
            .map_err(|e| AppError::internal(e.to_string()))?;

        // 2. Записываем backend.wasm
        if let Some(wasm) = &package.backend_wasm {
            zip.start_file("backend.wasm", options)
                .map_err(|e| AppError::internal(e.to_string()))?;
            zip.write_all(wasm)
                .map_err(|e| AppError::internal(e.to_string()))?;
        }

        // 3. Записываем цифровую подпись (если передан ключ)
        if let Some(signer) = signing_key {
            let mut signed_data = package.manifest_raw.clone();
            if let Some(wasm) = &package.backend_wasm {
                signed_data.extend_from_slice(wasm);
            }
            let sig = signer.sign(&signed_data);
            zip.start_file("signature.bin", options)
                .map_err(|e| AppError::internal(e.to_string()))?;
            zip.write_all(&sig.to_bytes())
                .map_err(|e| AppError::internal(e.to_string()))?;
        }

        // 4. Записываем локали
        for (lang, json) in &package.locales {
            let entry_name = format!("locales/{}.json", lang);
            zip.start_file(entry_name, options)
                .map_err(|e| AppError::internal(e.to_string()))?;
            zip.write_all(json.as_bytes())
                .map_err(|e| AppError::internal(e.to_string()))?;
        }

        // 5. Записываем фронтенд файлы
        for (rel_path, data) in &package.frontend_assets {
            zip.start_file(rel_path, options)
                .map_err(|e| AppError::internal(e.to_string()))?;
            zip.write_all(data)
                .map_err(|e| AppError::internal(e.to_string()))?;
        }

        zip.finish()
            .map_err(|e| AppError::internal(format!("Failed to finish ZIP archive: {}", e)))?;

        Ok(zip_buf)
    }
}

/// Рекурсивно сканировать и загрузить все файлы из директории статических веб-ассетов
///
/// # Аргументы
/// * `dir` — Путь к сканируемой папке.
/// * `prefix` — Префикс пути для формирования ключей в словаре (например, `"frontend"`).
/// * `map` — Результирующая хэш-таблица `путь -> байтовое_содержимое`.
///
/// # Ошибки
/// Возвращает [`AppError::Internal`](aethercore_common::error::AppError) при сбое чтения файла с диска.
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
