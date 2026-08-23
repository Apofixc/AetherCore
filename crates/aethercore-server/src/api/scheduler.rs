//! # Эндпоинты управления планировщиком задач (`/api/v1/system/scheduler`)
//!
//! Предоставляет REST API для:
//! - Получения списка и детальной информации о задачах (`GET /tasks`, `GET /tasks/{id}`).
//! - Создания, редактирования и удаления задач (`POST /tasks`, `PUT /tasks/{id}`, `DELETE /tasks/{id}`).
//! - Ручного немедленного запуска задач (`POST /tasks/{id}/run`).
//! - Включения/паузы задач (`POST /tasks/{id}/toggle`).
//! - Чтения и очистки истории выполнения (`GET /tasks/{id}/history`, `GET /history`, `DELETE /history`).

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use aethercore_common::error::{AppError, ErrorResponse};
use aethercore_common::models::scheduler::{
    CreateTaskDto, HistoryQueryDto, ScheduledTask, TaskExecutionRecord, UpdateTaskDto,
};
use aethercore_core::auth::check_permission;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

/// Создать вложенный роутер планировщика задач `/scheduler`
///
/// Маршрутизирует запросы управления задачами и журналом выполнения.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(list_tasks_handler).post(create_task_handler))
        .route(
            "/tasks/{id}",
            get(get_task_handler)
                .put(update_task_handler)
                .delete(delete_task_handler),
        )
        .route("/tasks/{id}/run", post(run_task_handler))
        .route("/tasks/{id}/toggle", post(toggle_task_handler))
        .route("/tasks/{id}/history", get(task_history_handler))
        .route("/history", get(all_history_handler).delete(prune_history_handler))
}

fn map_err(e: AppError, locale: aethercore_common::i18n::Locale) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(e.to_api_response(locale)),
    )
}

/// GET /api/v1/system/scheduler/tasks
///
/// Получить список всех зарегистрированных в системе задач
async fn list_tasks_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> ApiResult<Vec<ScheduledTask>> {
    check_permission(&claims, "system.view").map_err(|e| map_err(e, locale))?;

    let tasks = state
        .scheduler_service
        .list_tasks()
        .await
        .map_err(|e| map_err(e, locale))?;

    Ok(Json(tasks))
}

/// GET /api/v1/system/scheduler/tasks/{id}
///
/// Получить детальную информацию о конкретной задаче
async fn get_task_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<ScheduledTask> {
    check_permission(&claims, "system.view").map_err(|e| map_err(e, locale))?;

    let task = state
        .scheduler_service
        .get_task(&id)
        .await
        .map_err(|e| map_err(e, locale))?
        .ok_or_else(|| map_err(AppError::not_found(format!("Task '{}'", id)), locale))?;

    Ok(Json(task))
}

/// POST /api/v1/system/scheduler/tasks
///
/// Создать новую пользовательскую задачу в планировщике
async fn create_task_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(dto): Json<CreateTaskDto>,
) -> Result<(StatusCode, Json<ScheduledTask>), (StatusCode, Json<ErrorResponse>)> {
    check_permission(&claims, "system.manage").map_err(|e| map_err(e, locale))?;

    let task = state
        .scheduler_service
        .create_task(dto)
        .await
        .map_err(|e| map_err(e, locale))?;

    let user_id = claims.sub.to_string();
    let _ = state
        .audit_service
        .log(
            Some(&user_id),
            Some(&claims.username),
            "scheduler.task.create",
            &format!("task:{}", task.id),
            "success",
            Some(&serde_json::json!({ "name": task.name }).to_string()),
            None,
        )
        .await;

    Ok((StatusCode::CREATED, Json(task)))
}

/// PUT /api/v1/system/scheduler/tasks/{id}
///
/// Обновить параметры существующей задачи
async fn update_task_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(dto): Json<UpdateTaskDto>,
) -> ApiResult<ScheduledTask> {
    check_permission(&claims, "system.manage").map_err(|e| map_err(e, locale))?;

    let task = state
        .scheduler_service
        .update_task(&id, dto)
        .await
        .map_err(|e| map_err(e, locale))?;

    let user_id = claims.sub.to_string();
    let _ = state
        .audit_service
        .log(
            Some(&user_id),
            Some(&claims.username),
            "scheduler.task.update",
            &format!("task:{}", id),
            "success",
            Some(&serde_json::json!({ "name": task.name }).to_string()),
            None,
        )
        .await;

    Ok(Json(task))
}

