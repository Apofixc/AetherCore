//! # Модели данных планировщика задач (Task Scheduler / Cron Engine)
//!
//! Модуль содержит декларативные модели задач, типов расписаний (Cron, Interval, One-off),
//! целевых действий ядра и плагинов, политик конкурентности, а также записей истории выполнения.

use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Политика конкурентности при наложении запусков задачи (Task Overlap)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyPolicy {
    /// Пропустить новый тик, если предыдущий экземпляр еще выполняется (рекомендуется по умолчанию)
    #[default]
    Skip,
    /// Разрешить параллельный запуск нового экземпляра
    Allow,
    /// Поставить тик в очередь ожидания завершения предыдущего
    Queue,
}

impl FromStr for ConcurrencyPolicy {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "skip" => Ok(Self::Skip),
            "allow" | "parallel" => Ok(Self::Allow),
            "queue" => Ok(Self::Queue),
            _ => Err(AppError::validation("concurrency_policy", format!("Invalid policy: '{}'", s))),
        }
    }
}

impl std::fmt::Display for ConcurrencyPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Skip => write!(f, "skip"),
            Self::Allow => write!(f, "allow"),
            Self::Queue => write!(f, "queue"),
        }
    }
}

/// Политика обработки пропущенных запусков при простое системы (Misfire Policy)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MisfirePolicy {
    /// Пропустить пропущенные запуски и ждать следующего планового времени
    #[default]
    SkipToNext,
    /// Запустить ровно один раз немедленно при старте системы
    FireOnceImmediately,
}

impl FromStr for MisfirePolicy {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "skip_to_next" | "skip" => Ok(Self::SkipToNext),
            "fire_once_immediately" | "fire_once" => Ok(Self::FireOnceImmediately),
            _ => Err(AppError::validation("misfire_policy", format!("Invalid policy: '{}'", s))),
        }
    }
}

impl std::fmt::Display for MisfirePolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkipToNext => write!(f, "skip_to_next"),
            Self::FireOnceImmediately => write!(f, "fire_once_immediately"),
        }
    }
}

/// Тип расписания выполнения задачи
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum TaskSchedule {
    /// Стандартное Cron выражение (поддерживает 5 и 6-позиционный синтаксис)
    Cron(String),
    /// Фиксированный периодический интервал в секундах
    IntervalSec(u64),
    /// Однократный запуск в заданный момент времени
    OneOff(DateTime<Utc>),
}

impl TaskSchedule {
    /// Проверить корректность параметров расписания
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Validation`](crate::error::AppError), если выражение cron невалидно или интервал равен нулю.
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Cron(expr) => {
                Self::parse_cron(expr).map(|_| ())
            }
            Self::IntervalSec(secs) => {
                if *secs == 0 {
                    Err(AppError::validation("interval", "Interval must be greater than 0 seconds"))
                } else {
                    Ok(())
                }
            }
            Self::OneOff(_) => Ok(()),
        }
    }

    /// Распарсить cron строку, поддерживая как 5-позиционный (min hour dom month dow),
    /// так и 6-7 позиционный синтаксис (sec min hour dom month dow \[year\]).
    ///
    /// # Аргументы
    /// * `expr` — Строка cron-выражения (например, `"*/5 * * * *"`).
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Validation`](crate::error::AppError), если синтаксис некорректен.
    pub fn parse_cron(expr: &str) -> Result<Schedule> {
        let trimmed = expr.trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();

        let normalized = if parts.len() == 5 {
            // Классический 5-позиционный cron -> добавляем 0 секунд в начало
            format!("0 {}", trimmed)
        } else {
            trimmed.to_string()
        };

        Schedule::from_str(&normalized)
            .map_err(|e| AppError::validation("cron", format!("Invalid cron expression '{}': {}", expr, e)))
    }

    /// Вычислить время следующего запуска задачи относительно точки отсчета
    ///
    /// # Аргументы
    /// * `from_time` — Точка отсчета времени в формате UTC.
    ///
    /// # Возвращаемое значение
    /// [`Option<DateTime<Utc>>`] с вычисленным временем следующего срабатывания.
    pub fn calculate_next_run(&self, from_time: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Cron(expr) => {
                let schedule = Self::parse_cron(expr).ok()?;
                schedule.after(&from_time).next()
            }
            Self::IntervalSec(secs) => {
                Some(from_time + chrono::Duration::seconds(*secs as i64))
            }
            Self::OneOff(target_time) => {
                if *target_time > from_time {
                    Some(*target_time)
                } else {
                    None
                }
            }
        }
    }
}

/// Целевое системное или прикладное действие задачи
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "params", rename_all = "snake_case")]
pub enum TaskAction {
    /// Системная ротация и архивация журнала аудита ядра
    SystemAuditRotation,
    /// Системная очистка устаревшей истории самого планировщика
    SystemHistoryCleanup,
    /// Системное резервное копирование базы данных SQLite
    SystemDbBackup,
    /// Периодический вызов функции гостевого WASM-модуля по контракту timer-consumer::on-timer
    PluginTimer {
        /// Идентификатор модуля
        module_id: String,
        /// Идентификатор таймера внутри модуля
        timer_id: String,
    },
    /// Публикация запланированного события в системную шину EventBus
    EventBusPublish {
        /// Топик события
        topic: String,
        /// JSON-полезная нагрузка события
        payload: serde_json::Value,
    },
}

