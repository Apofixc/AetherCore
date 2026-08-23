//! # Сквозной интеграционный тест жизненного цикла ядра и модулей

use aethercore_common::i18n::{global, Locale};
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, NotifyService};
use aethercore_core::users::UserService;
use std::path::Path;

#[tokio::test]
async fn test_full_platform_lifecycle() {
    // 1. Инициализация БД SQLite в памяти
    let db = Db::init_in_memory().await.expect("Database init failed");

    // 2. Инициализация сервисов ядра
    let bus = EventBus::new(db.clone());
    let _jwt_mgr = JwtManager::new("super-secret-jwt-key", 3600);
    let user_service = UserService::new(db.clone());
    let _audit_service = AuditService::new(db.clone());
    let _notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());

    // 3. Создание дефолтного root суперпользователя
    user_service.ensure_default_admin().await.unwrap();
    let admin = user_service.authenticate("root", "root").await.unwrap();
    assert!(admin.is_superuser);

    // 4. Загрузка реального .aether-plugin архива из каталога modules/
    let modules_dir = Path::new("../../modules");
    let fallback_dir = Path::new("modules");
    let actual_dir = if modules_dir.exists() {
        modules_dir
    } else {
        fallback_dir
    };

    let loaded_count = plugin_manager
        .load_plugins_from_dir(actual_dir)
        .await
        .expect("Failed to load plugins");

    assert_eq!(loaded_count, 1);

    // 5. Проверка зарегистрированного плагина
    let plugin = plugin_manager
        .get_plugin("example-plugin")
        .expect("example-plugin not found");

    assert_eq!(plugin.package.manifest.name, "Демонстрационный Модуль");
    assert!(plugin.is_enabled);

    // 6. Проверка сквозного i18n
    let title_ru = global().translate(Locale::Ru, "example-plugin.title", &[]);
    assert_eq!(title_ru, "Демонстрационный Модуль");

    let title_en = global().translate(Locale::En, "example-plugin.title", &[]);
    assert_eq!(title_en, "Example Demo Plugin");

    // 7. Проверка отдачи фронтенд ассета из ZIP архива
    let ui_js = plugin_manager.get_frontend_asset("example-plugin", "dist/ui.js");
    assert!(ui_js.is_some());
    let ui_js_str = String::from_utf8(ui_js.unwrap()).unwrap();
    assert!(ui_js_str.contains("DemoDashboardView"));

    // 8. Проверка конфигурации и JSON Schema валидации
    let valid_cfg = serde_json::json!({
        "refresh_interval_sec": 15,
        "max_items": 100,
        "debug_mode": true
    });
    plugin_manager
        .set_plugin_config("example-plugin", &valid_cfg)
        .await
        .expect("Valid config should pass");

    let saved_cfg = plugin_manager
        .get_plugin_config("example-plugin")
        .await
        .unwrap();
    assert_eq!(saved_cfg, Some(valid_cfg));

    let invalid_cfg = serde_json::json!({
        "refresh_interval_sec": 0, // minimum is 1
        "max_items": 100
    });
    assert!(plugin_manager
        .set_plugin_config("example-plugin", &invalid_cfg)
        .await
        .is_err());

    // 9. Проверка журнала событий (микропауза для асинхронного L2 батч-воркера)
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
    let events = bus
        .query_journal(Some("example-plugin."), None, 10)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].topic, "example-plugin.config_changed");
}
