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
    /// Запросить ли сохраненные Retained-состояния топиков при подключении (`?retained=true`)
    pub retained: Option<bool>,
}

/// Входящая управляющая команда от WebSocket клиента
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WsClientCommand {
    /// Добавить подписку на темы
    Subscribe {
        /// Список топиков или масок подписки
        topics: Vec<String>,
        /// Запросить ли сохраненные Retained-состояния топиков
        #[serde(default)]
        with_retained: bool,
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

    let send_retained = query.retained.unwrap_or(false);

    ws.on_upgrade(move |socket| handle_socket(socket, state, initial_topics, send_retained))
}

async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    initial_topics: Vec<String>,
    send_retained: bool,
) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));

    // Инициализируем RAII-подписку
    let mut subscription = if initial_topics.is_empty() {
        state.bus.subscribe() // Подписка на всё по умолчанию
    } else {
        let topic_refs: Vec<&str> = initial_topics.iter().map(|s| s.as_str()).collect();
        state.bus.subscribe_topics(&topic_refs)
    };
    let sub_id = subscription.id();

    debug!("WebSocket client connected to /ws/events (sub_id: {})", sub_id);

    // Если запрошены сохраненные retained-состояния при подключении
    if send_retained {
        let mut initial_retained = Vec::new();
        if initial_topics.is_empty() {
            initial_retained.extend(state.bus.get_retained("*", 50));
        } else {
            for top in &initial_topics {
                initial_retained.extend(state.bus.get_retained(top, 20));
            }
        }
        for ev in initial_retained {
            if let Ok(json_str) = serde_json::to_string(&ev) {
                let mut guard = sender.lock().await;
                let _ = guard.send(Message::Text(json_str.into())).await;
            }
        }
    }

    let send_tx = sender.clone();
    // Задача отправки событий из шины клиенту
    let send_task = tokio::spawn(async move {
        while let Some(event) = subscription.recv().await {
            // Если есть бинарный payload и нет JSON payload — отправляем бинарный кадр
            if let Some(ref bin) = event.binary_payload {
                if event.payload.is_null() {
                    let mut guard = send_tx.lock().await;
                    if guard.send(Message::Binary(bin.clone().into())).await.is_err() {
                        break;
                    }
                    continue;
                }
            }

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
                            WsClientCommand::Subscribe { topics, with_retained } => {
                                for topic in &topics {
                                    state.bus.add_subscription_topic(sub_id, topic);
                                    if with_retained {
                                        for ev in state.bus.get_retained(topic, 20) {
                                            if let Ok(json_str) = serde_json::to_string(&ev) {
                                                let mut guard = recv_tx.lock().await;
                                                let _ = guard.send(Message::Text(json_str.into())).await;
                                            }
                                        }
                                    }
                                }
                                let reply = WsServerMessage::Subscribed { topics };
                                if let Ok(reply_json) = serde_json::to_string(&reply) {
                                    let mut guard = recv_tx.lock().await;
                                    let _ = guard.send(Message::Text(reply_json.into())).await;
                                }
                            }
                            WsClientCommand::Unsubscribe { topics } => {
                                for topic in &topics {
                                    state.bus.remove_subscription_topic(sub_id, topic);
                                }
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
