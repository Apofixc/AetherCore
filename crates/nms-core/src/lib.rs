// Основная библиотека ядра NMS приложения (nms-core)
// Модуль выносит все системные сервисы: конфигурацию, базу данных, аудит, уведомления, мониторинг системных метрик, плагины, асинхронную шину, логирование, аутентификацию, планировщик и веб-сервер

pub mod api;
pub mod audit;
pub mod auth;
pub mod bus;
pub mod config;
pub mod crypto;
pub mod db;
pub mod events;
pub mod exceptions;
pub mod i18n;
pub mod locales;
pub mod log_providers;
pub mod logger;
pub mod notify;
pub mod plugin;
pub mod rate_limiter;
pub mod scheduler;
pub mod server;

pub use api::events::*;
pub use audit::{log_audit_event, rotate_audit_logs, AuditLogEntry};
pub use auth::{
    clear_permissions_cache, consume_ws_ticket, create_access_token, create_refresh_token,
    create_ws_ticket, decode_access_token, decode_refresh_token, decode_token, generate_qr_svg,
    generate_totp_secret, get_allowed_cors_origins, get_current_user, get_current_user_optional,
    get_totp_code, get_totp_uri, has_module_permission, has_permission, has_role_permission,
    hash_password, is_ip_whitelisted, is_origin_allowed, is_session_revoked,
    require_module_permission, require_permission, user_has_permission, verify_password,
    verify_totp_code, Claims, CurrentUser, WsTicketManager, TOKEN_TTL_SECONDS,
};
pub use bus::{
    _inspect_subscriber_params, event_bus, match_topic, EventBus, EventBusStats, EventCallback,
    Subscriber, SystemEvent, EVENT_BUS,
};
pub use config::{get_or_create_secret_key, AppConfig};
pub use crypto::{decrypt_secret, encrypt_secret, mask_secret};
pub use db::{
    get_missed_events_from_db, get_system_setting, init_db, init_db_pool, record_event_in_db,
    set_system_setting, EventJournalQueue,
};
pub use events::*;
pub use exceptions::{ErrorDetail, ErrorResponse, NmsError};
pub use i18n::{get_lang, I18nEngine};
pub use log_providers::{
    clean_ansi, clean_ansi_codes, load_remote_sources_from_db, matches_log_level,
    DownloadLogResult, LocalFileLogProvider, LogDataResult, LogProvider, LogProviderInfo,
    LogProviderRegistry, RemoteHTTPLogProvider, SharedLogStreamManager,
};
pub use logger::{setup_logging, stop_logging, LoggingGuard};
pub use notify::{
    get_notification_categories, get_notification_modules, is_quiet_hours, notify,
    NotificationEngine, NotificationFilter, NotificationListResult, NotificationMessage,
    NotificationModuleInfo, NotificationSeverity, NotifyParams, SetPreferencesInput,
    UserNotificationPreferences,
};
pub use plugin::{core_version, ModuleLoadStatus, ModuleRecord, PluginEngine};
pub use rate_limiter::RateLimiter;
pub use scheduler::{get_next_cron_time, AsyncScheduler, JobInfo, ScheduledJob, SchedulerManager};
pub use server::start_server;
