//! # Системные сервисы ядра

pub mod audit;
pub mod notify;

pub use audit::{AuditLogRecord, AuditService};
pub use notify::{AlertMessage, AlertSeverity, NotifyService};
