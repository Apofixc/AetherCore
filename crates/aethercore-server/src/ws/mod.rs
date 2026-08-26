//! # WebSocket Gateway трансляции событий шины реального времени (`/ws/events`)
//!
//! Обеспечивает двунаправленное подключение браузеров и внешних систем для
//! получения событий шины ([`EventMessage`]) в реальном времени, вызова REST-over-WS,
//! управления динамическими подписками, поддержки In-Band авторизации и форматов JSON/MessagePack.

pub mod session;
pub mod types;

pub use session::WsSession;
pub use types::{WsAuthQuery, WsClientCommand, WsCodecFormat, WsServerMessage};

use crate::middleware::{extract_client_ip, is_ip_allowed, AuthUser};
use crate::state::AppState;
use aethercore_common::models::events::{EventMessage, EventType};
use aethercore_common::models::user::JwtClaims;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};

/// WebSocket обработчик подключения клиентов (`/ws/events`)
pub async fn ws_events_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WsAuthQuery>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(&headers);

    // Проверка белого списка IP-адресов
    if let Ok(Some(policies)) = aethercore_core::db::kv::KvStore::system(state.db.clone())
        .get::<aethercore_common::models::user::SecurityPoliciesDto>("security_policies")
        .await
    {
        if !is_ip_allowed(&client_ip, &policies.ip_whitelist) {
            warn!("WebSocket connection rejected for IP: {}", client_ip);
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    // Согласование субпротокола (JSON или MessagePack)
    let requested_proto = headers
        .get("Sec-WebSocket-Protocol")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let (codec_format, proto_header) = if requested_proto.contains("aethercore.msgpack") {
        (WsCodecFormat::MessagePack, Some("aethercore.msgpack"))
    } else if requested_proto.contains("aethercore.json") {
        (WsCodecFormat::Json, Some("aethercore.json"))
    } else {
        (WsCodecFormat::Json, None)
    };

    // Валидация токена из Query-параметра (если передан)
    let initial_claims = if let Some(token) = &query.token {
        match state.jwt_manager.verify_token(token) {
            Ok(claims) => {
                if let Some(session_id) = claims.session_id {
                    if let Ok(true) = state.session_service.is_session_valid(session_id).await {
                        let _ = state.session_service.touch_session(session_id).await;
                        Some(claims)
                    } else {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                } else {
                    Some(claims)
                }
            }
            Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
        }
    } else {
        None
    };

    let initial_topics: Vec<String> = query
        .topics
        .as_deref()
        .map(|t| {
            t.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let send_retained = query.retained.unwrap_or(false);

    let ws = if let Some(proto) = proto_header {
        ws.protocols([proto])
    } else {
        ws
    };

    ws.on_upgrade(move |socket| {
        handle_socket(
            socket,
            state,
            initial_claims,
            codec_format,
            client_ip,
            initial_topics,
            send_retained,
        )
    })
}

/// Внутренний обработчик установленного WebSocket-соединения
async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    initial_claims: Option<JwtClaims>,
    initial_format: WsCodecFormat,
    client_ip: String,
    initial_topics: Vec<String>,
    send_retained: bool,
) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let session = Arc::new(WsSession::new(initial_claims, initial_format, client_ip));

    // Ограниченный канал отправки клиенту (Backpressure)
    let (tx, mut rx) = mpsc::channel::<WsServerMessage>(256);

    // RAII подписка на шине
    let mut subscription = if initial_topics.is_empty() {
        state.bus.subscribe()
    } else {
        let topic_refs: Vec<&str> = initial_topics.iter().map(|s| s.as_str()).collect();
        state.bus.subscribe_topics(&topic_refs)
    };
    let sub_id = subscription.id();

    debug!(
        "WebSocket client connected to /ws/events (sub_id: {}, format: {:?})",
        sub_id, initial_format
    );

    // Аудит подключения
    if let Some(claims) = session.get_claims().await {
        let user_id_str = claims.sub.to_string();
        let details = serde_json::json!({
            "sub_id": sub_id,
            "format": format!("{:?}", initial_format)
        }).to_string();

        let _ = state.audit_service.log(
            Some(&user_id_str),
            Some(&claims.username),
            "websocket.connected",
            "ws",
            "success",
            Some(&details),
            Some(session.client_ip()),
        ).await;
    }

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
            let seq = session.next_seq();
            let _ = tx.send(WsServerMessage::Event { seq, event: ev }).await;
        }
    }

    // 1. Поток передачи сообщений из канала в физический WebSocket
    let session_out = session.clone();
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let codec = session_out.get_format().await;
            match codec.encode(&msg) {
                Ok(ws_frame) => {
                    if ws_sink.send(ws_frame).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("WebSocket serialization error: {}", e);
                }
            }
        }
    });

    // 2. Серверный Heartbeat поток (Ping каждые 30 секунд)
    let tx_heartbeat = tx.clone();
    let heartbeat_interval_secs = state.config.server.port; // AppConfig fallback
    let ping_interval = Duration::from_secs(30);
    let heartbeat_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(ping_interval);
        loop {
            interval.tick().await;
            // Серверный пинг для отсечения ghost-сокетов
            if tx_heartbeat.send(WsServerMessage::Pong).await.is_err() {
                break;
            }
        }
    });

    // 3. Поток передачи событий из шины EventBus подписчику
    let tx_bus = tx.clone();
    let session_bus = session.clone();
    let bus_state = state.clone();
    let bus_task = tokio::spawn(async move {
        while let Some(event) = subscription.recv().await {
            // Проверка прав пользователя на чтение топика
            if !session_bus.can_read_topic(&event.topic).await {
                continue;
            }

            let seq = session_bus.next_seq();
            let server_msg = WsServerMessage::Event { seq, event: event.clone() };

            // Backpressure: некритичная телеметрия сбрасывается при переполнении очереди
            match tx_bus.try_send(server_msg) {
                Ok(_) => {}
                Err(mpsc::error::TrySendError::Full(msg)) => {
                    if event.event_type == EventType::Telemetry {
                        bus_state.bus.metrics().record_dropped();
                        trace!("Dropped telemetry message for slow WS consumer (sub_id: {})", sub_id);
                    } else {
                        // Надежные события ждут место в очереди
                        if tx_bus.send(msg).await.is_err() {
                            break;
                        }
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    break;
                }
            }
        }
    });

    // 4. Поток приема входящих команд от клиента
    let tx_recv = tx.clone();
    let session_recv = session.clone();
    let state_recv = state.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_stream.next().await {
            let codec = session_recv.get_format().await;

            match msg {
                Message::Close(_) => break,
                Message::Ping(_data) => {
                    let _ = tx_recv.send(WsServerMessage::Pong).await;
                }
                Message::Pong(_) => {
                    // Ответ на серверный пинг получен — сокет живой
                }
                Message::Text(_) | Message::Binary(_) => {
                    match codec.decode::<WsClientCommand>(&msg) {
                        Ok(cmd) => {
                            match cmd {
                                // In-Band Авторизация
                                WsClientCommand::Auth { token } => {
                                    match state_recv.jwt_manager.verify_token(&token) {
                                        Ok(claims) => {
                                            let user_id = claims.sub;
                                            let username = claims.username.clone();
                                            let roles = claims.roles.clone();
                                            let permissions = claims.permissions.clone();

                                            session_recv.set_claims(claims).await;
                                            info!("WebSocket client authenticated as '{}' (sub_id: {})", username, sub_id);

                                            let _ = tx_recv.send(WsServerMessage::Authenticated {
                                                user_id,
                                                username,
                                                roles,
                                                permissions,
                                            }).await;
                                        }
                                        Err(e) => {
                                            let _ = tx_recv.send(WsServerMessage::Error {
                                                code: "UNAUTHORIZED".to_string(),
                                                message: format!("Invalid token: {}", e),
                                                request_id: None,
                                            }).await;
                                        }
                                    }
                                }

                                // Динамическая подписка на топики
                                WsClientCommand::Subscribe { topics, with_retained } => {
                                    let mut accepted_topics = Vec::new();
                                    for topic in &topics {
                                        if session_recv.can_read_topic(topic).await {
                                            state_recv.bus.add_subscription_topic(sub_id, topic);
                                            accepted_topics.push(topic.clone());

                                            if with_retained {
                                                for ev in state_recv.bus.get_retained(topic, 20) {
                                                    let seq = session_recv.next_seq();
                                                    let _ = tx_recv.send(WsServerMessage::Event { seq, event: ev }).await;
                                                }
                                            }
                                        } else {
                                            let _ = tx_recv.send(WsServerMessage::Error {
                                                code: "FORBIDDEN".to_string(),
                                                message: format!("Access denied to subscribe topic '{}'", topic),
                                                request_id: None,
                                            }).await;
                                        }
                                    }

                                    if !accepted_topics.is_empty() {
                                        let _ = tx_recv.send(WsServerMessage::Subscribed { topics: accepted_topics }).await;
                                    }
                                }

                                // Отписка от топиков
                                WsClientCommand::Unsubscribe { topics } => {
                                    for topic in &topics {
                                        state_recv.bus.remove_subscription_topic(sub_id, topic);
                                    }
                                    let _ = tx_recv.send(WsServerMessage::Unsubscribed { topics }).await;
                                }

                                // Публикация события / команды в шину ядра
                                WsClientCommand::Publish { msg_id, tab_id: _, topic, payload, priority, retain } => {
                                    if !session_recv.can_write_topic(&topic).await {
                                        let _ = tx_recv.send(WsServerMessage::Error {
                                            code: "FORBIDDEN".to_string(),
                                            message: format!("Access denied to publish to topic '{}'", topic),
                                            request_id: msg_id,
                                        }).await;
                                        continue;
                                    }

                                    let source = session_recv
                                        .get_claims()
                                        .await
                                        .map(|c| format!("user:{}", c.username))
                                        .unwrap_or_else(|| "anonymous".to_string());

                                    let mut ev = EventMessage::reliable(topic, source, payload)
                                        .with_priority(priority)
                                        .with_retain(retain);

                                    if let Some(ref mid) = msg_id {
                                        ev = ev.with_dedup_key(mid.clone());
                                    }

                                    if let Err(e) = state_recv.bus.publish(ev).await {
                                        let _ = tx_recv.send(WsServerMessage::Error {
                                            code: "PUBLISH_FAILED".to_string(),
                                            message: e.to_string(),
                                            request_id: msg_id,
                                        }).await;
                                    } else if let Some(mid) = msg_id {
                                        let _ = tx_recv.send(WsServerMessage::Ack {
                                            msg_id: mid,
                                            status: "ok".to_string(),
                                        }).await;
                                    }
                                }

                                // Пакетный запрос сохраненных состояний
                                WsClientCommand::GetState { patterns, limit_per_topic } => {
                                    let mut snapshot = Vec::new();
                                    for pat in &patterns {
                                        if session_recv.can_read_topic(pat).await {
                                            snapshot.extend(state_recv.bus.get_retained(pat, limit_per_topic));
                                        }
                                    }
                                    let _ = tx_recv.send(WsServerMessage::StateSnapshot { events: snapshot }).await;
                                }

                                // Вызов REST API через сокет (In-Process Axum Dispatch)
                                WsClientCommand::Call { request_id, tab_id, method, path, body } => {
                                    let claims_opt = session_recv.get_claims().await;
                                    let tx_call = tx_recv.clone();
                                    let state_call = state_recv.clone();

                                    tokio::spawn(async move {
                                        let method_verb = match method.to_uppercase().as_str() {
                                            "GET" => axum::http::Method::GET,
                                            "POST" => axum::http::Method::POST,
                                            "PUT" => axum::http::Method::PUT,
                                            "DELETE" => axum::http::Method::DELETE,
                                            "PATCH" => axum::http::Method::PATCH,
                                            _ => {
                                                let _ = tx_call.send(WsServerMessage::Error {
                                                    code: "BAD_REQUEST".to_string(),
                                                    message: format!("Unsupported HTTP method: {}", method),
                                                    request_id: Some(request_id),
                                                }).await;
                                                return;
                                            }
                                        };

                                        // Формируем виртуальный HTTP-запрос в памяти
                                        let body_str = body.to_string();
                                        let mut req_builder = axum::http::Request::builder()
                                            .method(method_verb)
                                            .uri(&path)
                                            .header(axum::http::header::CONTENT_TYPE, "application/json");

                                        // Внедряем контекст авторизованного пользователя
                                        if let Some(ref claims) = claims_opt {
                                            req_builder = req_builder.extension(AuthUser(claims.clone()));
                                        }

                                        let virtual_req = match req_builder.body(axum::body::Body::from(body_str)) {
                                            Ok(r) => r,
                                            Err(e) => {
                                                let _ = tx_call.send(WsServerMessage::Error {
                                                    code: "INTERNAL_ERROR".to_string(),
                                                    message: format!("Failed to build virtual request: {}", e),
                                                    request_id: Some(request_id),
                                                }).await;
                                                return;
                                            }
                                        };

                                        let router = crate::create_app_router(state_call);
                                        use tower::ServiceExt;

                                        match router.oneshot(virtual_req).await {
                                            Ok(resp) => {
                                                let status = resp.status().as_u16();
                                                let body_bytes = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
                                                    .await
                                                    .unwrap_or_default();

                                                let json_body: serde_json::Value = serde_json::from_slice(&body_bytes)
                                                    .unwrap_or_else(|_| {
                                                        serde_json::Value::String(String::from_utf8_lossy(&body_bytes).to_string())
                                                    });

                                                let _ = tx_call.send(WsServerMessage::Response {
                                                    request_id,
                                                    tab_id,
                                                    status,
                                                    body: json_body,
                                                }).await;
                                            }
                                            Err(e) => {
                                                let _ = tx_call.send(WsServerMessage::Error {
                                                    code: "ROUTER_ERROR".to_string(),
                                                    message: format!("In-process router execution failed: {}", e),
                                                    request_id: Some(request_id),
                                                }).await;
                                            }
                                        }
                                    });
                                }

                                // Ping
                                WsClientCommand::Ping => {
                                    let _ = tx_recv.send(WsServerMessage::Pong).await;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx_recv.send(WsServerMessage::Error {
                                code: "DECODE_ERROR".to_string(),
                                message: e,
                                request_id: None,
                            }).await;
                        }
                    }
                }
            }
        }
    });

    // Ожидание завершения любой из задач
    tokio::select! {
        _ = &mut send_task => {},
        _ = &mut recv_task => {},
        _ = &mut bus_task => {},
        _ = &mut heartbeat_task => {},
    }

    // ГАРАНТИРОВАННЫЙ СБРОС И ОТМЕНА ФОНОВЫХ ЗАДАЧ (Фикс утечки тасок)
    send_task.abort();
    recv_task.abort();
    bus_task.abort();
    heartbeat_task.abort();

    trace!("WebSocket client disconnected from /ws/events (sub_id: {})", sub_id);
}
