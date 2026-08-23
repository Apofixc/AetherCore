//! # Интеграционные тесты 2FA (TOTP и Backup Codes) API

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use aethercore_common::config::AppConfig;
use aethercore_core::auth::totp::create_totp_instance;
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, LoggerService, NotifyService};
use aethercore_core::users::UserService;
use aethercore_server::api::auth::{LoginResponse, TotpSetupResponse};
use aethercore_server::create_app_router;
use aethercore_server::state::AppState;
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
    let scheduler_service = std::sync::Arc::new(aethercore_core::services::SchedulerService::new(db.clone()));

    let backup_service = aethercore_core::services::BackupService::new(
        db.clone(),
        std::path::PathBuf::from("target/test_backups_2fa"),
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
async fn test_full_2fa_lifecycle_and_login_flow() {
    let (app, _state) = setup_test_app().await;

    // 1. Первичный вход администратора без 2FA
    let login_payload = serde_json::json!({
        "username": "root",
        "password": "root"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let login_data: LoginResponse = serde_json::from_slice(&body).unwrap();
    assert!(login_data.success);
    assert!(!login_data.requires_2fa);
    let auth_token = login_data.token;

    // 2. Инициация настройки 2FA
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/2fa/setup")
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let setup_data: TotpSetupResponse = serde_json::from_slice(&body).unwrap();
    assert!(!setup_data.secret.is_empty());
    assert!(setup_data.qr_code_url.starts_with("data:image/png;base64,"));
    assert_eq!(setup_data.backup_codes.len(), 8);

    // 3. Подтверждение и включение 2FA с валидным TOTP кодом
    let totp = create_totp_instance(&setup_data.secret, "root").unwrap();
    let valid_code = totp.generate_current().unwrap();

    let enable_payload = serde_json::json!({
        "secret": setup_data.secret,
        "code": valid_code,
        "backup_codes": setup_data.backup_codes
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/2fa/enable")
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&enable_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Попытка логина на шаге 1 (только логин и пароль) -> получение 2FA challenge
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let challenge: LoginResponse = serde_json::from_slice(&body).unwrap();
    assert!(!challenge.success);
    assert!(challenge.requires_2fa);
    assert!(challenge.temp_token.is_some());
    let temp_token = challenge.temp_token.unwrap();

    // 5. Попытка подтверждения входа с неверным кодом
    let verify_fail_payload = serde_json::json!({
        "temp_token": temp_token,
        "code": "000000"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/2fa/verify-login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&verify_fail_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 6. Подтверждение входа с валидным TOTP кодом -> успешное получение JWT
    let current_code = totp.generate_current().unwrap();
    let verify_ok_payload = serde_json::json!({
        "temp_token": temp_token,
        "code": current_code
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/2fa/verify-login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&verify_ok_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let full_login: LoginResponse = serde_json::from_slice(&body).unwrap();
    assert!(full_login.success);
    assert!(!full_login.token.is_empty());
    let authed_token_2fa = full_login.token;

    // 7. Проверка авторизованного запроса GET /me
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/auth/me")
        .header(header::AUTHORIZATION, format!("Bearer {}", authed_token_2fa))
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 8. Вход с использованием резервного кода (Backup code)
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let challenge: LoginResponse = serde_json::from_slice(&body).unwrap();
    let temp_token = challenge.temp_token.unwrap();

    let first_backup_code = &setup_data.backup_codes[0];
    let backup_verify_payload = serde_json::json!({
        "temp_token": temp_token,
        "code": first_backup_code,
        "is_backup_code": true
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/2fa/verify-login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&backup_verify_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let backup_login: LoginResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(backup_login.backup_codes_left, Some(7));

    // 9. Повторное использование того же резервного кода (должно быть отклонено)
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let challenge: LoginResponse = serde_json::from_slice(&body).unwrap();
    let temp_token = challenge.temp_token.unwrap();

    let backup_repeat_payload = serde_json::json!({
        "temp_token": temp_token,
        "code": first_backup_code,
        "is_backup_code": true
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/2fa/verify-login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&backup_repeat_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    // 10. Отключение 2FA
    let disable_payload = serde_json::json!({
        "password": "root"
    });
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/2fa/disable")
        .header(header::AUTHORIZATION, format!("Bearer {}", authed_token_2fa))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&disable_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 11. Проверка логина после отключения 2FA (прямой вход без 2FA)
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&login_payload).unwrap()))
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let final_login: LoginResponse = serde_json::from_slice(&body).unwrap();
    assert!(final_login.success);
    assert!(!final_login.requires_2fa);
}

#[tokio::test]
async fn test_mfa_scope_and_per_user_override() {
    let (app, state) = setup_test_app().await;

    // 1. Создание обычного оператора
    let operator = state
        .user_service
        .create_user(aethercore_common::models::user::CreateUserDto {
            username: "operator1".to_string(),
            password: "Operator123!".to_string(),
            full_name: Some("Operator One".to_string()),
            email: Some("op1@aethercore.local".to_string()),
            department: None,
            is_active: Some(true),
            is_superuser: Some(false),
            must_change_password: Some(false),
            is_username_locked: Some(false),
            force_2fa: None,
            roles: Some(vec!["operator".to_string()]),
        })
        .await
        .unwrap();

    let root_user = state
        .user_service
        .get_user_by_username("root")
        .await
        .unwrap();

    // 2. Тестирование логики is_mfa_enforced_for_user при mfa_scope = "admins_only"
    let mut policy = aethercore_common::models::user::SecurityPoliciesDto::default();
    policy.mfa_scope = "admins_only".to_string();

    assert!(aethercore_server::api::auth::is_mfa_enforced_for_user(&policy, &root_user));
    assert!(!aethercore_server::api::auth::is_mfa_enforced_for_user(&policy, &operator));

    // 3. Персональное требование 2FA для оператора (force_2fa = Some(true))
    let mut operator_enforced = operator.clone();
    operator_enforced.force_2fa = Some(true);
    assert!(aethercore_server::api::auth::is_mfa_enforced_for_user(&policy, &operator_enforced));

    // 4. Персональное освобождение администратора (force_2fa = Some(false))
    let mut root_exempt = root_user.clone();
    root_exempt.force_2fa = Some(false);
    assert!(!aethercore_server::api::auth::is_mfa_enforced_for_user(&policy, &root_exempt));

    // 5. Проверка GET /api/v1/auth/config
    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/auth/config")
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let auth_cfg: aethercore_server::api::auth::AuthConfigResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(auth_cfg.mfa_scope, "disabled");
    assert_eq!(auth_cfg.mfa_remember_device_days, 0);
    assert_eq!(auth_cfg.mfa_grace_period_days, 0);
    assert_eq!(auth_cfg.mfa_backup_codes_count, 8);
}
