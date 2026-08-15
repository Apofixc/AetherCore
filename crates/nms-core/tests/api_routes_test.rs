use nms_core::log_providers::SharedLogStreamManager;
use nms_core::server::{create_router, AppState, ConnectionManager};
use nms_core::{
    AppConfig, EventBus, I18nEngine, LocalFileLogProvider, LogProviderRegistry, RateLimiter,
};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn test_create_router_with_all_api_routes() {
    let event_bus = EventBus::new(1024);
    let log_registry = LogProviderRegistry::new();
    let default_provider = Arc::new(LocalFileLogProvider::new(
        "test.log",
        "test.log",
        PathBuf::from("./test.log"),
    ));
    log_registry.register(default_provider).await;

    let state = Arc::new(AppState {
        config: AppConfig::default(),
        event_bus,
        log_registry,
        i18n: I18nEngine::new(),
        connection_manager: ConnectionManager::new(),
        db_pool: None,
        rate_limiter: RateLimiter::new(),
        notification_engine: None,
        log_stream_manager: SharedLogStreamManager::new(),
    });

    let _app = create_router(state);
    // Роутер Axum со всеми REST API эндпоинтами (auth, users, notifications, system, events) успешно создается и монтируется
}