/// DELETE /api/v1/system/scheduler/tasks/{id}
///
/// Удалить задачу из планировщика
async fn delete_task_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    check_permission(&claims, "system.manage").map_err(|e| map_err(e, locale))?;

    state
        .scheduler_service
        .delete_task(&id)
        .await
        .map_err(|e| map_err(e, locale))?;

    let user_id = claims.sub.to_string();
    let _ = state
        .audit_service
        .log(
            Some(&user_id),
            Some(&claims.username),
            "scheduler.task.delete",
            &format!("task:{}", id),
            "success",
            None,
            None,
        )
        .await;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/v1/system/scheduler/tasks/{id}/run
///
/// Принудительный немедленный ручной запуск задачи ("Run Now")
async fn run_task_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<TaskExecutionRecord> {
    check_permission(&claims, "system.manage").map_err(|e| map_err(e, locale))?;

    let triggered_by = format!("manual:{}", claims.username);
    let record = state
        .scheduler_service
        .run_task_now(&id, &triggered_by)
        .await
        .map_err(|e| map_err(e, locale))?;

    let user_id = claims.sub.to_string();
    let _ = state
        .audit_service
        .log(
            Some(&user_id),
            Some(&claims.username),
            "scheduler.task.manual_run",
            &format!("task:{}", id),
            &record.status.to_string(),
            Some(
                &serde_json::json!({
                    "duration_ms": record.duration_ms,
                    "error": record.error_message
                })
                .to_string(),
            ),
            None,
        )
        .await;

    Ok(Json(record))
}

#[derive(Debug, Deserialize)]
struct ToggleDto {
    is_enabled: bool,
}

/// POST /api/v1/system/scheduler/tasks/{id}/toggle
///
/// Включение или пауза задачи
async fn toggle_task_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(dto): Json<ToggleDto>,
) -> ApiResult<ScheduledTask> {
    check_permission(&claims, "system.manage").map_err(|e| map_err(e, locale))?;

    let task = state
        .scheduler_service
        .toggle_task(&id, dto.is_enabled)
        .await
        .map_err(|e| map_err(e, locale))?;

    let user_id = claims.sub.to_string();
    let _ = state
        .audit_service
        .log(
            Some(&user_id),
            Some(&claims.username),
            "scheduler.task.toggle",
            &format!("task:{}", id),
            "success",
            Some(&serde_json::json!({ "is_enabled": dto.is_enabled }).to_string()),
            None,
        )
        .await;

    Ok(Json(task))
}

/// GET /api/v1/system/scheduler/tasks/{id}/history
///
/// Получить историю запусков конкретной задачи
async fn task_history_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(query): Query<HistoryQueryDto>,
) -> ApiResult<Vec<TaskExecutionRecord>> {
    check_permission(&claims, "system.view").map_err(|e| map_err(e, locale))?;

    let mut q = query;
    q.task_id = Some(id);
    let history = state
        .scheduler_service
        .get_history(q)
        .await
        .map_err(|e| map_err(e, locale))?;

    Ok(Json(history))
}

/// GET /api/v1/system/scheduler/history
///
/// Получить общую историю запусков всех задач
async fn all_history_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(query): Query<HistoryQueryDto>,
) -> ApiResult<Vec<TaskExecutionRecord>> {
    check_permission(&claims, "system.view").map_err(|e| map_err(e, locale))?;

    let history = state
        .scheduler_service
        .get_history(query)
        .await
        .map_err(|e| map_err(e, locale))?;

    Ok(Json(history))
}

#[derive(Debug, Deserialize)]
struct PruneHistoryDto {
    #[serde(default = "default_prune_days")]
    days: u32,
}

fn default_prune_days() -> u32 {
    30
}

#[derive(Debug, Serialize)]
struct PruneHistoryResponse {
    success: bool,
    deleted_count: u64,
}

/// DELETE /api/v1/system/scheduler/history
///
/// Очистить историю выполнения задач старше N дней
async fn prune_history_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(dto): Query<PruneHistoryDto>,
) -> ApiResult<PruneHistoryResponse> {
    check_permission(&claims, "system.manage").map_err(|e| map_err(e, locale))?;

    let deleted = state
        .scheduler_service
        .prune_history(dto.days)
        .await
        .map_err(|e| map_err(e, locale))?;

    let user_id = claims.sub.to_string();
    let _ = state
        .audit_service
        .log(
            Some(&user_id),
            Some(&claims.username),
            "scheduler.history.prune",
            "scheduler_history",
            "success",
            Some(&serde_json::json!({ "days": dto.days, "deleted": deleted }).to_string()),
            None,
        )
        .await;

    Ok(Json(PruneHistoryResponse {
        success: true,
        deleted_count: deleted,
    }))
}
