//! # Интеграционные тесты REST API глобальных сессий операторов

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use aethercore_common::config::AppConfig;
use aethercore_common::models::user::CreateUserDto;
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, BackupService, LoggerService, NotifyService, SchedulerService, SessionService};
use aethercore_core::users::UserService;
use aethercore_server::{create_app_router, AppState};
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState) {
    let db = Db::init_in_memory().await.expect("DB in memory failed");
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new("test-secret-key-12345", 3600);
    let user_service = UserService::new(db.clone());
    let session_service = SessionService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let logger_service = LoggerService::new();
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());
    let scheduler_service = std::sync::Arc::new(SchedulerService::new(db.clone()));

    let backup_service = BackupService::new(
        db.clone(),
        std::path::PathBuf::from("target/test_backups_sessions"),
    );

    user_service.ensure_default_admin().await.unwrap();

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
        start_time: std::time::Instant::now(),
    };

    let router = create_app_router(state.clone());
    (router, state)
}

#[tokio::test]
async fn test_sessions_api_full_flow() {
    let (app, state) = setup_test_app().await;

    // Создаем оператора operator1
    let _op1 = state
        .user_service
        .create_user(CreateUserDto {
            username: "operator1".into(),
            password: "Password123!".into(),
            full_name: Some("Op 1".into()),
            roles: Some(vec!["operator".into()]),
            is_active: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();

    // 1. Логин под root (админ)
    let admin_login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .header("User-Agent", "Admin Browser / Linux")
                .body(Body::from(
                    json!({ "username": "root", "password": "root" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(admin_login_res.status(), StatusCode::OK);
    let admin_body = admin_login_res.into_body().collect().await.unwrap().to_bytes();
    let admin_json: Value = serde_json::from_slice(&admin_body).unwrap();
    let admin_token = admin_json["token"].as_str().unwrap().to_string();

    // 2. Логин под operator1
    let op_login_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("Content-Type", "application/json")
                .header("User-Agent", "Operator Tablet / Android")
                .body(Body::from(
                    json!({ "username": "operator1", "password": "Password123!" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(op_login_res.status(), StatusCode::OK);
    let op_body = op_login_res.into_body().collect().await.unwrap().to_bytes();
    let op_json: Value = serde_json::from_slice(&op_body).unwrap();
    let op_token = op_json["token"].as_str().unwrap().to_string();

    // 3. Получение списка сессий администратором
    let list_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/system/sessions")
                .header("Authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_res.status(), StatusCode::OK);
    let list_body = list_res.into_body().collect().await.unwrap().to_bytes();
    let sessions_list: Vec<Value> = serde_json::from_slice(&list_body).unwrap();
    assert_eq!(sessions_list.len(), 2);

    let admin_sess = sessions_list.iter().find(|s| s["username"] == "root").unwrap();
    assert_eq!(admin_sess["is_current"], true);
    assert_eq!(admin_sess["user_agent"], "Admin Browser / Linux");

    let op_sess = sessions_list.iter().find(|s| s["username"] == "operator1").unwrap();
    assert_eq!(op_sess["is_current"], false);
    assert_eq!(op_sess["user_agent"], "Operator Tablet / Android");
    let op_sess_id = op_sess["id"].as_str().unwrap();

    // 4. Проверяем, что токен оператора успешно работает на защищенных эндпоинтах
    let me_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/me")
                .header("Authorization", format!("Bearer {}", op_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_res.status(), StatusCode::OK);

    // 5. Админ отзывает сессию оператора
    let revoke_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/system/sessions/{}", op_sess_id))
                .header("Authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke_res.status(), StatusCode::OK);

    // 6. Проверяем, что запрос с отозванным токеном оператора немедленно отклоняется (401 Unauthorized)
    let me_after_revoke_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/me")
                .header("Authorization", format!("Bearer {}", op_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_after_revoke_res.status(), StatusCode::UNAUTHORIZED);

    // 7. Сброс всех сессий
    let term_all_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/system/sessions/terminate-all")
                .header("Authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(term_all_res.status(), StatusCode::OK);

    // После terminate-all даже токен админа становится недействительным
    let admin_after_all_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/system/sessions")
                .header("Authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_after_all_res.status(), StatusCode::UNAUTHORIZED);
}
