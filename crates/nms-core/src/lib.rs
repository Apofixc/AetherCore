//! # Микроядро платформы nms-core
//!
//! Предоставляет сервисы базы данных, аутентификации, шины событий
//! и хостинга плагинов в песочнице Wasmtime.

pub mod auth;
pub mod bus;
pub mod db;
pub mod plugins;
pub mod services;
pub mod users;
