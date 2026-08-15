// Паритет с test_security_stage1.py / test_security_stage2.py:
// блокировка спуфинга топиков событий, валидация capabilities и совместимости ядра

use nms_core::plugin::manifest::{ManifestError, ModuleManifest};
use semver::Version;

fn base_manifest(extra: &str) -> String {
    format!("id: net-scanner\nname: Net Scanner\nversion: 1.0.0\n{extra}")
}

#[test]
fn test_topic_spoofing_blocked() {
    let yaml = base_manifest("events:\n  publishes:\n    - core.system.shutdown\n");
    let m = ModuleManifest::from_yaml(&yaml).unwrap();
    let err = m.validate(&Version::new(0, 1, 0)).unwrap_err();
    assert!(matches!(err, ManifestError::TopicSpoofing { .. }));
}

#[test]
fn test_own_prefix_topics_allowed() {
    let yaml = base_manifest(
        "events:\n  publishes:\n    - net-scanner.device.up\n    - net-scanner.device.down\n",
    );
    let m = ModuleManifest::from_yaml(&yaml).unwrap();
    assert!(m.validate(&Version::new(0, 1, 0)).is_ok());
}

#[test]
fn test_invalid_module_id_rejected() {
    let yaml = "id: Net_Scanner\nname: X\nversion: 1.0.0\n";
    let m = ModuleManifest::from_yaml(yaml).unwrap();
    assert!(matches!(
        m.validate(&Version::new(0, 1, 0)).unwrap_err(),
        ManifestError::InvalidId(_)
    ));
}

#[test]
fn test_incompatible_core_version_blocked() {
    let yaml = base_manifest("min_core_version: 9.0.0\n");
    let m = ModuleManifest::from_yaml(&yaml).unwrap();
    assert!(matches!(
        m.validate(&Version::new(0, 1, 0)).unwrap_err(),
        ManifestError::IncompatibleCoreVersion { .. }
    ));
}

#[test]
fn test_capabilities_default_deny() {
    // По умолчанию плагин не получает никаких прав: ни сети, ни ФС, ни env
    let m = ModuleManifest::from_yaml(&base_manifest("")).unwrap();
    assert!(!m.capabilities.network.allow_raw_sockets);
    assert!(m.capabilities.network.allowed_hosts.is_empty());
    assert!(m.capabilities.filesystem.allow_host_dirs.is_empty());
    assert!(m.capabilities.environment.allow_env_vars.is_empty());
}

#[test]
fn test_self_dependency_rejected() {
    let yaml = base_manifest("deps:\n  - net-scanner\n");
    let m = ModuleManifest::from_yaml(&yaml).unwrap();
    assert!(matches!(
        m.validate(&Version::new(0, 1, 0)).unwrap_err(),
        ManifestError::SelfDependency(_)
    ));
}
