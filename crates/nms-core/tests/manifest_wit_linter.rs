// Паритет с test_module_import_linter.py: линтер пакетов .nms-plugin —
// manifest.yaml из zip, ABI-версии и Ed25519-подпись перед установкой

use ed25519_dalek::{Signer, SigningKey};
use nms_core::plugin::discovery::{
    discover_plugins, load_from_archive, load_from_directory, verify_signature, PackageSource,
};
use std::io::Write;
use zip::write::SimpleFileOptions;

const MANIFEST: &str = "id: demo-module\nname: Demo\nversion: 1.2.3\n";

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nms-plugin-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_archive(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    for (name, data) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn test_zero_unpack_archive_load() {
    let dir = temp_dir();
    let archive = dir.join("demo.nms-plugin");
    write_archive(
        &archive,
        &[
            ("manifest.yaml", MANIFEST.as_bytes()),
            ("backend.wasm", b"\0asm"),
        ],
    );
    let plugin = load_from_archive(&archive).unwrap();
    assert_eq!(plugin.manifest.id, "demo-module");
    assert_eq!(plugin.manifest.version, "1.2.3");
    assert_eq!(plugin.wasm_bytes.as_deref(), Some(b"\0asm".as_slice()));
    assert!(matches!(plugin.source, PackageSource::Archive(_)));
    assert!(plugin.signature_bytes.is_none());
}

#[test]
fn test_archive_without_manifest_rejected() {
    let dir = temp_dir();
    let archive = dir.join("broken.nms-plugin");
    write_archive(&archive, &[("backend.wasm", b"\0asm")]);
    assert!(load_from_archive(&archive).is_err());
}

#[test]
fn test_dev_directory_load() {
    let dir = temp_dir();
    let plugin_dir = dir.join("demo-module");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(plugin_dir.join("manifest.yaml"), MANIFEST).unwrap();
    let plugin = load_from_directory(&plugin_dir).unwrap();
    assert_eq!(plugin.manifest.id, "demo-module");
    assert!(plugin.wasm_bytes.is_none());
    assert!(matches!(plugin.source, PackageSource::Directory(_)));
}

#[test]
fn test_discover_finds_archives_and_dirs() {
    let dir = temp_dir();
    write_archive(
        &dir.join("a.nms-plugin"),
        &[("manifest.yaml", MANIFEST.as_bytes())],
    );
    let dev_dir = dir.join("dev-module");
    std::fs::create_dir_all(&dev_dir).unwrap();
    std::fs::write(dev_dir.join("manifest.yaml"), MANIFEST).unwrap();
    // Каталог без манифеста игнорируется
    std::fs::create_dir_all(dir.join("not-a-plugin")).unwrap();
    let found = discover_plugins(&dir);
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|r| r.is_ok()));
}

#[test]
fn test_ed25519_signature_roundtrip() {
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let manifest = MANIFEST.as_bytes();
    let wasm = b"\0asm";
    let mut message = manifest.to_vec();
    message.extend_from_slice(wasm);
    let signature = key.sign(&message);

    let trusted = vec![key.verifying_key()];
    assert!(verify_signature(
        manifest,
        wasm,
        &signature.to_bytes(),
        &trusted
    ));
    // Подпись чужим ключом отклоняется
    let other = SigningKey::from_bytes(&[9u8; 32]);
    assert!(!verify_signature(
        manifest,
        wasm,
        &signature.to_bytes(),
        &[other.verifying_key()]
    ));
    // Мусорная подпись отклоняется
    assert!(!verify_signature(manifest, wasm, &[0u8; 64], &trusted));
    // Подмена содержимого после подписания отклоняется
    assert!(!verify_signature(
        b"id: evil\n",
        wasm,
        &signature.to_bytes(),
        &trusted
    ));
}

#[test]
fn test_signed_archive_signature_status() {
    let key = SigningKey::from_bytes(&[7u8; 32]);
    let manifest = MANIFEST.as_bytes();
    let wasm: &[u8] = b"\0asm";
    let mut message = manifest.to_vec();
    message.extend_from_slice(wasm);
    let signature = key.sign(&message).to_bytes();

    let dir = temp_dir();
    let archive = dir.join("signed.nms-plugin");
    write_archive(
        &archive,
        &[
            ("manifest.yaml", manifest),
            ("backend.wasm", wasm),
            ("signature.bin", &signature),
        ],
    );
    let plugin = load_from_archive(&archive).unwrap();
    assert_eq!(plugin.signature_status(&[key.verifying_key()]), Some(true));
    let other = SigningKey::from_bytes(&[9u8; 32]);
    assert_eq!(
        plugin.signature_status(&[other.verifying_key()]),
        Some(false)
    );
}
