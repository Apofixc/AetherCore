//! # Web-сервер и сетевой шлюз платформы (aethercore-server)
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

/// Обработчик прямой потоковой отдачи статических веб-ассетов модулей (`/modules/{id}/{*path}`)
///
/// Извлекает скомпилированные frontend-файлы (JS, CSS, HTML, SVG, PNG) напрямую из оперативной памяти
/// без распаковки архива плагина на диск (Zero-Unpack), определяет MIME-тип и выставляет заголовок кэширования `immutable`.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `(id, path)` — Идентификатор модуля и относительный путь к файлу ассета.
///
/// # Возвращаемое значение
/// HTTP-ответ [`Response`] с бинарным телом файла и заголовками `Content-Type` и `Cache-Control`,
/// либо статус `404 Not Found`, если модуль или ассет отсутствует.
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

use aethercore_core::db::kv::KvStore;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RetentionConfig {
    #[serde(default = "default_audit_retention_days")]
    audit_retention_days: u32,
}

fn default_audit_retention_days() -> u32 {
    90
}

fn spawn_audit_retention_worker(state: &AppState) {
    let db = state.db.clone();
    let audit_service = state.audit_service.clone();

    tokio::spawn(async move {
        // Запуск раз в 24 часа для удаления записей аудита старше audit_retention_days
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
        loop {
            interval.tick().await;
            let kv = KvStore::system(db.clone());
            let retention_days = match kv.get::<RetentionConfig>("maintenance_settings").await {
                Ok(Some(cfg)) => cfg.audit_retention_days,
                _ => 90,
            };

            if retention_days > 0 {
                match audit_service.prune_old_logs(retention_days).await {
                    Ok(pruned) if pruned > 0 => {
                        info!(
                            "Audit retention worker: pruned {} log records older than {} days",
                            pruned, retention_days
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("Audit retention worker error: {}", e);
                    }
                }
            }
        }
    });
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

    spawn_audit_retention_worker(&state);

    let app = create_app_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP Server listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
