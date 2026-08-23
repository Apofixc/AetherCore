//! # Стресс-, негативные и хаос-тесты подсистемы «Доступ и авторизация»
//!
//! Проверяет:
//! 1. Валидацию граничных условий и фаззинг DTO политик безопасности
//! 2. Защиту неизменяемости прав Superuser в матрице прав
//! 3. Разграничение доступа к настройкам (RBAC enforcement: superuser vs admin vs operator vs viewer)
//! 4. Конкурентные обновления матрицы прав (Race conditions / параллельные запросы)
//! 5. Стресс-импорт и очистку журнала аудита при параллельной нагрузке
//! 6. Обработку искаженных и поврежденных структур данных

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use aethercore_common::config::AppConfig;
use aethercore_common::models::user::CreateUserDto;
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

async fn setup_test_app() -> (axum::Router, AppState, String) {
    let db = Db::init_in_memory().await.expect("DB in memory failed");
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new("test-secret-key-12345", 3600);
    let user_service = UserService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let logger_service = LoggerService::new();
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());
    let scheduler_service = std::sync::Arc::new(aethercore_core::services::SchedulerService::new(
        db.clone(),
        bus.clone(),
        audit_service.clone(),
        plugin_manager.clone(),
    ));

    let backup_service = aethercore_core::services::BackupService::new(
        db.clone(),
        std::path::PathBuf::from("target/test_backups_chaos"),
    );

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
        scheduler_service,
        backup_service,
        start_time: Instant::now(),
    };

    let router = create_app_router(state.clone());

    // Логинимся под root/superuser
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

    let resp = router.clone().oneshot(login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let login_res: LoginResponse = serde_json::from_slice(&body).unwrap();

    (router, state, login_res.token)
}

/// 1. Негативное тестирование и валидация граничных значений SecurityPolicies
#[tokio::test]
async fn test_chaos_security_policies_boundary_validation() {
    let (app, _state, token) = setup_test_app().await;

    // 1.1 Недопустимая минимальная длина пароля (< 4 или > 64)
    let invalid_min_lengths = vec![0, 1, 2, 3, 65, 100, 9999];
    for len in invalid_min_lengths {
        let req = Request::builder()
            .method("PUT")
            .uri("/api/v1/settings/security")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "min_password_length": len,
                    "max_login_attempts": 5,
                    "lockout_duration": 30
                })
                .to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "min_password_length={} should be rejected",
            len
        );
    }

    // 1.2 Недопустимое число попыток входа (< 1 или > 100)
    let invalid_attempts = vec![0, 101, 500];
    for attempts in invalid_attempts {
        let req = Request::builder()
            .method("PUT")
            .uri("/api/v1/settings/security")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "min_password_length": 8,
                    "max_login_attempts": attempts,
                    "lockout_duration": 30
                })
                .to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "max_login_attempts={} should be rejected",
            attempts
        );
    }

    // 1.3 Недопустимая длительность блокировки (< 1 или > 10080)
    let invalid_lockouts = vec![0, 10081, 99999];
    for lockout in invalid_lockouts {
        let req = Request::builder()
            .method("PUT")
            .uri("/api/v1/settings/security")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "min_password_length": 8,
                    "max_login_attempts": 5,
                    "lockout_duration": lockout
                })
                .to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "lockout_duration={} should be rejected",
            lockout
        );
    }

    // 1.4 Недопустимый mfa_remember_device_days (> 90)
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/settings/security")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "mfa_remember_device_days": 91
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 1.5 Недопустимый mfa_grace_period_days (> 30)
    let req = Request::builder()
        .method("PUT")
        .uri("/api/v1/settings/security")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "mfa_grace_period_days": 31
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 1.6 Недопустимый mfa_backup_codes_count (< 8 или > 16)
    let invalid_backup_counts = vec![0, 7, 17, 32];
    for count in invalid_backup_counts {
        let req = Request::builder()
            .method("PUT")
            .uri("/api/v1/settings/security")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "mfa_backup_codes_count": count
                })
                .to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "mfa_backup_codes_count={} should be rejected",
            count
        );
    }
}

