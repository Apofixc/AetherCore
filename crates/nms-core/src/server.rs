use crate::api::{auth, events, modules, notifications, system, users};
use crate::bus::{match_topic, EventBus, SystemEvent};
use crate::config::AppConfig;
use crate::db::get_missed_events_from_db;
use crate::log_providers::{
    LocalFileLogProvider, LogProviderInfo, LogProviderRegistry, SharedLogStreamManager,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::header,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};

pub const MAX_CONNECTIONS_PER_USER: usize = 10;

/// Менеджер отслеживания активных WebSocket подключений пользователей
#[derive(Clone, Default)]
pub struct ConnectionManager {
    user_connections: Arc<RwLock<HashMap<String, usize>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_connection(&self, user_id: &str) -> bool {
        let mut guard = match self.user_connections.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        let count = guard.entry(user_id.to_string()).or_insert(0);
        if *count >= MAX_CONNECTIONS_PER_USER {
            return false;
        }
        *count += 1;
        true
    }

    pub fn remove_connection(&self, user_id: &str) {
        let mut guard = match self.user_connections.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            guard.entry(user_id.to_string())
        {
            if *entry.get() <= 1 {
                entry.remove();
            } else {
                *entry.get_mut() -= 1;
            }
        }
    }
}

/// Контекст общего состояния сервера NMS
pub struct AppState {
    pub config: AppConfig,
    pub event_bus: EventBus,
    pub log_registry: LogProviderRegistry,
    pub log_stream_manager: SharedLogStreamManager,
    pub i18n: crate::i18n::I18nEngine,
    pub connection_manager: ConnectionManager,
    pub db_pool: Option<sqlx::SqlitePool>,
    pub rate_limiter: crate::rate_limiter::RateLimiter,
    pub notification_engine: Option<crate::notify::NotificationEngine>,
}

