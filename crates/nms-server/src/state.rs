//! # Глобальное состояние веб-сервера (AppState)

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
#[derive(Clone)]
pub struct AppState {
    /// Конфигурация платформы
    pub config: AppConfig,
    /// Пул базы данных SQLite
    pub db: Db,
    /// Гибридная шина событий
    pub bus: EventBus,
    /// Менеджер JWT токенов аутентификации
    pub jwt_manager: JwtManager,
    /// Сервис управления пользователями и RBAC
    pub user_service: UserService,
    /// Сервис журнала аудита
    pub audit_service: AuditService,
    /// Сервис системного логирования
    pub logger_service: LoggerService,
    /// Сервис рассылки уведомлений и алертов
    pub notify_service: NotifyService,
    /// Менеджер Wasm-плагинов
    pub plugin_manager: PluginManager,
    /// Момент запуска сервера для расчета uptime
    pub start_time: Instant,
}

impl HasJwtManager for AppState {
    fn jwt_manager(&self) -> &JwtManager {
        &self.jwt_manager
    }
}
