//! # Системные сервисы ядра (System Services)
//!
//! Модуль объединяет ключевые вспомогательные сервисы платформы:
//! - [`audit`]: Журнал аудита действий пользователей и подсистем ([`AuditService`]).
//! - [`logger`]: In-memory кольцевой буфер и файловый логгер с ротацией и поиском ([`LoggerService`]).
//! - [`notify`]: Сервис рассылки аварийных уведомлений и вебхуков ([`NotifyService`]).
//! - [`scheduler`]: Центральный сервис планировщика задач и фоновых джобов ([`SchedulerService`]).
//! - [`backup`]: Сервис резервного копирования, снимков и восстановления SQLite ([`BackupService`]).

pub mod audit;
pub mod backup;
pub mod logger;
pub mod notify;
pub mod scheduler;

pub use audit::{AuditArchiveInfo, AuditLogRecord, AuditService};
pub use backup::{BackupInfo, BackupService, RestoreResult};
pub use logger::{LogEntry, LogLevel, LogProvider, LogQueryResult, LoggerConfig, LoggerService};
pub use notify::{AlertMessage, AlertSeverity, NotifyService};
pub use scheduler::{handlers, SchedulerService, TaskHandler};