/// Создание роутера Axum с маршрутами REST и WebSocket
pub fn create_router(state: Arc<AppState>) -> Router {
    let origins: Vec<axum::http::HeaderValue> = state
        .config
        .cors_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    let cors = if state.config.cors_origins.contains(&"*".to_string()) || origins.is_empty() {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    Router::new()
        .route("/api/v1/health", get(system::get_system_health_handler))
        // Auth & User Sessions API
        .route("/api/v1/auth/login", post(auth::login_handler))
        .route("/api/v1/auth/refresh", post(auth::refresh_handler))
        .route("/api/v1/auth/2fa/setup", get(auth::setup_mfa_handler))
        .route("/api/v1/auth/2fa/enable", post(users::enable_mfa_handler))
        .route("/api/v1/auth/2fa/disable", post(users::disable_mfa_handler))
        .route(
            "/api/v1/auth/verify-2fa",
            post(users::verify_mfa_login_handler),
        )
        .route("/api/v1/auth/logout", post(users::logout_handler))
        .route("/api/v1/auth/ws-ticket", get(users::get_ws_ticket_handler))
        // Users & Roles API
        .route(
            "/api/v1/users/me",
            get(users::get_me_handler).put(users::update_own_profile_handler),
        )
        .route(
            "/api/v1/users/me/change-password",
            post(users::change_own_password_handler),
        )
        .route(
            "/api/v1/users",
            get(users::list_users_handler).post(users::create_user_handler),
        )
        .route(
            "/api/v1/users/{id}",
            put(users::update_user_handler).delete(users::delete_user_handler),
        )
        .route(
            "/api/v1/users/{id}/terminate-sessions",
            post(users::terminate_user_sessions_handler),
        )
        .route(
            "/api/v1/users/{id}/sessions",
            get(users::get_user_sessions_handler),
        )
        .route("/api/v1/users/bulk", post(users::bulk_users_action_handler))
        .route(
            "/api/v1/roles",
            get(users::list_roles_handler).post(users::create_role_handler),
        )
        .route(
            "/api/v1/roles/{id}",
            put(users::update_role_handler).delete(users::delete_role_handler),
        )
        .route("/api/v1/permissions", get(users::list_permissions_handler))
        .route("/api/v1/audit", get(users::get_audit_logs_handler))
        .route(
            "/api/v1/audit/export",
            get(users::export_audit_logs_handler),
        )
        .route(
            "/api/v1/audit/rotate",
            post(users::rotate_audit_logs_endpoint_handler),
        )
        .route(
            "/api/v1/security-settings",
            get(users::get_security_settings_endpoint_handler)
                .put(users::update_security_settings_endpoint_handler),
        )
        .route("/api/v1/my-sessions", get(users::get_my_sessions_handler))
        .route(
            "/api/v1/my-sessions/{id}",
            delete(users::revoke_my_session_handler),
        )
        .route(
            "/api/v1/sessions/{id}",
            delete(users::revoke_session_handler),
        )
        // Modules API
        .route("/api/v1/modules", get(modules::list_modules_handler))
        .route(
            "/api/v1/modules/loaded",
            get(modules::loaded_modules_handler),
        )
        .route(
            "/api/v1/modules/widgets",
            get(modules::list_module_widgets_handler),
        )
        .route(
            "/api/v1/modules/summary_widget",
            get(modules::get_system_modules_widget_handler),
        )
        .route(
            "/api/v1/modules/scan",
            post(modules::scan_modules_endpoint_handler),
        )
        .route(
            "/api/v1/modules/install",
            post(modules::install_module_endpoint_handler),
        )
        .route(
            "/api/v1/modules/config-schema",
            get(modules::module_config_schema_handler),
        )
        .route(
            "/api/v1/modules/{id}/export",
            get(modules::export_module_endpoint_handler),
        )
        .route(
            "/api/v1/modules/{id}",
            delete(modules::delete_module_endpoint_handler),
        )
        .route(
            "/api/v1/modules/{id}/enabled",
            put(modules::toggle_module_handler),
        )
        .route(
            "/api/v1/modules/{id}/views",
            get(modules::module_views_handler),
        )
        .route(
            "/api/v1/modules/{id}/settings-definition",
            get(modules::module_settings_definition_handler),
        )
        .route(
            "/api/v1/modules/{id}/settings",
            get(modules::module_settings_get_handler).put(modules::module_settings_put_handler),
        )
        .route(
            "/api/v1/modules/{id}/status",
            get(modules::module_status_handler),
        )
        .route(
            "/api/v1/modules/{id}/locales/{lang}",
            get(modules::module_locales_handler),
        )
        .route(
            "/api/v1/modules/{id}/files/{*file_path}",
            get(modules::serve_module_file_handler),
        )
        // Notifications API
        .route(
            "/api/v1/notifications/categories",
            get(notifications::list_categories_handler),
        )
        .route(
            "/api/v1/notifications/modules",
            get(notifications::list_modules_handler),
        )
        .route(
            "/api/v1/notifications",
            get(notifications::list_notifications_handler),
        )
        .route(
            "/api/v1/notifications/{id}/read",
            post(notifications::mark_read_handler),
        )
        .route(
            "/api/v1/notifications/{id}/unread",
            post(notifications::unread_notification_handler),
        )
        .route(
            "/api/v1/notifications/{id}/acknowledge",
            post(notifications::acknowledge_handler),
        )
        .route(
            "/api/v1/notifications/acknowledge-all",
            post(notifications::acknowledge_all_user_notifications_handler),
        )
        .route(
            "/api/v1/notifications/read-all",
            post(notifications::mark_all_read_handler),
        )
        .route(
            "/api/v1/notifications/clear-read",
            delete(notifications::delete_all_read_handler),
        )
        .route(
            "/api/v1/notifications/prune",
            post(notifications::prune_stale_notifications_handler),
        )
        .route(
            "/api/v1/notifications/process-escalations",
            post(notifications::trigger_alert_escalations_handler),
        )
        .route(
            "/api/v1/notifications/{id}",
            delete(notifications::remove_notification_handler),
        )
        .route(
            "/api/v1/notifications/preferences",
            get(notifications::get_preferences_handler).put(notifications::set_preferences_handler),
        )
        .route(
            "/api/v1/notifications/export",
            get(notifications::export_notifications_handler),
        )
        // System & Wiki API
        .route(
            "/api/v1/system/health",
            get(system::get_system_health_handler),
        )
        .route(
            "/api/v1/system/backup",
            get(system::download_backup_handler),
        )
        .route(
            "/api/v1/system/restore",
            post(system::restore_backup_handler),
        )
        .route("/api/v1/system/logs", get(list_logs_handler))
        .route(
            "/api/v1/system/logs/{provider_id}",
            get(get_log_content_handler),
        )
        .route(
            "/api/v1/system/logs/{provider_id}/download",
            get(download_log_handler),
        )
        .route(
            "/api/v1/system/logs/remote-sources/list",
            get(system::list_remote_log_sources_handler),
        )
        .route(
            "/api/v1/system/logs/remote-sources",
            post(system::add_remote_log_source_handler),
        )
        .route(
            "/api/v1/system/logs/remote-sources/{id}",
            delete(system::delete_remote_log_source_handler),
        )
        .route(
            "/api/v1/system/ws-metrics",
            get(system::get_websocket_metrics_handler),
        )
        .route(
            "/api/v1/system/sessions",
            get(system::list_sessions_handler),
        )
        .route(
            "/api/v1/system/sessions/terminate-all",
            post(system::terminate_all_sessions_handler),
        )
        .route(
            "/api/v1/system/docs/module-guide",
            get(system::get_module_guide_doc_handler),
        )
        .route(
            "/api/v1/system/docs/wiki/tree",
            get(system::get_wiki_tree_handler),
        )
        .route(
            "/api/v1/system/docs/wiki/article",
            get(system::get_wiki_article_handler),
        )
        // Events API
        .route("/api/v1/events", get(events::get_events_info_handler))
        .route("/api/v1/events/info", get(events::get_events_info_handler))
        .route(
            "/api/v1/events/history",
            get(events::get_events_history_handler),
        )
        // Realtime WebSocket
        .route("/api/v1/ws/events", get(ws_events_handler))
        .route(
            "/api/v1/system/logs/stream/{log_name}",
            get(ws_log_stream_handler),
        )
        .layer(cors)
        .with_state(state)
}

/// Параметры запроса чтения файла логов
#[derive(Debug, Deserialize)]
pub struct LogQuery {
    #[serde(default = "default_lines")]
    pub lines: usize,
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default)]
    pub search: String,
}

