//! # WebSocket Gateway трансляции событий шины реального времени

use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use tracing::debug;

/// Query-параметры подключения к WebSocket потоку событий
#[derive(Debug, Deserialize)]
pub struct WsAuthQuery {
    /// Опциональный JWT токен доступа (`?token=<jwt>`)
    pub token: Option<String>,
}

/// WebSocket обработчик подключения клиентов (`/ws/events`)
///
/// Транслирует все события системной шины в реальном времени в формате JSON.
pub async fn ws_events_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<WsAuthQuery>,
) -> impl IntoResponse {
    // Валидация токена доступа (если задан)
    if let Some(token) = &query.token {
        if state.jwt_manager.verify_token(token).is_err() {
            return axum::http::StatusCode::UNAUTHORIZED.into_response();
        }
    }

    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut bus_rx = state.bus.subscribe();

    debug!("WebSocket client connected to /ws/events");

    // Задача отправки событий из шины клиенту
    let send_task = tokio::spawn(async move {
        while let Ok(event) = bus_rx.recv().await {
            if let Ok(json_str) = serde_json::to_string(&event) {
                if sender.send(Message::Text(json_str.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Задача чтения входящих сообщений (heartbeat / ping)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    debug!("WebSocket client disconnected from /ws/events");
}
