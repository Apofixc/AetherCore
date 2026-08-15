// REST API и WebSocket эндпоинты истории системных событий (1-в-1 с backend/api/events.py)

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::db::get_missed_events_from_db;
use crate::exceptions::NmsError;
use crate::server::AppState;

pub const MAX_FRAME_SIZE: usize = 65536; // 64 KB
pub const MAX_MESSAGES_PER_SECOND: usize = 50;
pub const MAX_JSON_ERRORS: usize = 5;

/// Динамическая проверка прав доступа пользователя к топику WebSocket (1-в-1 с Python can_subscribe_to_topic)
pub async fn can_subscribe_to_topic(
    pool: Option<&sqlx::SqlitePool>,
    user_id: Option<&str>,
    topic: &str,
    auth_enabled: bool,
) -> bool {
    if topic.is_empty() {
        return false;
    }
    if !auth_enabled {
        return true;
    }
    let uid = match user_id {
        Some(u) => u,
        None => return false,
    };
    if let Some(pool) = pool {
        if crate::auth::user_has_permission(pool, uid, "system.admin").await
            || crate::auth::user_has_permission(pool, uid, "system.all").await
        {
            return true;
        }
        let topic_str = topic.trim();
        let base_name = topic_str
            .split('.')
            .next()
            .unwrap_or("")
            .split('_')
            .next()
            .unwrap_or("");

        if crate::auth::user_has_permission(pool, uid, topic_str).await
            || crate::auth::user_has_permission(pool, uid, &format!("{}.view", base_name)).await
        {
            return true;
        }

        let protected_resources = [
            "audit", "logs", "users", "roles", "system", "admin", "security", "core",
        ];
        if protected_resources.contains(&base_name)
            || protected_resources.contains(&topic_str)
            || topic_str.starts_with("core.")
        {
            return false;
        }

        true
    } else {
        true
    }
}

/// Информационный эндпоинт реального времени (1-в-1 с Python get_events_info)
pub async fn get_events_info_handler() -> Json<Value> {
    Json(json!({
        "status": "online",
        "transport": "websocket",
        "ws_url": "/api/v1/ws/events",
        "message": "Real-time system events channel via WebSockets",
    }))
}

/// Извлечение токена и согласование subprotocol из заголовков или query (1-в-1 с Python _extract_token_and_subprotocol)
pub fn extract_token_and_subprotocol(
    subprotocol_header: Option<&str>,
    token_query: Option<&str>,
) -> (Option<String>, Option<String>) {
    let mut accepted_subprotocol = None;

    if let Some(header) = subprotocol_header {
        let parts: Vec<&str> = header.split(',').map(|s| s.trim()).collect();
        for (i, part) in parts.iter().enumerate() {
            if part.eq_ignore_ascii_case("bearer") {
                accepted_subprotocol = Some("bearer".to_string());
                if i + 1 < parts.len() {
                    return (Some(parts[i + 1].to_string()), accepted_subprotocol);
                }
            } else if part.to_lowercase().starts_with("bearer.") {
                accepted_subprotocol = Some("bearer".to_string());
                let token = part.splitn(2, '.').nth(1).unwrap_or("");
                return (Some(token.to_string()), accepted_subprotocol);
            }
        }
    }

    if let Some(token) = token_query {
        return (Some(token.to_string()), accepted_subprotocol);
    }

    (None, accepted_subprotocol)
}

/// Запрос истории событий
#[derive(Debug, Deserialize, Default)]
pub struct EventsHistoryQuery {
    pub from_seq_id: Option<i64>,
    pub topic: Option<String>,
    pub user_id: Option<String>,
    pub limit: Option<i64>,
}

/// Получить сохраненные события из журнала БД
pub async fn get_events_history_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<EventsHistoryQuery>,
) -> Result<Json<Value>, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    let from_seq = query.from_seq_id.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);
    let user_id = query.user_id.as_deref();

    let events = get_missed_events_from_db(pool, from_seq, user_id, query.topic.as_deref(), limit)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(json!(events)))
}
