// REST API эндпоинты системы уведомлений (Notifications Router)

use axum::{
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::exceptions::NmsError;
use crate::notify::{NotificationFilter, SetPreferencesInput};
use crate::server::AppState;

/// Запрос фильтрации уведомлений
#[derive(Debug, Deserialize, Default)]
pub struct NotifQuery {
    pub user_id: Option<String>,
    pub unread_only: Option<bool>,
    pub severity: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Получение списка уведомлений пользователя
pub async fn list_notifications_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<NotifQuery>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let target_user = query.user_id.clone().unwrap_or_else(|| "root".to_string());
    let filter = NotificationFilter {
        unread_only: query.unread_only,
        severity: query.severity,
        category: query.category,
        search: query.search,
        limit: query.limit,
        offset: query.offset,
    };

    let result = engine
        .get_user_notifications(&target_user, &filter)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!(result)))
}

/// Отметка одного уведомления как прочитанного
pub async fn mark_read_handler(
    State(state): State<Arc<AppState>>,
    Path(notif_id): Path<i64>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let ok = engine
        .mark_as_read(notif_id, "root")
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!({ "status": "ok", "success": ok })))
}

/// Отметка всех уведомлений как прочитанных
pub async fn mark_all_read_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let count = engine
        .mark_all_as_read("root")
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!({ "status": "ok", "updated_count": count })))
}

/// Квитирование конкретного алерта
pub async fn acknowledge_handler(
    State(state): State<Arc<AppState>>,
    Path(notif_id): Path<i64>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let ok = engine
        .acknowledge_notification(notif_id, "root")
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!({ "status": "ok", "success": ok })))
}

/// Получение предпочтений тишины и фильтрации пользователя
pub async fn get_preferences_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let prefs = engine
        .get_notification_preferences("root")
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!(prefs)))
}

/// Обновление предпочтений тишины пользователя
pub async fn set_preferences_handler(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SetPreferencesInput>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let prefs = engine
        .set_notification_preferences("root", input)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!(prefs)))
}

/// Экспорт уведомлений в форматах CSV / JSON
pub async fn export_notifications_handler(
    State(state): State<Arc<AppState>>,
    Query(format_opt): Query<Option<String>>,
) -> Result<impl IntoResponse, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let fmt = format_opt.as_deref().unwrap_or("json");
    let (content, filename) = engine
        .export_user_notifications("root", fmt, &NotificationFilter::default())
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    let media_type = if fmt == "csv" {
        "text/csv"
    } else {
        "application/json"
    };
    let disposition = format!("attachment; filename=\"{}\"", filename);

    Ok((
        [
            (header::CONTENT_TYPE, media_type.to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        content,
    ))
}

/// Получить список поддерживаемых категорий уведомлений
pub async fn list_categories_handler() -> Result<Json<Value>, NmsError> {
    let categories = crate::notify::get_notification_categories();
    Ok(Json(json!(categories)))
}

/// Получить список всех модулей системы для подписки
pub async fn list_modules_handler() -> Result<Json<Value>, NmsError> {
    let modules = crate::notify::get_notification_modules();
    Ok(Json(json!(modules)))
}

/// Пометить конкретное уведомление как непрочитанное
pub async fn unread_notification_handler(
    State(state): State<Arc<AppState>>,
    Path(notif_id): Path<i64>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let ok = engine
        .mark_as_unread(notif_id, "root")
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(
        json!({ "status": "success", "id": notif_id, "success": ok }),
    ))
}

/// Квитировать все неквитированные уведомления текущего пользователя
pub async fn acknowledge_all_user_notifications_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let count = engine
        .acknowledge_all_notifications("root")
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(
        json!({ "status": "success", "acknowledged_count": count }),
    ))
}

/// Удалить все прочитанные уведомления пользователя
pub async fn delete_all_read_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let count = engine
        .clear_read_notifications("root")
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!({ "status": "success", "deleted_count": count })))
}

/// Очистить уведомления старше указанного количества дней
pub async fn prune_stale_notifications_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let days = params.get("days").and_then(|v| v.as_i64()).unwrap_or(30);
    let count = engine
        .prune_notifications(days)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!({ "status": "success", "pruned_count": count })))
}

/// Проверить и эскалировать просроченные критические уведомления
pub async fn trigger_alert_escalations_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<serde_json::Value>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let minutes = params
        .get("escalation_minutes")
        .and_then(|v| v.as_i64())
        .unwrap_or(15);
    let count = engine
        .process_alert_escalations(minutes)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(
        json!({ "status": "success", "escalated_count": count }),
    ))
}

/// Удалить одно конкретное уведомление
pub async fn remove_notification_handler(
    State(state): State<Arc<AppState>>,
    Path(notif_id): Path<i64>,
) -> Result<Json<Value>, NmsError> {
    let engine = state
        .notification_engine
        .as_ref()
        .ok_or_else(|| NmsError::Internal {
            message: "Notification engine unavailable".to_string(),
            details: json!({}),
        })?;

    let ok = engine
        .delete_notification(notif_id, "root")
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(
        json!({ "status": "success", "id": notif_id, "success": ok }),
    ))
}
