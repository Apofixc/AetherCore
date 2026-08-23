//! # Модели настроек платформы (Platform & Maintenance Settings)
//!
//! Модуль содержит общие структуры параметров обслуживания и конфигурации ядра.

use serde::{Deserialize, Serialize};

/// Настройки регламентного обслуживания и политик хранения данных
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceSettings {
    /// Флаг автобэкапа базы данных
    #[serde(default = "default_true")]
    pub auto_backup: bool,
    /// Интервал автобэкапа в часах
    #[serde(default = "default_backup_interval")]
    pub backup_interval_hours: u32,
    /// Срок хранения бэкапов в днях
    #[serde(default = "default_backup_retention")]
    pub backup_retention_days: u32,
    /// Срок хранения журнала аудита в днях
    #[serde(default = "default_audit_retention")]
    pub audit_retention_days: u32,
    /// Срок хранения записей истории планировщика в днях
    #[serde(default = "default_history_retention")]
    pub history_retention_days: u32,
    /// Уровень системного логирования по умолчанию
    #[serde(default = "default_log_level")]
    pub default_log_level: String,
}

fn default_true() -> bool {
    true
}

fn default_backup_interval() -> u32 {
    24
}

fn default_backup_retention() -> u32 {
    30
}

fn default_audit_retention() -> u32 {
    90
}

fn default_history_retention() -> u32 {
    30
}

fn default_log_level() -> String {
    "INFO".to_string()
}

impl Default for MaintenanceSettings {
    fn default() -> Self {
        Self {
            auto_backup: true,
            backup_interval_hours: default_backup_interval(),
            backup_retention_days: default_backup_retention(),
            audit_retention_days: default_audit_retention(),
            history_retention_days: default_history_retention(),
            default_log_level: default_log_level(),
        }
    }
}
