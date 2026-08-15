// Паритет с test_module_context.py: жизненный цикл движка плагинов,
// реестр модулей, политика подписи и статусы загрузки

use nms_core::plugin::{ModuleLoadStatus, PluginEngine};

const MANIFEST: &str = "id: ui-only\nname: UI Only\nversion: 1.0.0\n";

fn temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("nms-engine-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_dev_module(modules_dir: &std::path::Path, id: &str, manifest: &str) {
    let dir = modules_dir.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("manifest.yaml"), manifest).unwrap();
}

#[tokio::test]
async fn test_manifest_only_module_registered() {
    let modules = temp_dir();
    let cache = temp_dir();
    write_dev_module(&modules, "ui-only", MANIFEST);

    let mut engine = PluginEngine::new(&modules, &cache, None, true).unwrap();
    let topo = engine.load_all().await.unwrap();
    assert_eq!(topo.order, vec!["ui-only"]);
    let record = engine.registry.get("ui-only").unwrap();
    assert_eq!(record.status, ModuleLoadStatus::ManifestOnly);
    assert!(engine.running_modules().is_empty());
}

#[tokio::test]
async fn test_unsigned_module_blocked_in_production_mode() {
    let modules = temp_dir();
    let cache = temp_dir();
    write_dev_module(&modules, "ui-only", MANIFEST);

    let mut engine = PluginEngine::new(&modules, &cache, None, false).unwrap();
    engine.load_all().await.unwrap();
    let record = engine.registry.get("ui-only").unwrap();
    assert!(matches!(record.status, ModuleLoadStatus::Blocked(_)));
}

#[tokio::test]
async fn test_invalid_manifest_blocked_isolated() {
    let modules = temp_dir();
    let cache = temp_dir();
    // Модуль со спуфингом топика блокируется, валидный модуль загружается
    write_dev_module(
        &modules,
        "spoofer",
        "id: spoofer\nname: S\nversion: 1.0.0\nevents:\n  publishes:\n    - core.fake\n",
    );
    write_dev_module(&modules, "ui-only", MANIFEST);

    let mut engine = PluginEngine::new(&modules, &cache, None, true).unwrap();
    let topo = engine.load_all().await.unwrap();
    assert_eq!(topo.order, vec!["ui-only"]);
    assert!(matches!(
        engine.registry.get("spoofer").unwrap().status,
        ModuleLoadStatus::Blocked(_)
    ));
    assert_eq!(
        engine.registry.get("ui-only").unwrap().status,
        ModuleLoadStatus::ManifestOnly
    );
}

#[tokio::test]
async fn test_missing_required_dependency_fails_load() {
    let modules = temp_dir();
    let cache = temp_dir();
    write_dev_module(
        &modules,
        "consumer",
        "id: consumer\nname: C\nversion: 1.0.0\ndeps:\n  - provider\n",
    );

    let mut engine = PluginEngine::new(&modules, &cache, None, true).unwrap();
    assert!(engine.load_all().await.is_err());
}

#[tokio::test]
async fn test_stop_module_noop_for_manifest_only() {
    let modules = temp_dir();
    let cache = temp_dir();
    write_dev_module(&modules, "ui-only", MANIFEST);

    let mut engine = PluginEngine::new(&modules, &cache, None, true).unwrap();
    engine.load_all().await.unwrap();
    engine.stop_module("ui-only").await.unwrap();
    assert!(engine.running_modules().is_empty());
}