fn default_lines() -> usize {
    200
}
fn default_level() -> String {
    "ALL".to_string()
}

/// Эндпоинт получения списка всех доступных провайдеров логов
async fn list_logs_handler(State(state): State<Arc<AppState>>) -> Json<Vec<LogProviderInfo>> {
    let providers = state.log_registry.list_all().await;
    Json(providers)
}

/// Эндпоинт чтения содержимого лог-источника с фильтрацией
async fn get_log_content_handler(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
    Query(query): Query<LogQuery>,
) -> Result<impl IntoResponse, crate::exceptions::NmsError> {
    let provider = state.log_registry.get(&provider_id).await.ok_or_else(|| {
        crate::exceptions::NmsError::NotFound {
            message: "Log provider not found".to_string(),
        }
    })?;

    let data = provider
        .get_logs(query.lines, &query.level, &query.search)
        .await
        .map_err(|err| crate::exceptions::NmsError::Internal {
            message: err.to_string(),
            details: json!({}),
        })?;

    Ok(Json(serde_json::to_value(data).unwrap()))
}

/// Эндпоинт скачивания лог-файла целиком
async fn download_log_handler(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
) -> Result<impl IntoResponse, crate::exceptions::NmsError> {
    let provider = state.log_registry.get(&provider_id).await.ok_or_else(|| {
        crate::exceptions::NmsError::NotFound {
            message: "Log provider not found".to_string(),
        }
    })?;

    let dl =
        provider
            .download_log()
            .await
            .map_err(|err| crate::exceptions::NmsError::Internal {
                message: err.to_string(),
                details: json!({}),
            })?;

    Ok((
        [
            (header::CONTENT_TYPE, dl.media_type),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", dl.filename),
            ),
        ],
        dl.content,
    ))
}

/// Обработчик проверки здоровья сервера (Health check)
async fn health_check_handler() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "nms-core",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Параметры WebSocket запроса
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
    pub user_id: Option<String>,
}

/// Параметры WebSocket запроса стриминга логов (1-в-1 с Python stream_log_websocket)
#[derive(Debug, Deserialize)]
pub struct WsLogQuery {
    pub token: Option<String>,
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default)]
    pub search: String,
}

