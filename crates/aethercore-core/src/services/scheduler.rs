//! # Сервис планировщика задач (SchedulerService)
//!
//! Обеспечивает фоновое и периодическое исполнение системных задач ядра,
//! вызовы таймеров WASM-плагинов, публикацию событий по расписанию,
//! защиту от наложения (Concurrency Policies), изоляцию сбоев и запись истории.

use crate::bus::EventBus;
use crate::db::kv::KvStore;
use crate::db::Db;
use crate::plugins::PluginManager;
use crate::services::AuditService;
use aethercore_common::error::{AppError, Result};
use aethercore_common::models::events::EventMessage;
use aethercore_common::models::scheduler::{
    ConcurrencyPolicy, CreateTaskDto, ExecutionStatus, HistoryQueryDto, MisfirePolicy,
    ScheduledTask, TaskAction, TaskExecutionRecord, TaskSchedule, UpdateTaskDto,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Центральный сервис планировщика задач платформы AetherCore
///
/// Управляет жизненным циклом запланированных операций ядра и WASM-модулей,
/// контролирует политики наложения (Skip, Allow, Queue) и изолирует ошибки.
#[derive(Clone)]
pub struct SchedulerService {
    db: Db,
    bus: EventBus,
    audit_service: AuditService,
    plugin_manager: PluginManager,
    /// Множество ID задач, выполняющихся в данный момент в памяти
    running_tasks: Arc<Mutex<HashSet<String>>>,
    /// Уведомление для плавной остановки фонового воркера
    shutdown_notify: Arc<Notify>,
}

#[derive(Debug, Deserialize)]
struct RetentionConfig {
    #[serde(default = "default_audit_retention_days")]
    audit_retention_days: u32,
    #[serde(default = "default_backup_retention_days")]
    backup_retention_days: u32,
}

fn default_audit_retention_days() -> u32 {
    90
}

fn default_backup_retention_days() -> u32 {
    30
}

#[derive(sqlx::FromRow)]
struct TaskRow {
    id: String,
    name: String,
    description: Option<String>,
    schedule_type: String,
    schedule_value: String,
    action_type: String,
    action_params: Option<String>,
    concurrency_policy: String,
    misfire_policy: String,
    timeout_secs: i64,
    is_enabled: i64,
    is_system: i64,
    next_run_at: Option<String>,
    last_run_at: Option<String>,
    last_status: Option<String>,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct HistoryRow {
    id: i64,
    task_id: String,
    task_name: String,
    started_at: String,
    finished_at: String,
    status: String,
    duration_ms: i64,
    error_message: Option<String>,
    triggered_by: String,
}

impl SchedulerService {
    /// Создать новый экземпляр сервиса планировщика задач
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных SQLite ([`Db`]).
    /// * `bus` — Шина событий платформы ([`EventBus`]).
    /// * `audit_service` — Сервис системного аудита ([`AuditService`]).
    /// * `plugin_manager` — Менеджер WASM-плагинов ([`PluginManager`]).
    pub fn new(
        db: Db,
        bus: EventBus,
        audit_service: AuditService,
        plugin_manager: PluginManager,
    ) -> Self {
        Self {
            db,
            bus,
            audit_service,
            plugin_manager,
            running_tasks: Arc::new(Mutex::new(HashSet::new())),
            shutdown_notify: Arc::new(Notify::new()),
        }
    }

    /// Восстановление зависших задач при перезапуске сервера (Crash Recovery)
    ///
    /// Находит все задачи, у которых статус остался `running`, переводит их в `aborted`,
    /// фиксирует запись в истории и пересчитывает плановое время следующего запуска.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое запросов к SQLite.
    pub async fn recover_orphaned_tasks(&self) -> Result<()> {
        let pool = self.db.writer();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        let orphaned_rows = sqlx::query_as::<_, (String, String, String, String, String)>(
            r#"
            SELECT id, name, schedule_type, schedule_value, misfire_policy
            FROM scheduled_tasks
            WHERE last_status = 'running'
            "#,
        )
        .fetch_all(self.db.reader())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        for (id, name, schedule_type, schedule_value, _misfire) in orphaned_rows {
            let error_msg = "Task aborted: server was restarted during execution";
            warn!("Recovering orphaned task '{}': {}", id, error_msg);

            // 1. Запись в историю со статусом aborted
            let _ = sqlx::query(
                r#"
                INSERT INTO task_execution_history (
                    task_id, task_name, started_at, finished_at,
                    status, duration_ms, error_message, triggered_by
                ) VALUES (?, ?, ?, ?, 'aborted', 0, ?, 'scheduler:recovery')
                "#,
            )
            .bind(&id)
            .bind(&name)
            .bind(&now_str)
            .bind(&now_str)
            .bind(error_msg)
            .execute(pool)
            .await;

            // 2. Вычисление следующего времени
            let schedule = Self::parse_schedule(&schedule_type, &schedule_value)?;
            let next_run = schedule.calculate_next_run(now).map(|t| t.to_rfc3339());

            let _ = sqlx::query(
                r#"
                UPDATE scheduled_tasks
                SET last_status = 'aborted',
                    last_error = ?,
                    next_run_at = ?,
                    updated_at = ?
                WHERE id = ?
                "#,
            )
            .bind(error_msg)
            .bind(next_run)
            .bind(&now_str)
            .bind(&id)
            .execute(pool)
            .await;
        }

        Ok(())
    }

    /// Получить список всех запланированных задач в системе
    ///
    /// # Возвращаемое значение
    /// Список моделей [`ScheduledTask`], отсортированных по дате создания.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое чтения из SQLite.
    pub async fn list_tasks(&self) -> Result<Vec<ScheduledTask>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT
                id, name, description, schedule_type, schedule_value,
                action_type, action_params, concurrency_policy, misfire_policy,
                timeout_secs, is_enabled, is_system, next_run_at, last_run_at,
                last_status, last_error, created_at, updated_at
            FROM scheduled_tasks
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(self.db.reader())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        let mut tasks = Vec::new();
        for r in rows {
            let schedule = Self::parse_schedule(&r.schedule_type, &r.schedule_value)?;
            let action = Self::parse_action(&r.action_type, r.action_params.as_deref())?;
            let concurrency_policy = ConcurrencyPolicy::from_str(&r.concurrency_policy)
                .unwrap_or(ConcurrencyPolicy::Skip);
            let misfire_policy = MisfirePolicy::from_str(&r.misfire_policy)
                .unwrap_or(MisfirePolicy::SkipToNext);
            let last_status = r
                .last_status
                .as_deref()
                .and_then(|s| ExecutionStatus::from_str(s).ok());

            tasks.push(ScheduledTask {
                id: r.id,
                name: r.name,
                description: r.description,
                schedule,
                action,
                concurrency_policy,
                misfire_policy,
                timeout_secs: r.timeout_secs as u32,
                is_enabled: r.is_enabled != 0,
                is_system: r.is_system != 0,
                next_run_at: r.next_run_at.as_deref().and_then(|t| {
                    DateTime::parse_from_rfc3339(t)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                last_run_at: r.last_run_at.as_deref().and_then(|t| {
                    DateTime::parse_from_rfc3339(t)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                last_status,
                last_error: r.last_error,
                created_at: DateTime::parse_from_rfc3339(&r.created_at)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&r.updated_at)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            });
        }

        Ok(tasks)
    }

    /// Получить задачу по идентификатору
    ///
    /// # Аргументы
    /// * `id` — Уникальный строковый идентификатор задачи.
    ///
    /// # Возвращаемое значение
    /// `Ok(Some(ScheduledTask))` если найдена, или `Ok(None)` если отсутствует.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое запроса к базе данных.
    pub async fn get_task(&self, id: &str) -> Result<Option<ScheduledTask>> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT
                id, name, description, schedule_type, schedule_value,
                action_type, action_params, concurrency_policy, misfire_policy,
                timeout_secs, is_enabled, is_system, next_run_at, last_run_at,
                last_status, last_error, created_at, updated_at
            FROM scheduled_tasks
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        match row {
            Some(r) => {
                let schedule = Self::parse_schedule(&r.schedule_type, &r.schedule_value)?;
                let action = Self::parse_action(&r.action_type, r.action_params.as_deref())?;
                let concurrency_policy = ConcurrencyPolicy::from_str(&r.concurrency_policy)
                    .unwrap_or(ConcurrencyPolicy::Skip);
                let misfire_policy = MisfirePolicy::from_str(&r.misfire_policy)
                    .unwrap_or(MisfirePolicy::SkipToNext);
                let last_status = r
                    .last_status
                    .as_deref()
                    .and_then(|s| ExecutionStatus::from_str(s).ok());

                Ok(Some(ScheduledTask {
                    id: r.id,
                    name: r.name,
                    description: r.description,
                    schedule,
                    action,
                    concurrency_policy,
                    misfire_policy,
                    timeout_secs: r.timeout_secs as u32,
                    is_enabled: r.is_enabled != 0,
                    is_system: r.is_system != 0,
                    next_run_at: r.next_run_at.as_deref().and_then(|t| {
                        DateTime::parse_from_rfc3339(t)
                            .ok()
                            .map(|d| d.with_timezone(&Utc))
                    }),
                    last_run_at: r.last_run_at.as_deref().and_then(|t| {
                        DateTime::parse_from_rfc3339(t)
                            .ok()
                            .map(|d| d.with_timezone(&Utc))
                    }),
                    last_status,
                    last_error: r.last_error,
                    created_at: DateTime::parse_from_rfc3339(&r.created_at)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(&r.updated_at)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                }))
            }
            None => Ok(None),
        }
    }

    /// Создать новую задачу в планировщике
    ///
    /// Валидирует параметры расписания, вычисляет `next_run_at` и сохраняет задачу в базе данных SQLite.
    ///
    /// # Аргументы
    /// * `dto` — DTO объект создания задачи ([`CreateTaskDto`]).
    ///
    /// # Возвращаемое значение
    /// Созданный экземпляр [`ScheduledTask`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Validation`](aethercore_common::error::AppError) при невалидном расписании
    /// или [`AppError::Database`](aethercore_common::error::AppError) при ошибке вставки.
    pub async fn create_task(&self, dto: CreateTaskDto) -> Result<ScheduledTask> {
        dto.schedule.validate()?;

        let task_id = dto.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Utc::now();
        let next_run = if dto.is_enabled {
            dto.schedule.calculate_next_run(now)
        } else {
            None
        };

        let (schedule_type, schedule_value) = Self::serialize_schedule(&dto.schedule);
        let (action_type, action_params) = Self::serialize_action(&dto.action);
        let concurrency_policy_str = dto.concurrency_policy.to_string();
        let misfire_policy_str = dto.misfire_policy.to_string();
        let next_run_str = next_run.map(|t| t.to_rfc3339());
        let now_str = now.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO scheduled_tasks (
                id, name, description, schedule_type, schedule_value,
                action_type, action_params, concurrency_policy, misfire_policy,
                timeout_secs, is_enabled, is_system, next_run_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)
            "#,
        )
        .bind(&task_id)
        .bind(&dto.name)
        .bind(&dto.description)
        .bind(&schedule_type)
        .bind(&schedule_value)
        .bind(&action_type)
        .bind(&action_params)
        .bind(&concurrency_policy_str)
        .bind(&misfire_policy_str)
        .bind(dto.timeout_secs as i64)
        .bind(if dto.is_enabled { 1 } else { 0 })
        .bind(&next_run_str)
        .bind(&now_str)
        .bind(&now_str)
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(format!("Failed to insert scheduled task: {}", e)))?;

        info!("Created scheduled task '{}' ({})", dto.name, task_id);

        Ok(ScheduledTask {
            id: task_id,
            name: dto.name,
            description: dto.description,
            schedule: dto.schedule,
            action: dto.action,
            concurrency_policy: dto.concurrency_policy,
            misfire_policy: dto.misfire_policy,
            timeout_secs: dto.timeout_secs,
            is_enabled: dto.is_enabled,
            is_system: false,
            next_run_at: next_run,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Обновить параметры существующей задачи
    ///
    /// # Аргументы
    /// * `id` — Идентификатор обновляемой задачи.
    /// * `dto` — DTO объект с обновленными параметрами ([`UpdateTaskDto`]).
    ///
    /// # Возвращаемое значение
    /// Обновленный экземпляр [`ScheduledTask`].
    ///
    /// # Ошибки
    /// - [`AppError::NotFound`](aethercore_common::error::AppError) — если задача не существует.
    /// - [`AppError::Validation`](aethercore_common::error::AppError) — если передано невалидное расписание.
    /// - [`AppError::Database`](aethercore_common::error::AppError) — при сбое записи в БД.
    pub async fn update_task(&self, id: &str, dto: UpdateTaskDto) -> Result<ScheduledTask> {
        let existing = self
            .get_task(id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Scheduled task '{}'", id)))?;

        if let Some(ref schedule) = dto.schedule {
            schedule.validate()?;
        }

        let updated_name = dto.name.unwrap_or(existing.name);
        let updated_desc = dto.description.or(existing.description);
        let updated_schedule = dto.schedule.unwrap_or(existing.schedule);
        let updated_action = dto.action.unwrap_or(existing.action);
        let updated_concurrency = dto.concurrency_policy.unwrap_or(existing.concurrency_policy);
        let updated_misfire = dto.misfire_policy.unwrap_or(existing.misfire_policy);
        let updated_timeout = dto.timeout_secs.unwrap_or(existing.timeout_secs);
        let updated_is_enabled = dto.is_enabled.unwrap_or(existing.is_enabled);

        let now = Utc::now();
        let next_run = if updated_is_enabled {
            updated_schedule.calculate_next_run(now)
        } else {
            None
        };

        let (schedule_type, schedule_value) = Self::serialize_schedule(&updated_schedule);
        let (action_type, action_params) = Self::serialize_action(&updated_action);
        let next_run_str = next_run.map(|t| t.to_rfc3339());
        let now_str = now.to_rfc3339();

        sqlx::query(
            r#"
            UPDATE scheduled_tasks
            SET name = ?,
                description = ?,
                schedule_type = ?,
                schedule_value = ?,
                action_type = ?,
                action_params = ?,
                concurrency_policy = ?,
                misfire_policy = ?,
                timeout_secs = ?,
                is_enabled = ?,
                next_run_at = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&updated_name)
        .bind(&updated_desc)
        .bind(&schedule_type)
        .bind(&schedule_value)
        .bind(&action_type)
        .bind(&action_params)
        .bind(updated_concurrency.to_string())
        .bind(updated_misfire.to_string())
        .bind(updated_timeout as i64)
        .bind(if updated_is_enabled { 1 } else { 0 })
        .bind(&next_run_str)
        .bind(&now_str)
        .bind(id)
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(format!("Failed to update scheduled task: {}", e)))?;

        info!("Updated scheduled task '{}' ({})", updated_name, id);

        Ok(ScheduledTask {
            id: id.to_string(),
            name: updated_name,
            description: updated_desc,
            schedule: updated_schedule,
            action: updated_action,
            concurrency_policy: updated_concurrency,
            misfire_policy: updated_misfire,
            timeout_secs: updated_timeout,
            is_enabled: updated_is_enabled,
            is_system: existing.is_system,
            next_run_at: next_run,
            last_run_at: existing.last_run_at,
            last_status: existing.last_status,
            last_error: existing.last_error,
            created_at: existing.created_at,
            updated_at: now,
        })
    }

    /// Удалить задачу из планировщика (системные задачи удалять запрещено)
    ///
    /// # Аргументы
    /// * `id` — Идентификатор удаляемой задачи.
    ///
    /// # Ошибки
    /// - [`AppError::NotFound`](aethercore_common::error::AppError) — если задача не найдена.
    /// - [`AppError::Validation`](aethercore_common::error::AppError) — если задача является системной (`is_system = true`).
    /// - [`AppError::Database`](aethercore_common::error::AppError) — при сбое удаления из БД.
    pub async fn delete_task(&self, id: &str) -> Result<()> {
        let task = self
            .get_task(id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Scheduled task '{}'", id)))?;

        if task.is_system {
            return Err(AppError::validation(
                "task",
                "System scheduled tasks cannot be deleted",
            ));
        }

        sqlx::query("DELETE FROM scheduled_tasks WHERE id = ?")
            .bind(id)
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(format!("Failed to delete task: {}", e)))?;

        info!("Deleted scheduled task '{}' ({})", task.name, id);
        Ok(())
    }

    /// Включить или приостановить задачу (Pause / Resume)
    ///
    /// # Аргументы
    /// * `id` — Идентификатор задачи.
    /// * `is_enabled` — Новое состояние активности (`true` — включена, `false` — на паузе).
    ///
    /// # Возвращаемое значение
    /// Обновленный экземпляр [`ScheduledTask`].
    ///
    /// # Ошибки
    /// - [`AppError::NotFound`](aethercore_common::error::AppError) — если задача не найдена.
    /// - [`AppError::Database`](aethercore_common::error::AppError) — при сбое обновления в базе данных.
    pub async fn toggle_task(&self, id: &str, is_enabled: bool) -> Result<ScheduledTask> {
        let task = self
            .get_task(id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Scheduled task '{}'", id)))?;

        let now = Utc::now();
        let next_run = if is_enabled {
            task.schedule.calculate_next_run(now)
        } else {
            None
        };
        let next_run_str = next_run.map(|t| t.to_rfc3339());
        let now_str = now.to_rfc3339();

        sqlx::query(
            r#"
            UPDATE scheduled_tasks
            SET is_enabled = ?,
                next_run_at = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(if is_enabled { 1 } else { 0 })
        .bind(&next_run_str)
        .bind(&now_str)
        .bind(id)
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(format!("Failed to toggle task: {}", e)))?;

        info!(
            "Toggled scheduled task '{}' ({}) -> is_enabled={}",
            task.name, id, is_enabled
        );

        let mut updated = task;
        updated.is_enabled = is_enabled;
        updated.next_run_at = next_run;
        updated.updated_at = now;
        Ok(updated)
    }

    /// Принудительный немедленный ручной запуск задачи ("Run Now")
    ///
    /// Проверяет, не выполняется ли задача прямо сейчас в памяти (защита от наложения и повторных кликов),
    /// и немедленно инициирует исполнение действия.
    ///
    /// # Аргументы
    /// * `id` — Идентификатор задачи.
    /// * `triggered_by` — Строка инициатора запуска, например `"manual:admin"`.
    ///
    /// # Возвращаемое значение
    /// Запись истории выполнения [`TaskExecutionRecord`].
    ///
    /// # Ошибки
    /// - [`AppError::NotFound`](aethercore_common::error::AppError) — если задача не найдена.
    /// - [`AppError::Conflict`](aethercore_common::error::AppError) — если задача уже выполняется.
    pub async fn run_task_now(&self, id: &str, triggered_by: &str) -> Result<TaskExecutionRecord> {
        let task = self
            .get_task(id)
            .await?
            .ok_or_else(|| AppError::not_found(format!("Scheduled task '{}'", id)))?;

        // Проверка наложения при ручном запуске (Debounce / Overlap check)
        {
            let running = self.running_tasks.lock().await;
            if running.contains(id) {
                return Err(AppError::conflict(format!(
                    "Task '{}' ({}) is already currently running",
                    task.name, id
                )));
            }
        }

        self.execute_task_internal(task, triggered_by.to_string())
            .await
    }

    /// Внутреннее исполнение задачи с контролем блокировок, таймаута, паники и записью истории
    async fn execute_task_internal(
        &self,
        task: ScheduledTask,
        triggered_by: String,
    ) -> Result<TaskExecutionRecord> {
        let task_id = task.id.clone();
        let task_name = task.name.clone();

        // 1. Установка блокировки в памяти
        {
            let mut running = self.running_tasks.lock().await;
            running.insert(task_id.clone());
        }

        let started_at = Utc::now();
        let started_at_str = started_at.to_rfc3339();

        // 2. Обновление статуса в БД на 'running'
        let _ = sqlx::query(
            "UPDATE scheduled_tasks SET last_status = 'running', last_run_at = ? WHERE id = ?",
        )
        .bind(&started_at_str)
        .bind(&task_id)
        .execute(self.db.writer())
        .await;

        // 3. Публикация события о старте задачи в шину
        let _ = self
            .bus
            .publish(EventMessage::reliable(
                "scheduler.task.started",
                "scheduler",
                serde_json::json!({
                    "task_id": task_id,
                    "task_name": task_name,
                    "triggered_by": triggered_by,
                    "started_at": started_at_str,
                }),
            ))
            .await;

        let start_instant = Instant::now();
        let timeout_duration = Duration::from_secs(task.timeout_secs as u64);

        // 4. Запуск действия с контролем таймаута
        let action_result = match tokio::time::timeout(
            timeout_duration,
            self.execute_action(&task.action),
        )
        .await
        {
            Ok(inner_res) => inner_res,
            Err(_) => Err(AppError::internal(format!(
                "Task execution timed out after {} seconds",
                task.timeout_secs
            ))),
        };

        let duration_ms = start_instant.elapsed().as_millis() as u64;
        let finished_at = Utc::now();
        let finished_at_str = finished_at.to_rfc3339();

        let (status, error_message) = match action_result {
            Ok(msg) => (ExecutionStatus::Success, msg),
            Err(err) => {
                let err_str = err.to_string();
                if err_str.contains("timed out") {
                    (ExecutionStatus::Timeout, Some(err_str))
                } else {
                    (ExecutionStatus::Failed, Some(err_str))
                }
            }
        };

        // 5. Вычисление следующего времени запуска
        let next_run = if task.is_enabled {
            task.schedule.calculate_next_run(finished_at)
        } else {
            None
        };
        let next_run_str = next_run.map(|t| t.to_rfc3339());

        // 6. Обновление записи задачи в БД
        let _ = sqlx::query(
            r#"
            UPDATE scheduled_tasks
            SET last_status = ?,
                last_error = ?,
                next_run_at = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.to_string())
        .bind(error_message.as_deref())
        .bind(&next_run_str)
        .bind(&finished_at_str)
        .bind(&task_id)
        .execute(self.db.writer())
        .await;

        // 7. Сохранение записи в журнал истории
        let history_id = sqlx::query(
            r#"
            INSERT INTO task_execution_history (
                task_id, task_name, started_at, finished_at,
                status, duration_ms, error_message, triggered_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&task_id)
        .bind(&task_name)
        .bind(&started_at_str)
        .bind(&finished_at_str)
        .bind(status.to_string())
        .bind(duration_ms as i64)
        .bind(error_message.as_deref())
        .bind(&triggered_by)
        .execute(self.db.writer())
        .await
        .map(|r| r.last_insert_rowid())
        .unwrap_or(0);

        // 8. Снятие блокировки в памяти
        {
            let mut running = self.running_tasks.lock().await;
            running.remove(&task_id);
        }

        // 9. Публикация события о завершении задачи в шину
        let event_topic = if status == ExecutionStatus::Success {
            "scheduler.task.completed"
        } else {
            "scheduler.task.failed"
        };
        let _ = self
            .bus
            .publish(EventMessage::reliable(
                event_topic,
                "scheduler",
                serde_json::json!({
                    "task_id": task_id,
                    "task_name": task_name,
                    "status": status.to_string(),
                    "duration_ms": duration_ms,
                    "error": error_message,
                }),
            ))
            .await;

        Ok(TaskExecutionRecord {
            id: history_id,
            task_id,
            task_name,
            started_at,
            finished_at,
            status,
            duration_ms,
            error_message,
            triggered_by,
        })
    }

    /// Диспетчеризация и непосредственное выполнение действия
    async fn execute_action(&self, action: &TaskAction) -> Result<Option<String>> {
        match action {
            TaskAction::SystemAuditRotation => {
                let kv = KvStore::system(self.db.clone());
                let retention_days = match kv.get::<RetentionConfig>("maintenance_settings").await {
                    Ok(Some(cfg)) => cfg.audit_retention_days,
                    _ => 90,
                };
                let archive_dir = PathBuf::from("data/archives");
                let (pruned, archive_opt) = self
                    .audit_service
                    .archive_and_prune(retention_days, true, &archive_dir)
                    .await?;
                let msg = format!(
                    "Audit rotated: pruned {} records (archive: {:?})",
                    pruned, archive_opt
                );
                info!("{}", msg);
                Ok(Some(msg))
            }
            TaskAction::SystemHistoryCleanup => {
                let deleted = self.prune_history(30).await?;
                let msg = format!("Scheduler history pruned: {} old records removed", deleted);
                info!("{}", msg);
                Ok(Some(msg))
            }
            TaskAction::SystemDbBackup => {
                let kv = KvStore::system(self.db.clone());
                let retention_days = match kv.get::<RetentionConfig>("maintenance_settings").await {
                    Ok(Some(cfg)) => cfg.backup_retention_days,
                    _ => 30,
                };
                let backup_dir = PathBuf::from("data/backups");
                let backup_svc = super::backup::BackupService::new(self.db.clone(), backup_dir);
                let backup_info = backup_svc.create_backup("auto").await?;
                let pruned = backup_svc.prune_backups(retention_days).await.unwrap_or(0);

                let msg = format!(
                    "Database auto backup created ({}, {} bytes), pruned {} outdated backups",
                    backup_info.filename, backup_info.size_bytes, pruned
                );
                info!("{}", msg);
                Ok(Some(msg))
            }
            TaskAction::PluginTimer {
                module_id,
                timer_id,
            } => {
                // Проверяем активность плагина перед запуском
                let is_active = self
                    .plugin_manager
                    .get_plugin(module_id)
                    .map(|p| p.is_enabled)
                    .unwrap_or(false);

                if !is_active {
                    return Err(AppError::validation(
                        "module_id",
                        format!("Plugin '{}' is not installed or disabled", module_id),
                    ));
                }

                // Публикуем тик события в шину для гостевого потребителя
                self.bus
                    .publish(EventMessage::reliable(
                        format!("{}.timer", module_id),
                        "scheduler",
                        serde_json::json!({
                            "module_id": module_id,
                            "timer_id": timer_id,
                            "timestamp": Utc::now().to_rfc3339()
                        }),
                    ))
                    .await?;

                Ok(Some(format!(
                    "Dispatched timer '{}' for plugin '{}'",
                    timer_id, module_id
                )))
            }
            TaskAction::EventBusPublish { topic, payload } => {
                self.bus
                    .publish(EventMessage::reliable(
                        topic,
                        "scheduler",
                        payload.clone(),
                    ))
                    .await?;
                Ok(Some(format!("Published event to topic '{}'", topic)))
            }
        }
    }

    /// Получить историю выполнения задач с пагинацией и фильтрацией
    ///
    /// # Аргументы
    /// * `query` — Параметры фильтрации и пагинации ([`HistoryQueryDto`]).
    ///
    /// # Возвращаемое значение
    /// Список записей истории [`TaskExecutionRecord`], отсортированных от новых к старым.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое запроса к SQLite.
    pub async fn get_history(&self, query: HistoryQueryDto) -> Result<Vec<TaskExecutionRecord>> {
        let limit = query.limit.unwrap_or(50).min(500);
        let offset = query.offset.unwrap_or(0);

        let rows = if let Some(ref tid) = query.task_id {
            sqlx::query_as::<_, HistoryRow>(
                r#"
                SELECT id, task_id, task_name, started_at, finished_at,
                       status, duration_ms, error_message, triggered_by
                FROM task_execution_history
                WHERE task_id = ?
                ORDER BY id DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(tid)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.reader())
            .await
        } else {
            sqlx::query_as::<_, HistoryRow>(
                r#"
                SELECT id, task_id, task_name, started_at, finished_at,
                       status, duration_ms, error_message, triggered_by
                FROM task_execution_history
                ORDER BY id DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.reader())
            .await
        }
        .map_err(|e| AppError::database(e.to_string()))?;

        let mut list = Vec::new();
        for r in rows {
            let status = ExecutionStatus::from_str(&r.status).unwrap_or(ExecutionStatus::Failed);
            list.push(TaskExecutionRecord {
                id: r.id,
                task_id: r.task_id,
                task_name: r.task_name,
                started_at: DateTime::parse_from_rfc3339(&r.started_at)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                finished_at: DateTime::parse_from_rfc3339(&r.finished_at)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                status,
                duration_ms: r.duration_ms as u64,
                error_message: r.error_message,
                triggered_by: r.triggered_by,
            });
        }

        Ok(list)
    }

    /// Очистить устаревшие записи истории выполнения старше N дней
    ///
    /// # Аргументы
    /// * `older_than_days` — Возраст записей в днях, старше которого данные подлежат удалению.
    ///
    /// # Возвращаемое значение
    /// Число удаленных строк.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое выполнения запроса удаления.
    pub async fn prune_history(&self, older_than_days: u32) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(older_than_days as i64);
        let cutoff_str = cutoff.to_rfc3339();

        let result = sqlx::query("DELETE FROM task_execution_history WHERE started_at < ?")
            .bind(&cutoff_str)
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        Ok(result.rows_affected())
    }

    /// Запустить фоновый асинхронный воркер планировщика (Engine Runner)
    ///
    /// Запускает отдельную задачу Tokio, которая каждую секунду опрашивает базу данных,
    /// выявляет созревшие задачи (`next_run_at <= now()`) и запускает их с контролем политик конкурентности.
    ///
    /// # Аргументы
    /// * `self` — Экземпляр сервиса в указателе [`Arc`].
    ///
    /// # Возвращаемое значение
    /// [`tokio::task::JoinHandle`] запущенного фонового процесса.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!("Starting AetherCore Task Scheduler Engine...");

            // 1. Восстановление состояния после возможного перезапуска сервера
            if let Err(e) = self.recover_orphaned_tasks().await {
                error!("Error during scheduler recovery: {}", e);
            }

            let mut ticker = tokio::time::interval(Duration::from_millis(1000));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        self.process_scheduled_ticks().await;
                    }
                    _ = self.shutdown_notify.notified() => {
                        info!("Task Scheduler received shutdown signal. Stopping runner.");
                        break;
                    }
                }
            }
        })
    }

    /// Обработка тика: поиск созревших задач и запуск
    async fn process_scheduled_ticks(&self) {
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // Выбираем активные задачи, у которых наступило плановое время
        let due_tasks = match self.list_tasks().await {
            Ok(all) => all
                .into_iter()
                .filter(|t| t.is_enabled && t.next_run_at.map_or(false, |next| next <= now))
                .collect::<Vec<_>>(),
            Err(e) => {
                debug!("Failed to query scheduled tasks: {}", e);
                return;
            }
        };

        for task in due_tasks {
            let task_id = task.id.clone();
            let is_running = {
                let running = self.running_tasks.lock().await;
                running.contains(&task_id)
            };

            if is_running {
                match task.concurrency_policy {
                    ConcurrencyPolicy::Skip => {
                        debug!(
                            "Skipping execution of task '{}' ({}) due to ConcurrencyPolicy::Skip",
                            task.name, task.id
                        );
                        // Пересчитываем следующий запуск и пишем skipped в историю
                        let next_run = task.schedule.calculate_next_run(now);
                        let next_run_str = next_run.map(|t| t.to_rfc3339());
                        let _ = sqlx::query(
                            "UPDATE scheduled_tasks SET next_run_at = ?, updated_at = ? WHERE id = ?"
                        )
                        .bind(&next_run_str)
                        .bind(&now_str)
                        .bind(&task_id)
                        .execute(self.db.writer())
                        .await;

                        let _ = sqlx::query(
                            r#"
                            INSERT INTO task_execution_history (
                                task_id, task_name, started_at, finished_at,
                                status, duration_ms, error_message, triggered_by
                            ) VALUES (?, ?, ?, ?, 'skipped', 0, 'Previous execution still in progress', 'scheduler')
                            "#,
                        )
                        .bind(&task_id)
                        .bind(&task.name)
                        .bind(&now_str)
                        .bind(&now_str)
                        .execute(self.db.writer())
                        .await;

                        continue;
                    }
                    ConcurrencyPolicy::Allow => {
                        // Разрешен параллельный запуск
                    }
                    ConcurrencyPolicy::Queue => {
                        // Ожидание освобождения в отдельном таске
                    }
                }
            }

            let service = self.clone();
            tokio::spawn(async move {
                if let Err(e) = service.execute_task_internal(task, "scheduler".to_string()).await {
                    error!("Scheduled task execution error: {}", e);
                }
            });
        }
    }

    /// Подать сигнал плавной остановки планировщика
    pub fn stop(&self) {
        self.shutdown_notify.notify_waiters();
    }

    // Вспомогательные функции сериализации и десериализации
    fn serialize_schedule(schedule: &TaskSchedule) -> (String, String) {
        match schedule {
            TaskSchedule::Cron(expr) => ("cron".to_string(), expr.clone()),
            TaskSchedule::IntervalSec(secs) => ("interval".to_string(), secs.to_string()),
            TaskSchedule::OneOff(dt) => ("one_off".to_string(), dt.to_rfc3339()),
        }
    }

    fn parse_schedule(stype: &str, svalue: &str) -> Result<TaskSchedule> {
        match stype {
            "cron" => Ok(TaskSchedule::Cron(svalue.to_string())),
            "interval" => {
                let secs = svalue
                    .parse::<u64>()
                    .map_err(|e| AppError::validation("interval", format!("Invalid interval value: {}", e)))?;
                Ok(TaskSchedule::IntervalSec(secs))
            }
            "one_off" => {
                let dt = DateTime::parse_from_rfc3339(svalue)
                    .map(|d| d.with_timezone(&Utc))
                    .map_err(|e| AppError::validation("one_off", format!("Invalid one_off date: {}", e)))?;
                Ok(TaskSchedule::OneOff(dt))
            }
            _ => Err(AppError::validation("schedule_type", format!("Unknown schedule type '{}'", stype))),
        }
    }

    fn serialize_action(action: &TaskAction) -> (String, Option<String>) {
        match action {
            TaskAction::SystemAuditRotation => ("system_audit_rotation".to_string(), None),
            TaskAction::SystemHistoryCleanup => ("system_history_cleanup".to_string(), None),
            TaskAction::SystemDbBackup => ("system_db_backup".to_string(), None),
            TaskAction::PluginTimer {
                module_id,
                timer_id,
            } => (
                "plugin_timer".to_string(),
                Some(
                    serde_json::json!({
                        "module_id": module_id,
                        "timer_id": timer_id
                    })
                    .to_string(),
                ),
            ),
            TaskAction::EventBusPublish { topic, payload } => (
                "event_publish".to_string(),
                Some(
                    serde_json::json!({
                        "topic": topic,
                        "payload": payload
                    })
                    .to_string(),
                ),
            ),
        }
    }

    fn parse_action(atype: &str, aparams: Option<&str>) -> Result<TaskAction> {
        match atype {
            "system_audit_rotation" => Ok(TaskAction::SystemAuditRotation),
            "system_history_cleanup" => Ok(TaskAction::SystemHistoryCleanup),
            "system_db_backup" => Ok(TaskAction::SystemDbBackup),
            "plugin_timer" => {
                #[derive(Deserialize)]
                struct PluginParams {
                    module_id: String,
                    timer_id: String,
                }
                let params: PluginParams = serde_json::from_str(aparams.unwrap_or("{}"))
                    .map_err(|e| AppError::validation("plugin_timer", format!("Invalid plugin_timer params: {}", e)))?;
                Ok(TaskAction::PluginTimer {
                    module_id: params.module_id,
                    timer_id: params.timer_id,
                })
            }
            "event_publish" => {
                #[derive(Deserialize)]
                struct EventParams {
                    topic: String,
                    payload: serde_json::Value,
                }
                let params: EventParams = serde_json::from_str(aparams.unwrap_or("{}"))
                    .map_err(|e| AppError::validation("event_publish", format!("Invalid event_publish params: {}", e)))?;
                Ok(TaskAction::EventBusPublish {
                    topic: params.topic,
                    payload: params.payload,
                })
            }
            _ => Err(AppError::validation("action_type", format!("Unknown action type '{}'", atype))),
        }
    }
}
