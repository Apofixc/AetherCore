//! # Разделяемое состояние веб-сервера (AppState)
//!
//! Инкапсулирует все ключевые подсистемы ядра платформы для безопасного совместного
//! использования между HTTP-обработчиками Axum и WebSocket-шлюзом.

use crate::middleware::HasJwtManager;
use nms_common::config::AppConfig;
use nms_core::auth::JwtManager;
use nms_core::bus::EventBus;
use nms_core::db::Db;
use nms_core::plugins::PluginManager;
use nms_core::services::{AuditService, LoggerService, NotifyService};
use nms_core::users::UserService;
use std::time::Instant;

/// Общее разделяемое состояние HTTP-сервера и обработчиков Axum
///
/// Клонируется для каждого входящего HTTP запроса (содержит атомарные Arc-указатели внутри сервисов).
#[derive(Clone)]
pub struct AppState {
    /// Полная конфигурация платформы ([`AppConfig`])
    pub config: AppConfig,
    /// Пул соединений с базой данных SQLite WAL ([`Db`])
    pub db: Db,
    /// Гибридная шина событий реального времени ([`EventBus`])
    pub bus: EventBus,
    /// Менеджер JWT токенов аутентификации ([`JwtManager`])
    pub jwt_manager: JwtManager,
    /// Сервис управления пользователями и RBAC ([`UserService`])
    pub user_service: UserService,
    /// Сервис персистентного журнала аудита ([`AuditService`])
    pub audit_service: AuditService,
    /// Сервис системного логирования ([`LoggerService`])
    pub logger_service: LoggerService,
    /// Сервис рассылки уведомлений и алертов ([`NotifyService`])
    pub notify_service: NotifyService,
    /// Менеджер Wasm-плагинов ([`PluginManager`])
    pub plugin_manager: PluginManager,
    /// Точный момент времени запуска ядра для расчета `uptime`
    pub start_time: Instant,
}

impl HasJwtManager for AppState {
    fn jwt_manager(&self) -> &JwtManager {
        &self.jwt_manager
    }

    fn db(&self) -> Option<&nms_core::db::Db> {
        Some(&self.db)
    }
}