/// Входящие команды клиента по WebSocket
#[derive(Debug, Deserialize)]
pub struct WsClientCommand {
    pub action: String,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub from_seq_id: Option<i64>,
}

/// WebSocket обработчик реального времени для трансляции системных событий
async fn ws_events_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<axum::response::Response, crate::exceptions::NmsError> {
    let user_id = if let Some(token) = &query.token {
        let secret = crate::config::get_or_create_secret_key();
        match crate::auth::decode_token(token, &secret) {
            Some(claims) => claims.sub,
            None => query
                .user_id
                .clone()
                .unwrap_or_else(|| "anonymous".to_string()),
        }
    } else {
        query
            .user_id
            .clone()
            .unwrap_or_else(|| "anonymous".to_string())
    };

    if !state.connection_manager.add_connection(&user_id) {
        return Err(crate::exceptions::NmsError::PermissionDenied {
            message: format!(
                "Max WebSocket connections ({}) reached for user",
                MAX_CONNECTIONS_PER_USER
            ),
        });
    }

    Ok(ws.on_upgrade(move |socket| handle_ws_socket(socket, state, user_id)))
}

/// Фильтрация событий для доставки конкретному WebSocket клиенту
fn should_deliver_event(
    event: &SystemEvent,
    user_id: &str,
    subscriptions: &HashSet<String>,
) -> bool {
    // 1. Проверка адресации по target_user_id
    if let Some(target_uid) = &event.target_user_id {
        if target_uid != user_id {
            return false;
        }
    }

    // 2. Проверка топиков подписки (если не пусто)
    if subscriptions.is_empty() {
        return true;
    }

    subscriptions
        .iter()
        .any(|pattern| match_topic(pattern, &event.topic))
}

