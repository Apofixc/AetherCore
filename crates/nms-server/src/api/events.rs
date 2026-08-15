//! # Эндпоинты журнала событий (/api/v1/events)

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use nms_common::error::ErrorResponse;
use nms_common::models::events::ReliableEventRecord;
use serde::Deserialize;

/// Создать вложенный роутер системных событий `/events`
pub fn router() -> Router<AppState> {
    Router::new().route("/", get(query_events_handler))
}

/// Параметры фильтрации и пагинации журнала событий
#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    /// Фильтр по префиксу или точному имени топика
    pub topic: Option<String>,
    /// ID последнего прочитанного события (для пагинации)
    pub after_id: Option<i64>,
    /// Максимальное количество записей в ответе (по умолчанию 100)
    pub limit: Option<u32>,
}

/// GET /api/v1/events
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
