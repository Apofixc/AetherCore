//! # Эндпоинты надежного журнала событий (`/api/v1/events`)
//!
//! Предоставляет HTTP эндпоинты для выборки исторических событий из журнала SQLite WAL
//! с поддержкой фильтрации по топикам и постраничной пагинации по ID, а также метрик шины.

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use aethercore_common::error::ErrorResponse;
use aethercore_common::models::events::{EventMessage, EventPriority, EventType, ReliableEventRecord};
use aethercore_core::bus::BusStats;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Создать вложенный роутер системных событий `/events`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(query_events_handler))
        .route("/stats", get(get_bus_stats_handler))
        .route("/publish", post(publish_event_handler))
}

/// Тело запроса на публикацию события через REST API
#[derive(Debug, Deserialize)]
pub struct PublishEventRequest {
    /// Топик события (например, `"devices.switch1.command"`)
    pub topic: String,
    /// Тип доставки (Telemetry или Reliable)
    #[serde(default)]
    pub event_type: EventType,
    /// Приоритет сообщения
    #[serde(default)]
    pub priority: EventPriority,
    /// Полезная нагрузка события
    pub payload: serde_json::Value,
    /// Опциональный бизнес-ключ дедупликации
    #[serde(default)]
    pub dedup_key: Option<String>,
    /// Флаг сохранения последнего состояния в Retained Store
    #[serde(default)]
    pub retain: bool,
}

/// Ответ на успешную публикацию события
#[derive(Debug, Serialize)]
pub struct PublishEventResponse {
    /// Статус публикации
    pub status: String,
    /// Сгенерированный или переданный UUID события
    pub event_id: Uuid,
}

/// POST /api/v1/events/publish
///
/// Публикует произвольное событие в шину от имени авторизованного пользователя.
async fn publish_event_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    auth: AuthUser,
    Json(req): Json<PublishEventRequest>,
) -> Result<Json<PublishEventResponse>, (StatusCode, Json<ErrorResponse>)> {
    let source = format!("user:{}", auth.0.username);
    let mut msg = match req.event_type {
        EventType::Telemetry => EventMessage::telemetry(req.topic, source, req.payload),
        EventType::Reliable => EventMessage::reliable(req.topic, source, req.payload),
    };

    msg = msg.with_priority(req.priority).with_retain(req.retain);
    if let Some(key) = req.dedup_key {
        msg = msg.with_dedup_key(key);
    }

    let event_id = msg.id;

    state.bus.publish(msg).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(PublishEventResponse {
        status: "published".to_string(),
        event_id,
    }))
}

/// Параметры фильтрации и пагинации журнала событий
#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    /// Опциональный фильтр по префиксу топика (например, `"users."` или `"system.started"`)
    pub topic: Option<String>,
    /// ID последней прочитанной записи (курсор) для постраничной пагинации
    pub after_id: Option<i64>,
    /// Максимальное количество записей в ответе (по умолчанию 100, максимум 1000)
    pub limit: Option<u32>,
}

/// GET /api/v1/events
///
/// Извлекает исторические записи системных событий из таблицы `event_journal` SQLite WAL.
async fn query_events_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    _auth: AuthUser,
    Query(query): Query<EventsQuery>,
) -> Result<Json<Vec<ReliableEventRecord>>, (StatusCode, Json<ErrorResponse>)> {
    let limit = query.limit.unwrap_or(100);
    let events = state
        .bus
        .query_journal(query.topic.as_deref(), query.after_id, limit)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(events))
}

/// GET /api/v1/events/stats
///
/// Возвращает метрики производительности и состояние очередей событийной шины.
async fn get_bus_stats_handler(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Json<BusStats> {
    Json(state.bus.stats())
}
