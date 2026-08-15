//! # Интеграционные тесты REST API сервера

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use nms_common::config::AppConfig;
use nms_common::manifest::ModuleManifest;
use nms_common::models::user::UserResponseDto;
use nms_core::auth::JwtManager;
use nms_core::bus::EventBus;
use nms_core::db::Db;
use nms_core::plugins::loader::PluginPackage;
use nms_core::plugins::PluginManager;
use nms_core::services::{AuditService, NotifyService};
use nms_core::users::UserService;
use nms_server::api::auth::LoginResponse;
use nms_server::api::modules::ModuleSummaryDto;
use nms_server::create_app_router;
use nms_server::state::AppState;
use std::collections::HashMap;
use std::time::Instant;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState) {
    let db = Db::init_in_memory().await.expect("DB in memory failed");
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new("test-secret-key-12345", 3600);
    let user_service = UserService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
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

    let state = AppState {
        config: AppConfig::default(),
        db,
        bus,
        jwt_manager,
        user_service,
        audit_service,
        notify_service,
        plugin_manager,
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

    // 1. Логин под admin
    let login_payload = serde_json::json!({
        "username": "admin",
        "password": "admin"
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
    assert_eq!(login_res.user.username, "admin");

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
    assert_eq!(me_user.username, "admin");
}

#[tokio::test]
async fn test_modules_api_and_assets() {
    let (app, _) = setup_test_app().await;

    // Логинимся под admin
    let login_payload = serde_json::json!({"username": "admin", "password": "admin"});
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