/// Обслуживание активного WebSocket подключения клиента
async fn handle_ws_socket(mut socket: WebSocket, state: Arc<AppState>, user_id: String) {
    let mut rx = state.event_bus.subscribe_receiver();
    let mut subscriptions = HashSet::<String>::new();
    info!("New WebSocket client connected for user '{}'", user_id);

    loop {
        tokio::select! {
            // Чтение сообщений от клиента (команды подписки, replay, ping)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(txt))) => {
                        if let Ok(cmd) = serde_json::from_str::<WsClientCommand>(&txt) {
                            match cmd.action.as_str() {
                                "subscribe" => {
                                    for t in cmd.topics {
                                        subscriptions.insert(t);
                                    }
                                    let _ = socket.send(Message::Text(json!({
                                        "status": "ok",
                                        "action": "subscribe",
                                        "subscriptions": subscriptions.iter().collect::<Vec<_>>()
                                    }).to_string().into())).await;
                                }
                                "unsubscribe" => {
                                    for t in cmd.topics {
                                        subscriptions.remove(&t);
                                    }
                                    let _ = socket.send(Message::Text(json!({
                                        "status": "ok",
                                        "action": "unsubscribe",
                                        "subscriptions": subscriptions.iter().collect::<Vec<_>>()
                                    }).to_string().into())).await;
                                }
                                "replay" => {
                                    if let Some(from_seq_id) = cmd.from_seq_id {
                                        if let Some(pool) = &state.db_pool {
                                            match get_missed_events_from_db(pool, from_seq_id, Some(&user_id), None, 100).await {
                                                Ok(missed_events) => {
                                                    for ev in missed_events {
                                                        if should_deliver_event(&ev, &user_id, &subscriptions) {
                                                            if let Ok(ev_json) = serde_json::to_string(&ev) {
                                                                let _ = socket.send(Message::Text(ev_json.into())).await;
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(err) => {
                                                    error!("Failed to fetch replay events: {}", err);
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Ping(b))) => {
                        let _ = socket.send(Message::Pong(b)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }

            // Получение системных событий из шины и отправка клиенту в JSON
            event_res = rx.recv() => {
                match event_res {
                    Ok(event) => {
                        if should_deliver_event(&event, &user_id, &subscriptions) {
                            if let Ok(json_str) = serde_json::to_string(&event) {
                                if socket.send(Message::Text(json_str.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        info!("WebSocket client lagged behind, skipped {} events", skipped);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    state.connection_manager.remove_connection(&user_id);
    info!("WebSocket client disconnected for user '{}'", user_id);
}

/// WebSocket обработчик стриминга логов в реальном времени (1-в-1 с Python stream_log_websocket)
async fn ws_log_stream_handler(
    ws: WebSocketUpgrade,
    Path(log_name): Path<String>,
    Query(query): Query<WsLogQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<axum::response::Response, crate::exceptions::NmsError> {
    // Проверка существования провайдера логов
    let _provider = state.log_registry.get(&log_name).await.ok_or_else(|| {
        crate::exceptions::NmsError::NotFound {
            message: "Log provider not found".to_string(),
        }
    })?;

    // Аутентификация через token/ticket (1-в-1 с Python)
    if let Some(token) = &query.token {
        let secret = crate::config::get_or_create_secret_key();
        if token.starts_with("wst_") {
            // Ticket-based auth
            let ticket_user = crate::auth::consume_ws_ticket(token).await;
            if ticket_user.is_none() {
                return Err(crate::exceptions::NmsError::AuthRequired {
                    message: "Unauthorized: Invalid ticket".to_string(),
                });
            }
        } else {
            // JWT token auth
            if crate::auth::decode_token(token, &secret).is_none() {
                return Err(crate::exceptions::NmsError::AuthRequired {
                    message: "Unauthorized: Invalid token".to_string(),
                });
            }
        }
    }

    let log_stream_mgr = state.log_stream_manager.clone();
    let log_registry = state.log_registry.clone();
    let level = query.level;
    let search = query.search;

    Ok(ws.on_upgrade(move |socket| {
        handle_log_ws_socket(
            socket,
            log_name,
            level,
            search,
            log_stream_mgr,
            log_registry,
        )
    }))
}

/// Обслуживание активного WebSocket подключения стриминга логов (1-в-1 с Python stream_log_websocket)
async fn handle_log_ws_socket(
    mut socket: WebSocket,
    log_name: String,
    level: String,
    search: String,
    manager: SharedLogStreamManager,
    registry: LogProviderRegistry,
) {
    let sub_id = uuid::Uuid::new_v4().to_string();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    manager
        .subscribe(
            sub_id.clone(),
            log_name.clone(),
            level.clone(),
            search.clone(),
            registry,
            tx,
        )
        .await;

    info!(
        "Log stream WS connected: sub_id={}, log={}",
        sub_id, log_name
    );

    loop {
        tokio::select! {
            // Получение строк логов из SharedLogStreamManager и отправка клиенту
            line = rx.recv() => {
                match line {
                    Some(payload) => {
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            // Чтение сообщений от клиента (ping/pong)
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(txt))) => {
                        if txt.as_str() == "ping" {
                            let _ = socket.send(Message::Text("{\"type\":\"pong\"}".into())).await;
                        }
                    }
                    Some(Ok(Message::Ping(b))) => {
                        let _ = socket.send(Message::Pong(b)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    manager
        .unsubscribe(&sub_id, &log_name, &level, &search)
        .await;
    info!("Log stream WS disconnected: sub_id={}", sub_id);
}

/// Запуск асинхронного веб-сервера Axum
pub async fn start_server(config: AppConfig) -> anyhow::Result<()> {
    let addr = config.socket_addr()?;
    let event_bus = EventBus::new(2048);

    let log_registry = LogProviderRegistry::new();
    let default_provider = Arc::new(LocalFileLogProvider::new(
        "backend.log",
        "backend.log",
        PathBuf::from("./backend.log"),
    ));
    log_registry.register(default_provider).await;

    let i18n = crate::i18n::I18nEngine::new();
    let connection_manager = ConnectionManager::new();
    let rate_limiter = crate::rate_limiter::RateLimiter::new();

    let log_stream_manager = SharedLogStreamManager::new();

    let state = Arc::new(AppState {
        config,
        event_bus,
        log_registry,
        log_stream_manager,
        i18n,
        connection_manager,
        db_pool: None,
        rate_limiter,
        notification_engine: None,
    });
    let app = create_router(state);

    let listener = TcpListener::bind(addr).await?;
    info!("NMS HTTP/WS server started successfully on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
