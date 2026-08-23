use aethercore_common::config::AppConfig;
use aethercore_common::models::scheduler::{
    ConcurrencyPolicy, CreateTaskDto, ExecutionStatus, MisfirePolicy, ScheduledTask,
    TaskAction, TaskExecutionRecord, TaskSchedule,
};
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, LoggerService, NotifyService, SchedulerService};
use aethercore_core::users::UserService;
use aethercore_server::create_app_router;
use aethercore_server::state::AppState;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceExt;

async fn setup_test_app() -> (axum::Router, AppState, String) {
    let db = Db::init_in_memory().await.expect("DB in memory failed");
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new("test-secret-key-12345", 3600);
    let user_service = UserService::new(db.clone());
    let session_service = aethercore_core::services::SessionService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let logger_service = LoggerService::new();
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());
    let scheduler_service = Arc::new(SchedulerService::new(db.clone()));
    scheduler_service
        .register_handler(
            "system_history_cleanup",
            Arc::new(aethercore_core::services::handlers::HistoryCleanupTaskHandler::new(db.clone())),
        )
        .await;
    scheduler_service.seed_default_tasks().await.unwrap();

    let backup_service = aethercore_core::services::BackupService::new(
        db.clone(),
        std::path::PathBuf::from("target/test_backups_sched"),
    );

    user_service.ensure_default_admin().await.unwrap();

    let state = AppState {
        config: AppConfig::default(),
        db,
        bus,
        jwt_manager: jwt_manager.clone(),
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

    // Выпускаем токен для админа
    let token = jwt_manager
        .generate_token(
            uuid::Uuid::new_v4(),
            "admin",
            false,
            vec!["admin".to_string()],
            vec![
                "system.view".to_string(),
                "system.manage".to_string(),
            ],
        )
        .unwrap();

    (router, state, token)
}

#[tokio::test]
async fn test_api_scheduler_crud_and_run() {
    let (app, _state, token) = setup_test_app().await;

    // 1. GET /api/v1/system/scheduler/tasks
    let req = Request::builder()
        .uri("/api/v1/system/scheduler/tasks")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let tasks: Vec<ScheduledTask> = serde_json::from_slice(&body_bytes).unwrap();
    assert!(tasks.iter().any(|t| t.id == "sys-audit-retention"));

    // 2. POST /api/v1/system/scheduler/tasks -> create new task
    let create_dto = CreateTaskDto {
        id: Some("api-test-task".to_string()),
        name: "API Test Task".to_string(),
        description: Some("Created via REST API".to_string()),
        schedule: TaskSchedule::Cron("0 */5 * * * *".to_string()),
        action: TaskAction::SystemHistoryCleanup,
        concurrency_policy: ConcurrencyPolicy::Skip,
        misfire_policy: MisfirePolicy::SkipToNext,
        timeout_secs: 180,
        is_enabled: true,
    };
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/system/scheduler/tasks")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&create_dto).unwrap()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let created: ScheduledTask = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(created.id, "api-test-task");
    assert_eq!(created.name, "API Test Task");

    // 3. POST /api/v1/system/scheduler/tasks/api-test-task/run -> manual execution
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/system/scheduler/tasks/api-test-task/run")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let record: TaskExecutionRecord = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(record.task_id, "api-test-task");
    assert_eq!(record.status, ExecutionStatus::Success);

    // 4. GET /api/v1/system/scheduler/tasks/api-test-task/history
    let req = Request::builder()
        .uri("/api/v1/system/scheduler/tasks/api-test-task/history")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let history: Vec<TaskExecutionRecord> = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(history.len(), 1);

    // 5. POST /api/v1/system/scheduler/tasks/api-test-task/toggle -> disable
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/system/scheduler/tasks/api-test-task/toggle")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::json!({ "is_enabled": false }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let toggled: ScheduledTask = serde_json::from_slice(&body_bytes).unwrap();
    assert!(!toggled.is_enabled);

    // 6. DELETE /api/v1/system/scheduler/tasks/api-test-task
    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/system/scheduler/tasks/api-test-task")
        .header("Authorization", format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}
