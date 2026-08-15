//! # Системные сервисы ядра (System Services)
//!
//! Модуль объединяет ключевые вспомогательные сервисы платформы:
//! - [`audit`]: Журнал аудита действий пользователей и подсистем ([`AuditService`]).
//! - [`logger`]: In-memory кольцевой буфер и файловый логгер с ротацией и поиском ([`LoggerService`]).
//! - [`notify`]: Сервис рассылки аварийных уведомлений и вебхуков ([`NotifyService`]).

pub mod audit;
pub mod logger;
pub mod notify;

pub use audit::{AuditLogRecord, AuditService};
pub use logger::{LogEntry, LogLevel, LogProvider, LogQueryResult, LoggerConfig, LoggerService};
pub use notify::{AlertMessage, AlertSeverity, NotifyService};

