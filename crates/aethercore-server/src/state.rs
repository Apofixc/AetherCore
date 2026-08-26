//! # Разделяемое состояние веб-сервера (AppState)
//!
//! Инкапсулирует все ключевые подсистемы ядра платформы для безопасного совместного
//! использования между HTTP-обработчиками Axum и WebSocket-шлюзом.

use crate::middleware::HasJwtManager;
use aethercore_common::config::AppConfig;
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{
    AuditService, BackupService, LoggerService, NotifyService, SchedulerService, SessionService,
};
use aethercore_core::users::UserService;
use std::sync::Arc;
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
    /// Сервис управления глобальными сессиями операторов ([`SessionService`])
    pub session_service: SessionService,
    /// Сервис персистентного журнала аудита ([`AuditService`])
    pub audit_service: AuditService,
    /// Сервис системного логирования ([`LoggerService`])
    pub logger_service: LoggerService,
    /// Сервис рассылки уведомлений и алертов ([`NotifyService`])
    pub notify_service: NotifyService,
    /// Менеджер Wasm-плагинов ([`PluginManager`])
    pub plugin_manager: PluginManager,
    /// Центральный сервис планировщика задач ([`SchedulerService`])
    pub scheduler_service: Arc<SchedulerService>,
    /// Сервис создания и восстановления резервных копий SQLite ([`BackupService`])
    pub backup_service: BackupService,
    /// Реестр активных WebSocket-подключений шлюза
    pub ws_registry: crate::ws::registry::WsConnectionRegistry,
    /// Точный момент времени запуска ядра для расчета `uptime`
    pub start_time: Instant,
}

impl HasJwtManager for AppState {
    fn jwt_manager(&self) -> &JwtManager {
        &self.jwt_manager
    }

    fn db(&self) -> Option<&aethercore_core::db::Db> {
        Some(&self.db)
    }

    fn session_service(&self) -> Option<&SessionService> {
        Some(&self.session_service)
    }
}

