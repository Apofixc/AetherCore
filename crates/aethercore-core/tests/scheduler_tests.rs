use aethercore_common::models::scheduler::{
    ConcurrencyPolicy, CreateTaskDto, ExecutionStatus, HistoryQueryDto, MisfirePolicy,
    TaskAction, TaskSchedule, UpdateTaskDto,
};
use aethercore_core::db::Db;
use aethercore_core::services::SchedulerService;
use chrono::Utc;

#[tokio::test]
async fn test_cron_parsing_and_next_run() {
    let now = Utc::now();

    // 1. 5-позиционный cron
    let sched_5 = TaskSchedule::Cron("0 0 * * *".to_string());
    assert!(sched_5.validate().is_ok());
    let next_5 = sched_5.calculate_next_run(now);
    assert!(next_5.is_some());
    assert!(next_5.unwrap() > now);

    // 2. 6-позиционный cron
    let sched_6 = TaskSchedule::Cron("0 */10 * * * *".to_string());
    assert!(sched_6.validate().is_ok());
    let next_6 = sched_6.calculate_next_run(now);
    assert!(next_6.is_some());

    // 3. Интервал
    let sched_interval = TaskSchedule::IntervalSec(60);
    assert!(sched_interval.validate().is_ok());
    let next_int = sched_interval.calculate_next_run(now);
    assert_eq!(
        next_int.unwrap().timestamp(),
        (now + chrono::Duration::seconds(60)).timestamp()
    );

    // 4. Невалидный cron
    let invalid_cron = TaskSchedule::Cron("invalid * * cron".to_string());
    assert!(invalid_cron.validate().is_err());
}

#[tokio::test]
async fn test_scheduler_service_lifecycle_and_crud() {
    let db = Db::init_in_memory().await.expect("Init DB failed");
    let scheduler = SchedulerService::new(db.clone());
    scheduler.seed_default_tasks().await.expect("Seed tasks failed");

    // 1. Проверяем наличие сидированных системных задач
    let tasks = scheduler.list_tasks().await.expect("List tasks failed");
    assert!(tasks.iter().any(|t| t.id == "sys-audit-retention" && t.is_system));
    assert!(tasks.iter().any(|t| t.id == "sys-history-cleanup" && t.is_system));
    assert!(tasks.iter().any(|t| t.id == "sys-auto-backup" && t.is_system));

    // 2. Создание пользовательской задачи
    let create_dto = CreateTaskDto {
        id: Some("test-backup-task".to_string()),
        name: "Test Backup Task".to_string(),
        description: Some("Тестовый бэкап".to_string()),
        schedule: TaskSchedule::IntervalSec(300),
        action: TaskAction::SystemDbBackup,
        concurrency_policy: ConcurrencyPolicy::Skip,
        misfire_policy: MisfirePolicy::SkipToNext,
        timeout_secs: 60,
        is_enabled: true,
    };
    let created = scheduler.create_task(create_dto).await.expect("Create task failed");
    assert_eq!(created.id, "test-backup-task");
    assert!(!created.is_system);

    // 3. Получение задачи по ID
    let fetched = scheduler.get_task("test-backup-task").await.expect("Get task failed");
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().name, "Test Backup Task");

    // 4. Обновление задачи
    let update_dto = UpdateTaskDto {
        name: Some("Updated Backup Task".to_string()),
        description: None,
        schedule: None,
        action: None,
        concurrency_policy: Some(ConcurrencyPolicy::Allow),
        misfire_policy: None,
        timeout_secs: Some(120),
        is_enabled: Some(false),
    };
    let updated = scheduler.update_task("test-backup-task", update_dto).await.expect("Update failed");
    assert_eq!(updated.name, "Updated Backup Task");
    assert_eq!(updated.concurrency_policy, ConcurrencyPolicy::Allow);
    assert!(!updated.is_enabled);

    // 5. Переключение активности (Toggle)
    let toggled_on = scheduler
        .toggle_task("test-backup-task", true)
        .await
        .expect("Toggle on failed");
    assert!(toggled_on.is_enabled);
    assert!(toggled_on.next_run_at.is_some());

    // 6. Регистрация обработчика и ручной запуск задачи
    scheduler
        .register_handler(
            "system_db_backup",
            std::sync::Arc::new(aethercore_core::services::handlers::HistoryCleanupTaskHandler::new(
                db.clone(),
            )),
        )
        .await;

    let record = scheduler
        .run_task_now("test-backup-task", "manual:admin")
        .await
        .expect("Run task now failed");
    assert_eq!(record.status, ExecutionStatus::Success);
    assert_eq!(record.triggered_by, "manual:admin");

    // 7. Проверка истории выполнения
    let history = scheduler
        .get_history(HistoryQueryDto {
            task_id: Some("test-backup-task".to_string()),
            limit: Some(10),
            offset: None,
        })
        .await
        .expect("Get history failed");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, ExecutionStatus::Success);

    // 8. Попытка изменить системную задачу
    let forbidden_update = scheduler
        .update_task(
            "sys-audit-retention",
            UpdateTaskDto {
                name: Some("Hacked".to_string()),
                description: None,
                schedule: None,
                action: None,
                concurrency_policy: None,
                misfire_policy: None,
                timeout_secs: None,
                is_enabled: None,
            },
        )
        .await;
    assert!(forbidden_update.is_err());

    // 9. Удаление пользовательской задачи
    scheduler
        .delete_task("test-backup-task")
        .await
        .expect("Delete failed");
    let after_delete = scheduler.get_task("test-backup-task").await.expect("Get failed");
    assert!(after_delete.is_none());

    // 10. Попытка удалить системную задачу
    let forbidden_delete = scheduler.delete_task("sys-audit-retention").await;
    assert!(forbidden_delete.is_err());
}

#[tokio::test]
async fn test_scheduler_crash_recovery() {
    let db = Db::init_in_memory().await.expect("Init DB failed");
    let pool = db.writer();
    let now = Utc::now().to_rfc3339();

    // Вручную вставляем задачу со статусом running (как будто сервер упал во время исполнения)
    sqlx::query(
        r#"
        INSERT INTO scheduled_tasks (
            id, name, description, schedule_type, schedule_value,
            action_type, action_params, concurrency_policy, misfire_policy,
            timeout_secs, is_enabled, is_system, last_status, next_run_at, created_at, updated_at
        ) VALUES (
            'hung-task', 'Hung Task', 'Test', 'interval', '60',
            'system_history_cleanup', NULL, 'skip', 'skip_to_next',
            300, 1, 0, 'running', ?, ?, ?
        )
        "#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .expect("Insert hung task failed");

    let scheduler = SchedulerService::new(db.clone());

    // Запускаем восстановление
    scheduler.recover_orphaned_tasks().await.expect("Recover failed");

    // Проверяем, что статус переведен в aborted
    let task = scheduler.get_task("hung-task").await.expect("Get task failed").unwrap();
    assert_eq!(task.last_status, Some(ExecutionStatus::Aborted));

    let history = scheduler
        .get_history(HistoryQueryDto {
            task_id: Some("hung-task".to_string()),
            limit: None,
            offset: None,
        })
        .await
        .expect("Get history failed");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, ExecutionStatus::Aborted);
}
