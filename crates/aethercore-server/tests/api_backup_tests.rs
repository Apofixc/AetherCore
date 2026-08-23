//! # Интеграционные тесты REST API резервного копирования и телеметрии БД

use aethercore_common::config::AppConfig;
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, BackupService, LoggerService, NotifyService, SchedulerService};
use aethercore_core::users::UserService;
use aethercore_server::create_app_router;
use aethercore_server::state::AppState;
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;
use tower::ServiceExt;

async fn setup_backup_test_app() -> (axum::Router, AppState, String, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("api_test.db");
    let backups_dir = temp_dir.path().join("backups");

    let db = Db::init(&db_path, 5, 5000).await.expect("DB init failed");
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new("test-secret-key-backup-12345", 3600);
    let user_service = UserService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let logger_service = LoggerService::new();
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());
    let scheduler_service = Arc::new(SchedulerService::new(db.clone()));
    let backup_service = BackupService::new(db.clone(), backups_dir);

    user_service.ensure_default_admin().await.unwrap();

    let state = AppState {
        config: AppConfig::default(),
        db,
        bus,
        jwt_manager: jwt_manager.clone(),
        user_service,
        audit_service,
        logger_service,
        notify_service,
        plugin_manager,
        scheduler_service,
        backup_service,
        start_time: Instant::now(),
    };

    let router = create_app_router(state.clone());
    let admin_token = jwt_manager
        .generate_token(
            uuid::Uuid::new_v4(),
            "admin",
            true,
            vec!["admin".to_string()],
            vec!["*".to_string()],
        )
        .unwrap();

    (router, state, admin_token, temp_dir)
}

#[tokio::test]
async fn test_db_stats_and_backup_api_lifecycle() {
    let (app, _state, token, _dir) = setup_backup_test_app().await;

    // 1. GET /api/v1/system/db/stats
    let req = Request::builder()
        .uri("/api/v1/system/db/stats")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["storage"]["db_size_bytes"].as_u64().unwrap() > 0);
    assert_eq!(json["total_backups_count"], 0);

    // 2. POST /api/v1/system/backup/create
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/system/backup/create")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::json!({ "tag": "test_api" }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let created_backup: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let filename = created_backup["filename"].as_str().unwrap().to_string();
    assert!(filename.contains("test_api"));

    // 3. GET /api/v1/system/backup/list
    let req = Request::builder()
        .uri("/api/v1/system/backup/list")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["filename"], filename);

    // 4. GET /api/v1/system/backup/download/:filename
    let req = Request::builder()
        .uri(format!("/api/v1/system/backup/download/{}", filename))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(!body.is_empty());

    // 5. POST /api/v1/system/backup/restore
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/system/backup/restore")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::json!({ "filename": filename }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let restore_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(restore_json["success"], true);

    // 6. DELETE /api/v1/system/backup/:filename
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v1/system/backup/{}", filename))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
