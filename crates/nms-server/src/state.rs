//! # Глобальное состояние веб-сервера (AppState)

use crate::middleware::HasJwtManager;
use nms_common::config::AppConfig;
use nms_core::auth::JwtManager;
use nms_core::bus::EventBus;
use nms_core::db::Db;
use nms_core::plugins::PluginManager;
use nms_core::services::{AuditService, NotifyService};
use nms_core::users::UserService;
use std::time::Instant;

/// Общее разделяемое состояние приложения Axum
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: Db,
    pub bus: EventBus,
    pub jwt_manager: JwtManager,
    pub user_service: UserService,
    pub audit_service: AuditService,
    pub notify_service: NotifyService,
    pub plugin_manager: PluginManager,
    pub start_time: Instant,
}

impl HasJwtManager for AppState {
    fn jwt_manager(&self) -> &JwtManager {
        &self.jwt_manager
    }
}
