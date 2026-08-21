//! # Маршрутизация REST API v1 (`/api/v1`)
//!
//! Модуль объединяет все ветки REST API платформы NMSNext-Gen:
//! - [`auth`]: Аутентификация по логину/паролю и проверка текущего пользователя (`/api/v1/auth`).
//! - [`users`]: Управление пользователями, ролями и учетными записями (`/api/v1/users`).
//! - [`modules`]: Управление жизненным циклом и настройками плагинов (`/api/v1/modules`).
//! - [`system`]: Системная информация, аудит, провайдеры и скачивание логов (`/api/v1/system`).
//! - [`events`]: Запрос сохраненных событий из журнала SQLite (`/api/v1/events`).
//! - [`settings`]: Настройки платформы, политики безопасности, права и предпочтения (`/api/v1/settings`).

pub mod auth;
pub mod events;
pub mod modules;
pub mod settings;
pub mod system;
pub mod users;

use crate::state::AppState;
use axum::Router;

/// Создать объединенный маршрутизатор подсистем REST API v1 (`/auth`, `/users`, `/modules`, `/system`, `/events`, `/settings`)
///
/// Все маршруты привязаны к разделяемому состоянию [`AppState`].
pub fn create_api_v1_router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/users", users::router())
        .nest("/modules", modules::router())
        .nest("/system", system::router())
        .nest("/events", events::router())
        .nest("/settings", settings::router())
}
