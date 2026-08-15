// Паритет с test_settings_changed_notification.py: валидация JSON Schema
// настроек плагина и генерация системных событий изменения конфигурации

use nms_core::plugin::manifest::ModuleManifest;
use semver::Version;
use serde_json::json;

fn manifest_with_schema() -> ModuleManifest {
    let yaml = r#"
id: poller
name: Poller
version: 1.0.0
config_schema:
  type: object
  required: [interval]
  properties:
    interval:
      type: integer
      minimum: 1
    enabled:
      type: boolean
"#;
    ModuleManifest::from_yaml(yaml).unwrap()
}

#[test]
fn test_valid_config_accepted() {
    let m = manifest_with_schema();
    assert!(m.validate(&Version::new(0, 1, 0)).is_ok());
    assert!(m
        .validate_config(&json!({"interval": 30, "enabled": true}))
        .is_ok());
}

#[test]
fn test_invalid_config_rejected() {
    let m = manifest_with_schema();
    // Отсутствует обязательное поле
    assert!(m.validate_config(&json!({"enabled": true})).is_err());
    // Нарушение типа и minimum
    assert!(m.validate_config(&json!({"interval": 0})).is_err());
    assert!(m.validate_config(&json!({"interval": "fast"})).is_err());
}

#[test]
fn test_module_without_schema_accepts_any_config() {
    let m = ModuleManifest::from_yaml("id: simple\nname: S\nversion: 1.0.0\n").unwrap();
    assert!(m.validate_config(&json!({"anything": [1, 2, 3]})).is_ok());
}

#[test]
fn test_broken_schema_blocks_manifest() {
    let yaml = r#"
id: broken
name: B
version: 1.0.0
config_schema:
  type: no-such-type
"#;
    let m = ModuleManifest::from_yaml(yaml).unwrap();
    assert!(m.validate(&Version::new(0, 1, 0)).is_err());
}

#[tokio::test]
async fn test_settings_changed_event_broadcast() {
    // notify_settings_changed рассылает module_settings_changed без падения при pool = None
    nms_core::events::notify_settings_changed(None, "poller", None, None).await;
}
