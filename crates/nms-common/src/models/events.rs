//! # Модели событий шины сообщений и журнала событий

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Тип события в гибридной шине
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Эфемеренное высокочастотное событие телеметрии (Live broadcast)
    #[default]
    Telemetry,
    /// Гарантированное персистентное системное событие (запись в SQLite WAL)
    Reliable,
}

/// Сообщение события шины
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventMessage {
    /// Уникальный ID сообщения
    pub id: Uuid,
    /// Топик события (например, "users.created", "system.started", "ping-collector.metrics")
    pub topic: String,
    /// Тип доставки
    pub event_type: EventType,
    /// Источник события ("core" или id модуля)
    pub source: String,
    /// Полезная нагрузка события в формате JSON Value
    pub payload: serde_json::Value,
    /// Опциональный бинарный payload (для high-frequency телеметрии / msgpack)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_payload: Option<Vec<u8>>,
    /// Время создания события
    pub timestamp: DateTime<Utc>,
}

impl EventMessage {
    /// Создать новое событие телеметрии
    pub fn telemetry(topic: impl Into<String>, source: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            topic: topic.into(),
            event_type: EventType::Telemetry,
            source: source.into(),
            payload,
            binary_payload: None,
            timestamp: Utc::now(),
        }
    }

    /// Создать новое гарантированное системное событие
    pub fn reliable(topic: impl Into<String>, source: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            topic: topic.into(),
            event_type: EventType::Reliable,
            source: source.into(),
            payload,
            binary_payload: None,
            timestamp: Utc::now(),
        }
    }
}

/// Персистентная запись в журнале системных событий (SQLite WAL)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReliableEventRecord {
    /// Порядковый автоинкрементный номер события
    pub id: i64,
    /// Уникальный UUID события
    pub event_uuid: Uuid,
    /// Топик
    pub topic: String,
    /// Источник
    pub source: String,
    /// JSON строка полезной нагрузки
    pub payload_json: String,
    /// Время фиксации события
    pub created_at: DateTime<Utc>,
}
