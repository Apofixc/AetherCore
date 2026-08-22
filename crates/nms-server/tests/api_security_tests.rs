//! # Интеграционные тесты REST API: Защита эскалации прав и проверка иерархии RBAC

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use nms_common::config::AppConfig;
use nms_common::models::user::{CreateUserDto, UpdateUserDto, UserResponseDto};
use nms_core::auth::JwtManager;
use nms_core::bus::EventBus;
use nms_core::db::Db;
use nms_core::plugins::PluginManager;
use nms_core::services::{AuditService, LoggerService, NotifyService};
use nms_core::users::UserService;
use nms_server::api::auth::LoginResponse;
use nms_server::create_app_router;
use nms_server::state::AppState;
use std::time::Instant;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState) {
    let db = Db::init_in_memory().await.expect("DB in memory failed");
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new("test-secret-key-12345", 3600);
    let user_service = UserService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let logger_service = LoggerService::new();
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());

    user_service.ensure_default_admin().await.unwrap();

    let state = AppState {
        config: AppConfig::default(),
        db,
        bus,
        jwt_manager,
        user_service,
        audit_service,
        logger_service,
        notify_service,
        plugin_manager,
        start_time: Instant::now(),
    };

    let router = create_app_router(state.clone());
    (router, state)
}

#[tokio::test]
async fn test_api_privilege_escalation_protection() {
    let (app, state) = setup_test_app().await;

    // 1. Вход под суперпользователем admin
    let login_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "admin",
                "password": "admin"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let superuser_login: LoginResponse = serde_json::from_slice(&body).unwrap();
    let superuser_token = superuser_login.token;

    // 2. Создаем обычного администратора (is_superuser = false, роль admin)
    let create_admin_req = Request::builder()
        .method("POST")
        .uri("/api/v1/users")
        .header(header::AUTHORIZATION, format!("Bearer {}", superuser_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&CreateUserDto {
                username: "reg_admin".into(),
                password: "adminpassword123".into(),
                full_name: Some("Regular Admin".into()),
                email: Some("reg_admin@nms.local".into()),
                is_active: Some(true),
                is_superuser: Some(false),
                must_change_password: Some(false),
                roles: Some(vec!["admin".into()]),
                ..Default::default()
            })
            .unwrap(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_admin_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let normal_admin: UserResponseDto = serde_json::from_slice(&body).unwrap();
    assert!(!normal_admin.is_superuser);

    // 3. Логинимся под обычным администратором reg_admin
    let admin_login_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "reg_admin",
                "password": "adminpassword123"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(admin_login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let normal_admin_login: LoginResponse = serde_json::from_slice(&body).unwrap();
    let normal_admin_token = normal_admin_login.token;

    // -------------------------------------------------------------------------
    // Атака A: Обычный администратор пытается создать суперпользователя -> 403
    // -------------------------------------------------------------------------
    let create_super_attack = Request::builder()
        .method("POST")
        .uri("/api/v1/users")
        .header(header::AUTHORIZATION, format!("Bearer {}", normal_admin_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&CreateUserDto {
                username: "super_backdoor".into(),
                password: "password123".into(),
                is_active: Some(true),
                is_superuser: Some(true),
                roles: Some(vec!["superuser".into()]),
                ..Default::default()
            })
            .unwrap(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_super_attack).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Обычный администратор не должен иметь возможности создавать суперпользователей"
    );

    // -------------------------------------------------------------------------
    // Атака B: Обычный администратор пытается отредактировать суперпользователя -> 403
    // -------------------------------------------------------------------------
    let root_admin = state.user_service.get_user_by_username("admin").await.unwrap();
    let edit_super_attack = Request::builder()
        .method("PUT")
        .uri(format!("/api/v1/users/{}", root_admin.id))
        .header(header::AUTHORIZATION, format!("Bearer {}", normal_admin_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&UpdateUserDto {
                full_name: Some("Hacked Root".into()),
                password: Some("hacked_root_password".into()),
                ..Default::default()
            })
            .unwrap(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(edit_super_attack).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Обычный администратор не должен иметь возможности менять профиль суперпользователя"
    );

    // -------------------------------------------------------------------------
    // Атака C: Обычный администратор пытается удалить суперпользователя -> 403
    // -------------------------------------------------------------------------
    let del_super_attack = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v1/users/{}", root_admin.id))
        .header(header::AUTHORIZATION, format!("Bearer {}", normal_admin_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(del_super_attack).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Обычный администратор не должен иметь возможности удалять суперпользователей"
    );

    // -------------------------------------------------------------------------
    // Атака D: Пользователь пытается удалить самого себя -> 400 Bad Request
    // -------------------------------------------------------------------------
    let self_del_attack = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v1/users/{}", normal_admin.id))
        .header(header::AUTHORIZATION, format!("Bearer {}", normal_admin_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(self_del_attack).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Пользователь не должен иметь возможности удалить самого себя"
    );
}
