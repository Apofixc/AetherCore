//! # Web-сервер и сетевой шлюз платформы (nms-server)
//!
//! Обеспечивает маршрутизацию REST API (`/api/v1`), WebSocket поток системных событий (`/ws/events`),
//! JWT аутентификацию, RBAC авторизацию и потоковую Zero-Unpack раздачу ассетов плагинов (`/modules/{id}/*`).

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

pub mod api;
pub mod middleware;
pub mod state;
pub mod ws;

pub use state::AppState;

use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

/// Создать сконфигурированный Axum роутер с поддержкой CORS, трейсинга и инъекцией состояния
///
/// # Аргументы
/// * `state` — Глобальное состояние приложения [`AppState`].
pub fn create_app_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .nest("/api/v1", api::create_api_v1_router())
        .route("/modules/{id}/{*path}", get(module_assets_handler))
        .route("/ws/events", get(ws::ws_events_handler))
        .route("/health", get(|| async { "OK" }))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Обработчик прямой потоковой отдачи статических ассетов модулей из оперативной памяти
async fn module_assets_handler(
    State(state): State<AppState>,
    Path((id, path)): Path<(String, String)>,
) -> Response {
    match state.plugin_manager.get_frontend_asset(&id, &path) {
        Some(data) => {
            let content_type = mime_guess::from_path(&path)
                .first_raw()
                .unwrap_or("application/octet-stream");

            (
                StatusCode::OK,
                [
                    (CONTENT_TYPE, content_type),
                    (CACHE_CONTROL, "public, max-age=31536000, immutable"),
                ],
                data,
            )
                .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            format!("Asset '{}' for module '{}' not found", path, id),
        )
            .into_response(),
    }
}

/// Запустить асинхронный HTTP/WebSocket веб-сервер Tokio/Axum
///
/// # Аргументы
/// * `state` — Экземпляр состояния [`AppState`].
///
/// # Ошибки
/// Возвращает ошибку, если привязка к TCP-порту завершилась сбоем.
pub async fn run_server(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    let host = &state.config.server.host;
    let port = state.config.server.port;
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

    let app = create_app_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP Server listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
