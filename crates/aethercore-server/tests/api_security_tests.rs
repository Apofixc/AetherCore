//! # Интеграционные тесты REST API: Защита эскалации прав и проверка иерархии RBAC

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use aethercore_common::config::AppConfig;
use aethercore_common::models::user::{CreateUserDto, UpdateUserDto, UserResponseDto};
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, LoggerService, NotifyService};
use aethercore_core::users::UserService;
use aethercore_server::api::auth::LoginResponse;
use aethercore_server::create_app_router;
use aethercore_server::state::AppState;
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

    // 1. Вход под суперпользователем root
    let login_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "root",
                "password": "root"
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
                password: "AdminPassword123!".into(),
                full_name: Some("Regular Admin".into()),
                email: Some("reg_admin@aethercore.local".into()),
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
                "password": "AdminPassword123!"
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
                password: "Password123!".into(),
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
    let root_admin = state.user_service.get_user_by_username("root").await.unwrap();
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

#[tokio::test]
async fn test_ip_whitelist_and_policy_enforcement() {
    use aethercore_server::middleware::is_ip_allowed;

    // 1. Пустой белый список разрешает всё
    assert!(is_ip_allowed("192.168.1.50", ""));
    assert!(is_ip_allowed("10.0.0.1", "   "));

    // 2. Localhost всегда разрешен (Anti-Lockout)
    assert!(is_ip_allowed("127.0.0.1", "10.0.0.0/24"));
    assert!(is_ip_allowed("::1", "10.0.0.0/24"));

    // 3. Точные IP и подсети
    let whitelist = "192.168.1.100, 10.10.0.0/16, 172.16.5.0/24";
    assert!(is_ip_allowed("192.168.1.100", whitelist));
    assert!(is_ip_allowed("10.10.20.30", whitelist));
    assert!(is_ip_allowed("172.16.5.123", whitelist));

    // 4. Запрещенные IP
    assert!(!is_ip_allowed("192.168.1.101", whitelist));
    assert!(!is_ip_allowed("10.11.0.1", whitelist));
    assert!(!is_ip_allowed("8.8.8.8", whitelist));
}

#[tokio::test]
async fn test_role_hierarchy_escalation_and_permissions_matrix() {
    let (app, state) = setup_test_app().await;

    // Генерируем токен оператора с правами users.manage и access.roles.manage
    let op_user_id = uuid::Uuid::new_v4();
    let op_token = state
        .jwt_manager
        .generate_token(
            op_user_id,
            "power_operator",
            false,
            vec!["operator".into()],
            vec![
                "users.manage".into(),
                "users.view".into(),
                "access.roles.manage".into(),
                "access.roles.view".into(),
            ],
        )
        .unwrap();

    // 1. Оператор пытается создать администратора (роль выше своего уровня) -> 403 Forbidden
    let create_admin_by_op = Request::builder()
        .method("POST")
        .uri("/api/v1/users")
        .header(header::AUTHORIZATION, format!("Bearer {}", op_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&CreateUserDto {
                username: "sneaky_admin".into(),
                password: "Password123!".into(),
                roles: Some(vec!["admin".into()]),
                ..Default::default()
            })
            .unwrap(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_admin_by_op).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Оператор не должен иметь возможности создать администратора"
    );

    // 2. Оператор пытается изменить матрицу прав ролей -> 403 Forbidden
    let update_matrix_by_op = Request::builder()
        .method("PUT")
        .uri("/api/v1/settings/permissions")
        .header(header::AUTHORIZATION, format!("Bearer {}", op_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!([
                {
                    "id": "core",
                    "name": "Core System",
                    "icon": "settings",
                    "items": []
                }
            ])
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(update_matrix_by_op).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Оператор не должен иметь возможности сохранять матрицу прав ролей"
    );

    // 3. Суперпользователь обновляет матрицу прав, выдавая оператору право modules.manage
    let super_token = state
        .jwt_manager
        .generate_token(
            uuid::Uuid::new_v4(),
            "root_admin",
            true,
            vec!["superuser".into()],
            vec!["*".into()],
        )
        .unwrap();

    let new_matrix = serde_json::json!([
        {
            "id": "modules",
            "name": "Modules",
            "icon": "view_in_ar",
            "items": [
                { "id": "modules_view", "name": "View Modules", "code": "modules.view", "description": "", "admin": true, "operator": true, "viewer": true },
                { "id": "modules_manage", "name": "Manage Modules", "code": "modules.manage", "description": "", "admin": true, "operator": true, "viewer": false }
            ]
        },
        {
            "id": "users",
            "name": "Users",
            "icon": "group",
            "items": [
                { "id": "users_view", "name": "View Users", "code": "users.view", "description": "", "admin": true, "operator": true, "viewer": false },
                { "id": "users_manage", "name": "Manage Users", "code": "users.manage", "description": "", "admin": true, "operator": false, "viewer": false }
            ]
        }
    ]);

    let update_matrix_by_super = Request::builder()
        .method("PUT")
        .uri("/api/v1/settings/permissions")
        .header(header::AUTHORIZATION, format!("Bearer {}", super_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&new_matrix).unwrap()))
        .unwrap();

    let resp_super = app.clone().oneshot(update_matrix_by_super).await.unwrap();
    assert_eq!(resp_super.status(), StatusCode::OK);

    // 4. Создаем оператора через UserService и проверяем, что его разрешения загрузились из обновленной role_permissions таблицы
    let created_op = state
        .user_service
        .create_user(CreateUserDto {
            username: "test_operator_dyn".into(),
            password: "Password123!".into(),
            full_name: Some("Operator Dyn".into()),
            email: Some("op_dyn@example.com".into()),
            department: Some("Operations".into()),
            roles: Some(vec!["operator".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(
        created_op.permissions.contains(&"modules.manage".to_string()),
        "Оператор должен динамически получить право modules.manage из обновленной матрицы ролей"
    );
}
