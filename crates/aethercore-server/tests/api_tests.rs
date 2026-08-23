//! # Интеграционные тесты REST API сервера

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use aethercore_common::config::AppConfig;
use aethercore_common::manifest::ModuleManifest;
use aethercore_common::models::user::UserResponseDto;
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::loader::PluginPackage;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, LogLevel, LoggerService, NotifyService};
use aethercore_core::users::UserService;
use aethercore_server::api::auth::LoginResponse;
use aethercore_server::api::modules::ModuleSummaryDto;
use aethercore_server::create_app_router;
use aethercore_server::state::AppState;
use std::collections::HashMap;
use std::time::Instant;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState) {
    let db = Db::init_in_memory().await.expect("DB in memory failed");
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new("test-secret-key-12345", 3600);
    let user_service = UserService::new(db.clone());
    let session_service = aethercore_core::services::SessionService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let logger_service = LoggerService::new();
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());

    user_service.ensure_default_admin().await.unwrap();

    // Регистрируем тестовый плагин
    let manifest = ModuleManifest::from_yaml(
        r#"
id: "test-plugin"
name: "Test Module"
version: "1.0.0"
description: "Test module for API"
"#,
    )
    .unwrap();

    let mut frontend_assets = HashMap::new();
    frontend_assets.insert("frontend/dist/ui.js".into(), b"window.__test = 1;".to_vec());

    let package = PluginPackage {
        manifest,
        manifest_raw: vec![],
        backend_wasm: None,
        signature: None,
        locales: HashMap::new(),
        frontend_assets,
    };
    plugin_manager.register_plugin(package).await.unwrap();

    let scheduler_service = std::sync::Arc::new(aethercore_core::services::SchedulerService::new(db.clone()));

    let backup_service = aethercore_core::services::BackupService::new(
        db.clone(),
        std::path::PathBuf::from("target/test_backups_api"),
    );

    let state = AppState {
        config: AppConfig::default(),
        db,
        bus,
        jwt_manager,
        user_service,
        session_service,
        audit_service,
        logger_service,
        notify_service,
        plugin_manager,
        scheduler_service,
        backup_service,
        start_time: Instant::now(),
    };

    let router = create_app_router(state.clone());
    (router, state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let (app, _) = setup_test_app().await;

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_auth_login_and_me() {
    let (app, _) = setup_test_app().await;

    // 1. Логин под root
    let login_payload = serde_json::json!({
        "username": "root",
        "password": "root"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let login_res: LoginResponse = serde_json::from_slice(&body_bytes).unwrap();
    assert!(login_res.success);
    assert_eq!(login_res.user.as_ref().unwrap().username, "root");

    // 2. Запрос /api/v1/auth/me с полученным Bearer токеном
    let me_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/me")
                .header(header::AUTHORIZATION, format!("Bearer {}", login_res.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(me_response.status(), StatusCode::OK);
    let me_bytes = me_response.into_body().collect().await.unwrap().to_bytes();
    let me_user: UserResponseDto = serde_json::from_slice(&me_bytes).unwrap();
    assert_eq!(me_user.username, "root");
}

#[tokio::test]
async fn test_auth_config_and_disabled_web_ui_auth() {
    let (app, state) = setup_test_app().await;

    // 1. Проверяем публичный эндпоинт /api/v1/auth/config
    let config_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(config_res.status(), StatusCode::OK);
    let config_bytes = config_res.into_body().collect().await.unwrap().to_bytes();
    let config_val: serde_json::Value = serde_json::from_slice(&config_bytes).unwrap();
    assert_eq!(config_val["web_ui_auth"], true);

    // 2. Отключаем web_ui_auth в KV Store
    let kv = aethercore_core::db::kv::KvStore::system(state.db.clone());
    kv.set(
        "security_policies",
        &serde_json::json!({
            "web_ui_auth": false,
            "mandatory_password_change": true,
            "force_2fa": false,
            "max_login_attempts": 5,
            "lockout_duration": 30,
            "session_ttl": 12,
            "inactivity_timeout": 30,
            "min_password_length": 8,
            "require_uppercase": true,
            "require_digits": true,
            "require_special": true,
            "ip_whitelist": ""
        }),
    )
    .await
    .unwrap();

    // 3. Запрос без Authorization заголовка к защищенному эндпоинту (/api/v1/users)
    let users_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Запрос успешно проходит без токена, так как web_ui_auth = false
    assert_eq!(users_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_modules_api_and_assets() {
    let (app, _) = setup_test_app().await;

    // Логинимся под root
    let login_payload = serde_json::json!({"username": "root", "password": "root"});
    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body_bytes = login_res.into_body().collect().await.unwrap().to_bytes();
    let login_data: LoginResponse = serde_json::from_slice(&body_bytes).unwrap();
    let token = login_data.token;

    // 1. Получаем список модулей через /api/v1/modules
    let modules_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/modules")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(modules_res.status(), StatusCode::OK);
    let mod_bytes = modules_res.into_body().collect().await.unwrap().to_bytes();
    let modules: Vec<ModuleSummaryDto> = serde_json::from_slice(&mod_bytes).unwrap();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].id, "test-plugin");

    // 2. Запрашиваем фронтенд-ассет плагина напрямую через /modules/test-plugin/dist/ui.js
    let asset_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/modules/test-plugin/dist/ui.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(asset_res.status(), StatusCode::OK);
    let asset_bytes = asset_res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&asset_bytes[..], b"window.__test = 1;");
}

#[tokio::test]
async fn test_logs_endpoints() {
    let (app, state) = setup_test_app().await;

    // Авторизуемся под root
    let login_payload = serde_json::json!({
        "username": "root",
        "password": "root"
    });

    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body_bytes = login_res.into_body().collect().await.unwrap().to_bytes();
    let login_data: LoginResponse = serde_json::from_slice(&body_bytes).unwrap();
    let token = login_data.token;

    // Пишем несколько тестовых записей в LoggerService
    state.logger_service.log(LogLevel::Info, "aethercore_core", "Server boot initialized");
    state.logger_service.log(LogLevel::Warn, "aethercore_core", "High memory usage detected");
    state.logger_service.log(LogLevel::Error, "test_plugin", "SNMP device unreachable");

    // 1. Проверяем /api/v1/system/logs/providers
    let prov_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/system/logs/providers")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(prov_res.status(), StatusCode::OK);
    let prov_bytes = prov_res.into_body().collect().await.unwrap().to_bytes();
    let providers: Vec<aethercore_core::services::LogProvider> = serde_json::from_slice(&prov_bytes).unwrap();
    assert!(providers.iter().any(|p| p.id == "system"));

    // 2. Проверяем /api/v1/system/logs (все логи)
    let logs_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/system/logs")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(logs_res.status(), StatusCode::OK);
    let logs_bytes = logs_res.into_body().collect().await.unwrap().to_bytes();
    let query_res: aethercore_core::services::LogQueryResult = serde_json::from_slice(&logs_bytes).unwrap();
    assert_eq!(query_res.total, 3);

    // 3. Проверяем фильтрацию по уровню ERROR
    let error_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/system/logs?level=ERROR")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(error_res.status(), StatusCode::OK);
    let error_bytes = error_res.into_body().collect().await.unwrap().to_bytes();
    let error_query: aethercore_core::services::LogQueryResult = serde_json::from_slice(&error_bytes).unwrap();
    assert_eq!(error_query.total, 1);
    assert_eq!(error_query.entries[0].level, LogLevel::Error);
    assert!(error_query.entries[0].message.contains("SNMP device unreachable"));

    // 4. Проверяем скачивание лога /api/v1/system/logs/download
    let dl_res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/system/logs/download?provider=system")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(dl_res.status(), StatusCode::OK);
    assert_eq!(
        dl_res.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/plain; charset=utf-8"
    );
    let dl_bytes = dl_res.into_body().collect().await.unwrap().to_bytes();
    let dl_content = String::from_utf8(dl_bytes.to_vec()).unwrap();
    assert!(dl_content.contains("Server boot initialized"));
    assert!(dl_content.contains("SNMP device unreachable"));
}

#[tokio::test]
async fn test_settings_endpoints() {
    let (app, _) = setup_test_app().await;

    // 1. Авторизация под root
    let login_payload = serde_json::json!({
        "username": "root",
        "password": "root"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let login_res: LoginResponse = serde_json::from_slice(&bytes).unwrap();
    let token = login_res.token;

    // 2. Тест user-preferences: GET (дефолтные)
    let get_pref_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/settings/user-preferences")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_pref_res.status(), StatusCode::OK);
    let pref_bytes = get_pref_res.into_body().collect().await.unwrap().to_bytes();
    let user_prefs: aethercore_server::api::settings::UserPreferencesDto =
        serde_json::from_slice(&pref_bytes).unwrap();
    assert_eq!(user_prefs.timezone, "UTC");

    // 3. Тест user-preferences: PUT (обновление)
    let mut updated_prefs = user_prefs.clone();
    updated_prefs.timezone = "Europe/Moscow".to_string();
    updated_prefs.sound_info = "Gentle Bell".to_string();

    let put_pref_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings/user-preferences")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&updated_prefs).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(put_pref_res.status(), StatusCode::OK);

    // Проверяем, что сохранилось в SQLite
    let get_pref_res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/settings/user-preferences")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let pref_bytes2 = get_pref_res2.into_body().collect().await.unwrap().to_bytes();
    let user_prefs2: aethercore_server::api::settings::UserPreferencesDto =
        serde_json::from_slice(&pref_bytes2).unwrap();
    assert_eq!(user_prefs2.timezone, "Europe/Moscow");
    assert_eq!(user_prefs2.sound_info, "Gentle Bell");
    assert_eq!(user_prefs2.avatar, None);

    // 3.1 Тест частичного обновления avatar
    let avatar_data = "data:image/jpeg;base64,/9j/4AAQSkZJRg==";
    let put_avatar_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings/user-preferences")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "avatar": avatar_data }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(put_avatar_res.status(), StatusCode::OK);

    // Проверяем, что аватар сохранился, а предыдущие поля (timezone) не затерлись
    let get_pref_res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/settings/user-preferences")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let pref_bytes3 = get_pref_res3.into_body().collect().await.unwrap().to_bytes();
    let user_prefs3: aethercore_server::api::settings::UserPreferencesDto =
        serde_json::from_slice(&pref_bytes3).unwrap();
    assert_eq!(user_prefs3.timezone, "Europe/Moscow");
    assert_eq!(user_prefs3.avatar, Some(avatar_data.to_string()));

    // 4. Тест security policies: GET и PUT
    let get_sec_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/settings/security")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_sec_res.status(), StatusCode::OK);
    let sec_bytes = get_sec_res.into_body().collect().await.unwrap().to_bytes();
    let sec_policies: aethercore_server::api::settings::SecurityPoliciesDto =
        serde_json::from_slice(&sec_bytes).unwrap();
    assert_eq!(sec_policies.max_login_attempts, 5);

    let mut updated_sec = sec_policies.clone();
    updated_sec.max_login_attempts = 10;
    updated_sec.ip_whitelist = "10.0.0.0/8".to_string();

    let put_sec_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings/security")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&updated_sec).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(put_sec_res.status(), StatusCode::OK);

    // 5. Тест permissions matrix: GET и PUT
    let get_perm_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/settings/permissions")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_perm_res.status(), StatusCode::OK);
    let perm_bytes = get_perm_res.into_body().collect().await.unwrap().to_bytes();
    let mut matrix: Vec<serde_json::Value> = serde_json::from_slice(&perm_bytes).unwrap();
    assert!(!matrix.is_empty());

    // Обновляем и сохраняем
    matrix[0]["name"] = serde_json::json!("Updated Category");
    let put_perm_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings/permissions")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&matrix).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(put_perm_res.status(), StatusCode::OK);

    // 6. Тест maintenance settings: GET и PUT
    let get_maint_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/settings/maintenance")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_maint_res.status(), StatusCode::OK);
    let maint_bytes = get_maint_res.into_body().collect().await.unwrap().to_bytes();
    let mut maint: aethercore_server::api::settings::MaintenanceSettingsDto =
        serde_json::from_slice(&maint_bytes).unwrap();
    assert_eq!(maint.auto_backup, true);

    maint.backup_interval_hours = 12;
    let put_maint_res = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings/maintenance")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&maint).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(put_maint_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_audit_clear_rotate_and_import() {
    let (app, state) = setup_test_app().await;

    let login_payload = serde_json::json!({
        "username": "root",
        "password": "root"
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let login_res: LoginResponse = serde_json::from_slice(&bytes).unwrap();
    let token = login_res.token;

    // 1. Создаем тестовые записи аудита
    for i in 1..=5 {
        state
            .audit_service
            .log(
                Some("usr-1"),
                Some("admin"),
                &format!("action.{}", i),
                "resource/test",
                "success",
                Some("test details"),
                Some("127.0.0.1"),
            )
            .await
            .unwrap();
    }

    // Проверяем количество
    let count = state.audit_service.count_logs(None).await.unwrap();
    assert!(count >= 5);

    // 2. Тест POST /api/v1/system/audit/rotate (ротация с днями = 0 для очистки тестовых)
    let rotate_payload = serde_json::json!({
        "days": 0,
        "archive": true
    });

    let rotate_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/audit/rotate")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&rotate_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(rotate_res.status(), StatusCode::OK);
    let rotate_bytes = rotate_res.into_body().collect().await.unwrap().to_bytes();
    let rotate_data: serde_json::Value = serde_json::from_slice(&rotate_bytes).unwrap();
    assert_eq!(rotate_data["success"], true);

    // 3. Тест POST /api/v1/system/audit/import
    let import_payload = serde_json::json!({
        "records": [
            {
                "id": 9999,
                "user_id": "usr-restored",
                "username": "restored_admin",
                "action": "restored.action",
                "resource": "audit/archive",
                "status": "success",
                "details": "Restored event from cold archive",
                "ip_address": "10.0.0.1",
                "created_at": "2026-08-01T12:00:00Z"
            }
        ]
    });

    let import_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/audit/import")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&import_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(import_res.status(), StatusCode::OK);
    let import_bytes = import_res.into_body().collect().await.unwrap().to_bytes();
    let import_data: serde_json::Value = serde_json::from_slice(&import_bytes).unwrap();
    assert_eq!(import_data["success"], true);
    assert_eq!(import_data["imported_count"], 1);

    // 4. Тест DELETE /api/v1/system/audit (ручная очистка)
    let clear_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/system/audit")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(clear_res.status(), StatusCode::OK);
    let clear_bytes = clear_res.into_body().collect().await.unwrap().to_bytes();
    let clear_data: serde_json::Value = serde_json::from_slice(&clear_bytes).unwrap();
    assert_eq!(clear_data["success"], true);

    // В журнале должен остаться только 1 лог (о факте очистки audit.clear)
    let logs_after = state.audit_service.list_logs(50, None, None).await.unwrap();
    assert_eq!(logs_after.len(), 1);
    assert_eq!(logs_after[0].action, "audit.clear");
}

#[tokio::test]
async fn test_events_topology_and_dlq_endpoints() {
    let (app, state) = setup_test_app().await;

    // Логин для получения токена
    let login_payload = serde_json::json!({
        "username": "root",
        "password": "root"
    });

    let login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body_bytes = login_res.into_body().collect().await.unwrap().to_bytes();
    let login_data: LoginResponse = serde_json::from_slice(&body_bytes).unwrap();
    let token = login_data.token;

    // 1. Публикуем событие через шину и регистрируем подписку
    let mut _sub = state.bus.subscribe_named("plugin:test", &["sensor.*"]);
    state
        .bus
        .publish(aethercore_common::models::events::EventMessage::telemetry(
            "sensor.temp",
            "plugin:sensor_source",
            serde_json::json!({"val": 25.0}),
        ))
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 2. Тест GET /api/v1/events/topology
    let topo_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/events/topology")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(topo_res.status(), StatusCode::OK);
    let topo_bytes = topo_res.into_body().collect().await.unwrap().to_bytes();
    let topo_data: aethercore_core::bus::BusTopologySnapshot =
        serde_json::from_slice(&topo_bytes).unwrap();
    assert!(topo_data.publishers_count >= 1);
    assert!(topo_data.subscribers_count >= 1);

    // 3. Симулируем таймаут RPC для попадания в DLQ
    let _ = state
        .bus
        .request("unresponsive.endpoint", serde_json::json!({}), std::time::Duration::from_millis(20))
        .await;

    // 4. Тест GET /api/v1/events/dlq
    let dlq_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/events/dlq")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(dlq_res.status(), StatusCode::OK);
    let dlq_bytes = dlq_res.into_body().collect().await.unwrap().to_bytes();
    let dlq_list: Vec<aethercore_core::bus::DeadLetter> =
        serde_json::from_slice(&dlq_bytes).unwrap();
    assert_eq!(dlq_list.len(), 1);
    let dlq_id = dlq_list[0].id;

    // 5. Тест POST /api/v1/events/dlq/{id}/redrive
    let redrive_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/events/dlq/{}/redrive", dlq_id))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(redrive_res.status(), StatusCode::OK);

    // 6. Тест DELETE /api/v1/events/dlq
    let delete_dlq_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/events/dlq")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_dlq_res.status(), StatusCode::OK);
}