/// Текущий оперативный статус задачи
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Ожидает наступления времени следующего запуска
    #[default]
    Idle,
    /// В данный момент выполняется воркером
    Running,
    /// Задача временно отключена администратором
    Disabled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Running => write!(f, "running"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

/// Статус завершения единичного запуска задачи
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Завершено успешно
    Success,
    /// Завершено с ошибкой
    Failed,
    /// Прервано по таймауту
    Timeout,
    /// Пропущено по политике конкурентности (Skip)
    Skipped,
    /// Прервано из-за перезагрузки сервера (Crash recovery)
    Aborted,
}

impl FromStr for ExecutionStatus {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "success" => Ok(Self::Success),
            "failed" => Ok(Self::Failed),
            "timeout" => Ok(Self::Timeout),
            "skipped" => Ok(Self::Skipped),
            "aborted" => Ok(Self::Aborted),
            _ => Err(AppError::validation("status", format!("Invalid execution status: '{}'", s))),
        }
    }
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Failed => write!(f, "failed"),
            Self::Timeout => write!(f, "timeout"),
            Self::Skipped => write!(f, "skipped"),
            Self::Aborted => write!(f, "aborted"),
        }
    }
}

/// Полная сущность запланированной задачи в системе
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduledTask {
    /// Уникальный идентификатор задачи (kebab-case или UUID)
    pub id: String,
    /// Человекочитаемое название
    pub name: String,
    /// Подробное описание назначения
    pub description: Option<String>,
    /// Конфигурация расписания
    pub schedule: TaskSchedule,
    /// Исполняемое действие
    pub action: TaskAction,
    /// Политика конкурентности при наложениях
    pub concurrency_policy: ConcurrencyPolicy,
    /// Политика при пропуске запусков во время простоя
    pub misfire_policy: MisfirePolicy,
    /// Максимальный таймаут исполнения в секундах
    pub timeout_secs: u32,
    /// Флаг активности задачи
    pub is_enabled: bool,
    /// Системная встроенная задача ядра (защищена от случайного удаления)
    pub is_system: bool,
    /// Рассчитанное время следующего планового запуска (UTC)
    pub next_run_at: Option<DateTime<Utc>>,
    /// Время фактического последнего запуска (UTC)
    pub last_run_at: Option<DateTime<Utc>>,
    /// Статус последнего запуска
    pub last_status: Option<ExecutionStatus>,
    /// Сообщение об ошибке/результате последнего запуска
    pub last_error: Option<String>,
    /// Дата создания записи
    pub created_at: DateTime<Utc>,
    /// Дата последнего обновления конфигурации
    pub updated_at: DateTime<Utc>,
}

/// Запись в журнале истории выполнения задач
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskExecutionRecord {
    /// Уникальный ID записи истории
    pub id: i64,
    /// ID связанной задачи
    pub task_id: String,
    /// Название задачи на момент запуска
    pub task_name: String,
    /// Время старта задачи (UTC)
    pub started_at: DateTime<Utc>,
    /// Время завершения задачи (UTC)
    pub finished_at: DateTime<Utc>,
    /// Итоговый статус выполнения
    pub status: ExecutionStatus,
    /// Длительность выполнения в миллисекундах
    pub duration_ms: u64,
    /// Описание ошибки или детали результата
    pub error_message: Option<String>,
    /// Инициатор запуска: `"scheduler"` или `"manual:{username}"`
    pub triggered_by: String,
}

/// DTO создания новой задачи через API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskDto {
    /// Опциональный кастомный ID (если не указан, генерируется uuid)
    pub id: Option<String>,
    /// Название задачи
    pub name: String,
    /// Описание задачи
    pub description: Option<String>,
    /// Расписание
    pub schedule: TaskSchedule,
    /// Целевое действие
    pub action: TaskAction,
    /// Политика конкурентности
    #[serde(default)]
    pub concurrency_policy: ConcurrencyPolicy,
    /// Политика пропусков
    #[serde(default)]
    pub misfire_policy: MisfirePolicy,
    /// Таймаут в секундах (по умолчанию 300)
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u32,
    /// Включена ли задача при создании
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

fn default_timeout_secs() -> u32 {
    300
}

fn default_true() -> bool {
    true
}

/// DTO обновления существующей задачи через API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskDto {
    /// Название задачи
    pub name: Option<String>,
    /// Описание задачи
    pub description: Option<String>,
    /// Расписание
    pub schedule: Option<TaskSchedule>,
    /// Целевое действие
    pub action: Option<TaskAction>,
    /// Политика конкурентности
    pub concurrency_policy: Option<ConcurrencyPolicy>,
    /// Политика пропусков
    pub misfire_policy: Option<MisfirePolicy>,
    /// Таймаут в секундах
    pub timeout_secs: Option<u32>,
    /// Флаг активности
    pub is_enabled: Option<bool>,
}

/// DTO фильтрации истории задач
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoryQueryDto {
    /// ID конкретной задачи (опционально)
    pub task_id: Option<String>,
    /// Лимит строк (по умолчанию 50)
    pub limit: Option<i64>,
    /// Смещение пагинации
    pub offset: Option<i64>,
}
