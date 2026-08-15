//! # Микроядро платформы NMS (nms-core)
//!
//! `nms-core` содержит ключевые сервисы и компоненты серверной платформы мониторинга:
//!
//! - [`auth`]: Аутентификация JWT (HMAC-SHA256), безопасное хэширование паролей Argon2id и RBAC авторизация.
//! - [`bus`]: Гибридная шина событий (in-memory broadcast для live-подписчиков и персистентный журнал SQLite WAL).
//! - [`db`]: Подсистема SQLite с архитектурой Single-Writer / Multi-Reader и встроенным изолированным [`kv::KvStore`](db::kv::KvStore).
//! - [`plugins`]: Менеджер и Zero-Unpack загрузчик плагинов WebAssembly/ZIP с верификацией цифровых подписей Ed25519.
//! - [`services`]: Системные сервисы (журнал аудита действий [`AuditService`](services::AuditService), алерты и вебхуки [`NotifyService`](services::NotifyService), структурированное логирование [`LoggerService`](services::LoggerService)).
//! - [`users`]: Управление учетными записями, ролями и инициализацией системы ([`UserService`](users::UserService)).

#![warn(rustdoc::broken_intra_doc_links)]

pub mod auth;
pub mod bus;
pub mod db;
pub mod plugins;
pub mod services;
pub mod users;

