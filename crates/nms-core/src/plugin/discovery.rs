// Обнаружение и линтинг пакетов плагинов (.nms-plugin / dev-каталоги)
// Спецификация: MIGRATION_RUST_WASM.md, раздел 1.2.А (Zero-Unpack) и 1.2.В (Discovery & Validation)

use super::manifest::ModuleManifest;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use zip::ZipArchive;

/// Источник пакета: единый zip-архив или распакованный dev-каталог
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSource {
    /// Единый архив .nms-plugin / .zip (Zero-Unpack)
    Archive(PathBuf),
    /// Локальный распакованный каталог (режим разработки)
    Directory(PathBuf),
}

/// Обнаруженный пакет плагина с разобранным манифестом
#[derive(Debug, Clone)]
pub struct DiscoveredPlugin {
    pub source: PackageSource,
    pub manifest: ModuleManifest,
    /// Сырые байты backend.wasm (если присутствует в пакете)
    pub wasm_bytes: Option<Vec<u8>>,
    /// Сырые байты manifest.yaml (для проверки подписи)
    pub manifest_bytes: Vec<u8>,
    /// Сырые байты signature.bin (None = пакет не подписан)
    pub signature_bytes: Option<Vec<u8>>,
}

impl DiscoveredPlugin {
    /// Проверка Ed25519-подписи пакета по доверенным ключам (None = подпись отсутствует)
    pub fn signature_status(&self, trusted_keys: &[VerifyingKey]) -> Option<bool> {
        let signature = self.signature_bytes.as_deref()?;
        let wasm = self.wasm_bytes.as_deref().unwrap_or(&[]);
        Some(verify_signature(
            &self.manifest_bytes,
            wasm,
            signature,
            trusted_keys,
        ))
    }
}

/// Ошибка обнаружения/линтинга пакета плагина
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("io error while reading '{0}': {1}")]
    Io(PathBuf, String),
    #[error("package '{0}' does not contain manifest.yaml")]
    MissingManifest(PathBuf),
    #[error("manifest parse error in '{0}': {1}")]
    ManifestParse(PathBuf, String),
    #[error("unsigned plugin '{0}' rejected: allow_unsigned_plugins = false")]
    UnsignedRejected(String),
    #[error("invalid Ed25519 signature for plugin '{0}'")]
    InvalidSignature(String),
}

/// Чтение файла из zip-архива в память (без распаковки на диск)
fn read_zip_entry(archive: &mut ZipArchive<fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name).ok()?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// Загрузка пакета плагина из единого zip-архива (.nms-plugin) напрямую в память
pub fn load_from_archive(path: &Path) -> Result<DiscoveredPlugin, DiscoveryError> {
    let file =
        fs::File::open(path).map_err(|e| DiscoveryError::Io(path.to_path_buf(), e.to_string()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| DiscoveryError::Io(path.to_path_buf(), e.to_string()))?;

    // manifest.yaml читается парсером напрямую из zip-потока в память
    let manifest_bytes = read_zip_entry(&mut archive, "manifest.yaml")
        .ok_or_else(|| DiscoveryError::MissingManifest(path.to_path_buf()))?;
    let manifest_text = String::from_utf8_lossy(&manifest_bytes).to_string();
    let manifest = ModuleManifest::from_yaml(&manifest_text)
        .map_err(|e| DiscoveryError::ManifestParse(path.to_path_buf(), e.to_string()))?;

    let wasm_bytes = read_zip_entry(&mut archive, "backend.wasm");
    let signature_bytes = read_zip_entry(&mut archive, "signature.bin");

    Ok(DiscoveredPlugin {
        source: PackageSource::Archive(path.to_path_buf()),
        manifest,
        wasm_bytes,
        manifest_bytes,
        signature_bytes,
    })
}

/// Загрузка пакета плагина из распакованного dev-каталога
pub fn load_from_directory(dir: &Path) -> Result<DiscoveredPlugin, DiscoveryError> {
    let manifest_path = dir.join("manifest.yaml");
    let manifest_text = fs::read_to_string(&manifest_path)
        .map_err(|_| DiscoveryError::MissingManifest(dir.to_path_buf()))?;
    let manifest = ModuleManifest::from_yaml(&manifest_text)
        .map_err(|e| DiscoveryError::ManifestParse(dir.to_path_buf(), e.to_string()))?;

    let wasm_bytes = fs::read(dir.join("backend.wasm")).ok();
    let signature_bytes = fs::read(dir.join("signature.bin")).ok();

    Ok(DiscoveredPlugin {
        source: PackageSource::Directory(dir.to_path_buf()),
        manifest,
        wasm_bytes,
        manifest_bytes: manifest_text.into_bytes(),
        signature_bytes,
    })
}

/// Проверка Ed25519-подписи пакета: подпись покрывает конкатенацию manifest.yaml + backend.wasm
pub fn verify_signature(
    manifest_bytes: &[u8],
    wasm_bytes: &[u8],
    signature_bytes: &[u8],
    trusted_keys: &[VerifyingKey],
) -> bool {
    let Ok(sig_array) = <&[u8; 64]>::try_from(signature_bytes) else {
        return false;
    };
    let signature = Signature::from_bytes(sig_array);
    // Подписываемое сообщение: манифест, затем байткод WASM
    let mut message = Vec::with_capacity(manifest_bytes.len() + wasm_bytes.len());
    message.extend_from_slice(manifest_bytes);
    message.extend_from_slice(wasm_bytes);
    trusted_keys
        .iter()
        .any(|key| key.verify(&message, &signature).is_ok())
}

/// Сканирование каталога modules/ на предмет архивов .nms-plugin/.zip и dev-каталогов
pub fn discover_plugins(modules_dir: &Path) -> Vec<Result<DiscoveredPlugin, DiscoveryError>> {
    let mut results = Vec::new();
    let entries = match fs::read_dir(modules_dir) {
        Ok(e) => e,
        Err(e) => {
            warn!(
                "Modules directory '{}' is not readable: {}",
                modules_dir.display(),
                e
            );
            return results;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Dev-режим: распакованный каталог плагина с manifest.yaml
            if path.join("manifest.yaml").exists() {
                results.push(load_from_directory(&path));
            }
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == "nms-plugin" || ext == "zip" {
                results.push(load_from_archive(&path));
            }
        }
    }

    info!(
        "Plugin discovery finished: {} package(s) found in '{}'",
        results.len(),
        modules_dir.display()
    );
    results
}
