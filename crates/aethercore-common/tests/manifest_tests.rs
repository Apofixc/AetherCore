//! # Тесты парсинга, валидации и DAG резолвера манифеста плагинов

use aethercore_common::manifest::{resolve_module_dag, ModuleManifest};

const SAMPLE_MANIFEST: &str = r#"
manifest_version: 1
id: "example-plugin"
name: "Example Demo Plugin"
version: "1.0.0"
description: "Universal demo plugin for tests"
type: "feature"
enabled_by_default: true
min_core_version: "2.0.0"

events:
  publishes:
    - "example-plugin.status_updated"
  subscribes:
    - "core.system_started"

config_schema:
  type: "object"
  required: ["interval_sec"]
  properties:
    interval_sec:
      type: "integer"
      minimum: 1
"#;

#[test]
fn test_parse_and_validate_manifest() {
    let manifest = ModuleManifest::from_yaml(SAMPLE_MANIFEST).expect("Valid YAML");
    assert_eq!(manifest.id, "example-plugin");
    assert_eq!(manifest.version, "1.0.0");
    assert!(manifest.is_compatible_with_core("2.0.0").unwrap());
    assert!(manifest.is_compatible_with_core("2.5.0").unwrap());
    assert!(!manifest.is_compatible_with_core("1.9.0").unwrap());
}

#[test]
fn test_spoofing_prevention() {
    let invalid_manifest = r#"
id: "evil-plugin"
name: "Evil Plugin"
version: "1.0.0"
description: "Attempts to spoof events"
events:
  publishes:
    - "other-plugin.secret_event"
"#;
    let res = ModuleManifest::from_yaml(invalid_manifest);
    assert!(res.is_err());
}

#[test]
fn test_config_schema_validation() {
    let manifest = ModuleManifest::from_yaml(SAMPLE_MANIFEST).unwrap();

    let valid_config = serde_json::json!({"interval_sec": 10});
    assert!(manifest.validate_config(&valid_config).is_ok());

    let invalid_config = serde_json::json!({"interval_sec": 0});
    assert!(manifest.validate_config(&invalid_config).is_err());
}

#[test]
fn test_dag_resolution() {
    let mut m1 = ModuleManifest::from_yaml(SAMPLE_MANIFEST).unwrap();
    m1.id = "mod-a".into();
    m1.events.publishes = vec!["mod-a.event".into()];

    let mut m2 = ModuleManifest::from_yaml(SAMPLE_MANIFEST).unwrap();
    m2.id = "mod-b".into();
    m2.events.publishes = vec!["mod-b.event".into()];
    m2.deps = vec!["mod-a".into()];

    let mut m3 = ModuleManifest::from_yaml(SAMPLE_MANIFEST).unwrap();
    m3.id = "mod-c".into();
    m3.events.publishes = vec!["mod-c.event".into()];
    m3.deps = vec!["mod-b".into()];

    let resolved = resolve_module_dag(&[m3.clone(), m1.clone(), m2.clone()]).unwrap();
    let ids: Vec<String> = resolved.into_iter().map(|m| m.id).collect();
    assert_eq!(ids, vec!["mod-a", "mod-b", "mod-c"]);
}
