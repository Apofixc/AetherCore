//! # Модели событий шины сообщений и журнала событий
//!
//! Определяет структуру сообщений событий платформы ([`EventMessage`]),
//! режимы доставки ([`EventType`]) и формат персистентных записей SQLite WAL ([`ReliableEventRecord`]).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Тип и семантика доставки события в гибридной шине событий
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Эфемеренное высокочастотное событие телеметрии (Live broadcast через Tokio канал, не пишется в БД)
    #[default]
    Telemetry,
    /// Гарантированное персистентное системное событие (запись в журнал SQLite WAL + Live broadcast)
    Reliable,
}

/// Сообщение события в шине сообщений платформы
///
/// Передает структурированные данные между ядром, плагинами и внешними WebSocket клиентами.
///
/// # Примеры
/// ```rust
/// use aethercore_common::models::events::EventMessage;
/// use serde_json::json;
///
/// // Создание системного события
/// let event = EventMessage::reliable("users.created", "users", json!({ "username": "admin" }));
/// assert_eq!(event.topic, "users.created");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventMessage {
    /// Уникальный идентификатор сообщения (UUID v4)
    pub id: Uuid,
    /// Топик/тема события (например, `"users.created"`, `"system.started"`, `"snmp.metrics"`)
    pub topic: String,
    /// Тип доставки ([`EventType::Telemetry`] или [`EventType::Reliable`])
    pub event_type: EventType,
    /// Источник события (`"core"` или идентификатор модуля-издателя)
    pub source: String,
    /// Полезная нагрузка события в формате JSON Value
    pub payload: serde_json::Value,
    /// Опциональный бинарный payload (для high-frequency телеметрии, MessagePack или Protobuf)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_payload: Option<Vec<u8>>,
    /// Время создания события (UTC)
    pub timestamp: DateTime<Utc>,
}

impl EventMessage {
    /// Создать новое эфемеренное событие телеметрии (высокочастотная доставка только в оперативную память)
    ///
    /// # Аргументы
    /// * `topic` — Тема события (например, `"devices.metrics"`).
    /// * `source` — Идентификатор отправителя (например, `"ping-collector"`).
    /// * `payload` — Сериализованные в JSON данные события.
    ///
    /// # Возвращаемое значение
    /// Новый экземпляр [`EventMessage`] с типом [`EventType::Telemetry`].
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

    /// Создать новое системное событие с гарантированной персистентной записью в журнал SQLite WAL
    ///
    /// # Аргументы
    /// * `topic` — Тема события (например, `"users.created"`).
    /// * `source` — Идентификатор отправителя (например, `"core"`).
    /// * `payload` — Сериализованные в JSON данные события.
    ///
    /// # Возвращаемое значение
    /// Новый экземпляр [`EventMessage`] с типом [`EventType::Reliable`].
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

/// Персистентная запись в надежном журнале системных событий (таблица SQLite `event_journal`)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReliableEventRecord {
    /// Порядковый автоинкрементный номер записи в базе данных
    pub id: i64,
    /// Уникальный UUID события
    pub event_uuid: Uuid,
    /// Топик/тема события
    pub topic: String,
    /// Идентификатор подсистемы или плагина-источника
    pub source: String,
    /// Строковое JSON-представление полезной нагрузки
    pub payload_json: String,
    /// Временная метка фиксации события в журнале (UTC)
    pub created_at: DateTime<Utc>,
}
