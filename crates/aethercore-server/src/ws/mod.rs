//! # WebSocket Gateway трансляции событий шины реального времени (`/ws/events`)
//!
//! Обеспечивает двунаправленное подключение браузеров и внешних систем для
//! получения событий шины (`EventMessage`) в реальном времени с поддержкой динамических
//! подписок по топикам и маскам (`*`, `#`).

use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, trace};

/// Query-параметры подключения к WebSocket потоку событий
#[derive(Debug, Deserialize)]
pub struct WsAuthQuery {
    /// Опциональный JWT токен доступа (`?token=<jwt>`)
    pub token: Option<String>,
    /// Начальные темы подписки через запятую (например, `?topics=devices.*,system.#`)
    pub topics: Option<String>,
}

/// Входящая управляющая команда от WebSocket клиента
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WsClientCommand {
    /// Добавить подписку на темы
    Subscribe {
        /// Список топиков или масок подписки
        topics: Vec<String>,
    },
    /// Удалить подписку на темы
    Unsubscribe {
        /// Список топиков или масок для отписки
        topics: Vec<String>,
    },
    /// Heartbeat пинг
    Ping,
}

/// Исходящее системное сообщение WebSocket шлюза
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsServerMessage {
    /// Ответ на пинг
    Pong,
    /// Подтверждение успешной подписки
    Subscribed {
        /// Подписанные темы
        topics: Vec<String>,
    },
    /// Подтверждение отписки
    Unsubscribed {
        /// Отписанные темы
        topics: Vec<String>,
    },
}

/// WebSocket обработчик подключения клиентов (`/ws/events`)
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

    let initial_topics: Vec<String> = query
        .topics
        .as_deref()
        .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    ws.on_upgrade(move |socket| handle_socket(socket, state, initial_topics))
}

async fn handle_socket(socket: WebSocket, state: AppState, initial_topics: Vec<String>) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));

    // Инициализируем RAII-подписку
    let mut subscription = if initial_topics.is_empty() {
        state.bus.subscribe() // Подписка на всё по умолчанию
    } else {
        let topic_refs: Vec<&str> = initial_topics.iter().map(|s| s.as_str()).collect();
        state.bus.subscribe_topics(&topic_refs)
    };

    debug!("WebSocket client connected to /ws/events (sub_id: {})", subscription.id());

    let send_tx = sender.clone();
    // Задача отправки событий из шины клиенту
    let send_task = tokio::spawn(async move {
        while let Some(event) = subscription.recv().await {
            if let Ok(json_str) = serde_json::to_string(&event) {
                let mut guard = send_tx.lock().await;
                if guard.send(Message::Text(json_str.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    let recv_tx = sender.clone();
    // Задача чтения входящих управляющих команд от клиента
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(cmd) = serde_json::from_str::<WsClientCommand>(&text) {
                        match cmd {
                            WsClientCommand::Subscribe { topics } => {
                                for topic in &topics {
                                    state.bus.subscribe_topic(topic); // router dinamically handles
                                }
                                let reply = WsServerMessage::Subscribed { topics };
                                if let Ok(reply_json) = serde_json::to_string(&reply) {
                                    let mut guard = recv_tx.lock().await;
                                    let _ = guard.send(Message::Text(reply_json.into())).await;
                                }
                            }
                            WsClientCommand::Unsubscribe { topics } => {
                                let reply = WsServerMessage::Unsubscribed { topics };
                                if let Ok(reply_json) = serde_json::to_string(&reply) {
                                    let mut guard = recv_tx.lock().await;
                                    let _ = guard.send(Message::Text(reply_json.into())).await;
                                }
                            }
                            WsClientCommand::Ping => {
                                let reply = WsServerMessage::Pong;
                                if let Ok(reply_json) = serde_json::to_string(&reply) {
                                    let mut guard = recv_tx.lock().await;
                                    let _ = guard.send(Message::Text(reply_json.into())).await;
                                }
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                Message::Ping(data) => {
                    let mut guard = recv_tx.lock().await;
                    let _ = guard.send(Message::Pong(data)).await;
                }
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    trace!("WebSocket client disconnected from /ws/events");
}