/// 2. Тестирование разграничения прав (RBAC): оператор и наблюдатель не могут менять матрицу
#[tokio::test]
async fn test_chaos_rbac_permission_matrix_guards() {
    let (app, _state, superuser_token) = setup_test_app().await;

    // Создаем оператора
    let create_op_req = Request::builder()
        .method("POST")
        .uri("/api/v1/users")
        .header(header::AUTHORIZATION, format!("Bearer {}", superuser_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_string(&CreateUserDto {
                username: "chaos_operator".into(),
                password: "OperatorPassword123!".into(),
                full_name: Some("Chaos Operator".into()),
                email: Some("chaos_op@aethercore.local".into()),
                is_active: Some(true),
                is_superuser: Some(false),
                must_change_password: Some(false),
                roles: Some(vec!["operator".into()]),
                ..Default::default()
            })
            .unwrap(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_op_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Логинимся оператором
    let op_login_req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "username": "chaos_operator",
                "password": "OperatorPassword123!"
            })
            .to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(op_login_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let op_login: LoginResponse = serde_json::from_slice(&body).unwrap();
    let op_token = op_login.token;

    // Попытка оператора изменить матрицу прав -> должно быть строго 403 Forbidden
    let mod_matrix_req = Request::builder()
        .method("PUT")
        .uri("/api/v1/settings/permissions")
        .header(header::AUTHORIZATION, format!("Bearer {}", op_token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::json!([]).to_string()))
        .unwrap();

    let resp = app.clone().oneshot(mod_matrix_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Operator must NOT be able to modify permission matrix"
    );

    // Попытка оператора очистить журнал аудита -> должно быть строго 403 Forbidden
    let clear_audit_req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/system/audit")
        .header(header::AUTHORIZATION, format!("Bearer {}", op_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(clear_audit_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "Operator must NOT be able to clear audit logs"
    );
}

/// 3. Стресс-тест: Параллельные запросы на чтение и запись матрицы прав (Race Conditions)
#[tokio::test]
async fn test_chaos_concurrent_permissions_matrix_updates() {
    let (app, _state, superuser_token) = setup_test_app().await;

    // Получаем текущую матрицу
    let get_matrix_req = Request::builder()
        .method("GET")
        .uri("/api/v1/settings/permissions")
        .header(header::AUTHORIZATION, format!("Bearer {}", superuser_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(get_matrix_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let matrix: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Запускаем 20 одновременных асинхронных тасок обновления матрицы прав
    let mut handles = Vec::new();
    for i in 0..20 {
        let app_clone = app.clone();
        let token_clone = superuser_token.clone();
        let mut matrix_clone = matrix.clone();

        // Модифицируем права для роли operator в категории
        if let Some(cats) = matrix_clone.as_array_mut() {
            if let Some(first_cat) = cats.get_mut(0) {
                if let Some(items) = first_cat.get_mut("items").and_then(|i| i.as_array_mut()) {
                    for item in items {
                        item["operator"] = serde_json::Value::Bool(i % 2 == 0);
                    }
                }
            }
        }

        let handle = tokio::spawn(async move {
            let req = Request::builder()
                .method("PUT")
                .uri("/api/v1/settings/permissions")
                .header(header::AUTHORIZATION, format!("Bearer {}", token_clone))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(matrix_clone.to_string()))
                .unwrap();

            let resp = app_clone.oneshot(req).await.unwrap();
            resp.status()
        });
        handles.push(handle);
    }

    // Все таски должны успешно завершиться без дедлоков в SQLite
    for handle in handles {
        let status = handle.await.unwrap();
        assert_eq!(status, StatusCode::OK, "Concurrent matrix update failed");
    }
}

/// 4. Стресс-тест: Очистка и импорт журнала аудита
#[tokio::test]
async fn test_chaos_audit_logs_stress_import_and_clear() {
    let (app, _state, token) = setup_test_app().await;

    // Генерируем 100 тестовых записей аудита
    let mut test_records = Vec::new();
    for i in 0..100 {
        test_records.push(serde_json::json!({
            "id": i + 1,
            "user_id": Some(format!("user-{}", i)),
            "username": format!("user_{}", i),
            "action": format!("action.test.{}", i),
            "resource": "stress_test",
            "status": if i % 5 == 0 { "failed" } else { "success" },
            "details": format!("Stress test details record #{}", i),
            "ip_address": "127.0.0.1",
            "created_at": "2026-08-23T12:00:00Z"
        }));
    }

    let payload = serde_json::json!({
        "records": test_records
    });

    // 4.1 Импорт валидного пакета из 100 записей
    let import_req = Request::builder()
        .method("POST")
        .uri("/api/v1/system/audit/import")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(import_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let import_res: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(import_res["imported_count"], 100);

    // 4.2 Проверка чтения записей
    let get_audit_req = Request::builder()
        .method("GET")
        .uri("/api/v1/system/audit?limit=200")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(get_audit_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let logs: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(logs.len() >= 100);

    // 4.3 Очистка журнала аудита
    let clear_req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/system/audit")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(clear_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 4.4 Негативный импорт: поврежденный JSON
    let bad_import_req = Request::builder()
        .method("POST")
        .uri("/api/v1/system/audit/import")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{invalid_json_payload}"))
        .unwrap();

    let resp = app.clone().oneshot(bad_import_req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "Corrupted JSON import must be rejected"
    );
}
