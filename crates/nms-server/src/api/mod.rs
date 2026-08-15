//! # Модуль API эндпоинтов ядра

pub mod auth;
pub mod events;
pub mod modules;
pub mod system;
pub mod users;

use crate::state::AppState;
use axum::Router;

/// Создать объединенный маршрутизатор подсистем REST API v1 (`/auth`, `/users`, `/modules`, `/system`, `/events`)
pub fn create_api_v1_router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/users", users::router())
        .nest("/modules", modules::router())
        .nest("/system", system::router())
        .nest("/events", events::router())
}
