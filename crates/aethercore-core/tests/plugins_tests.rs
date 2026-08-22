//! # Тесты подсистемы загрузки и управления плагинами

use ed25519_dalek::SigningKey;
use nms_common::i18n::{global, Locale};
use nms_common::manifest::ModuleManifest;
use nms_core::bus::EventBus;
use nms_core::db::Db;
use nms_core::plugins::loader::PluginPackage;
use nms_core::plugins::PluginManager;
use rand::rngs::OsRng;
use std::collections::HashMap;
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

    // Создаем тестовый backend.wasm
    let dummy_wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    std::fs::write(dir_path.join("backend.wasm"), &dummy_wasm).unwrap();

    // Создаем локаль
    let locales_dir = dir_path.join("locales");
    std::fs::create_dir_all(&locales_dir).unwrap();
    std::fs::write(locales_dir.join("ru.json"), r#"{"hello": "Привет"}"#).unwrap();

    // Генерируем Ed25519 ключи
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    // Упаковываем в ZIP
    let zip_bytes = PluginPackage::pack(dir_path, Some(&signing_key)).unwrap();

    // Читаем из ZIP напрямую
    let package = PluginPackage::from_zip_bytes(&zip_bytes).unwrap();
    assert_eq!(package.manifest.id, "test-plugin");
    assert_eq!(package.backend_wasm.as_ref().unwrap(), &dummy_wasm);
    assert_eq!(package.locales.get("ru").unwrap(), r#"{"hello": "Привет"}"#);

    // Проверяем валидность подписи
    assert!(package.verify_signature(&verifying_key.to_bytes()).unwrap());

    // Проверяем с неверным ключом
    let another_key = SigningKey::generate(&mut csprng);
    assert!(!package
        .verify_signature(&another_key.verifying_key().to_bytes())
        .unwrap());
}

#[tokio::test]
async fn test_plugin_manager_lifecycle() {
    let db = Db::init_in_memory().await.unwrap();
    let bus = EventBus::new(db.clone());
    let manager = PluginManager::new(db, bus);

    let manifest = ModuleManifest::from_yaml(
        r#"
id: "demo-plugin"
name: "Demo Plugin"
version: "1.0.0"
description: "Demo"
config_schema:
  type: "object"
  required: ["interval"]
  properties:
    interval:
      type: "integer"
      minimum: 1
"#,
    )
    .unwrap();

    let mut locales = HashMap::new();
    locales.insert("ru".into(), r#"{"status": "Работает"}"#.into());

    let mut frontend_assets = HashMap::new();
    frontend_assets.insert("frontend/dist/ui.js".into(), b"console.log('hi');".to_vec());

    let package = PluginPackage {
        manifest,
        manifest_raw: vec![],
        backend_wasm: None,
        signature: None,
        locales,
        frontend_assets,
    };

    manager.register_plugin(package).await.unwrap();

    // Проверяем i18n
    assert_eq!(
        global().translate(Locale::Ru, "demo-plugin.status", &[]),
        "Работает"
    );

    // Проверяем фронтенд ассет
    let asset = manager.get_frontend_asset("demo-plugin", "dist/ui.js");
    assert_eq!(asset, Some(b"console.log('hi');".to_vec()));

    // Проверяем валидацию и сохранение настроек
    let valid_config = serde_json::json!({"interval": 5});
    assert!(manager
        .set_plugin_config("demo-plugin", &valid_config)
        .await
        .is_ok());

    let invalid_config = serde_json::json!({"interval": 0});
    assert!(manager
        .set_plugin_config("demo-plugin", &invalid_config)
        .await
        .is_err());
}
