//! # Модели событий шины сообщений и журнала событий
//!
//! Определяет структуру сообщений событий платформы ([`EventMessage`]),
//! приоритеты ([`EventPriority`]), режимы доставки ([`EventType`])
//! и формат записей журнала ([`ReliableEventRecord`]).

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Уровни приоритета доставки событий в шине сообщений
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventPriority {
    /// Фоновая телеметрия, отладочные метрики, низкоприоритетные уведомления
    Low = 0,
    /// Стандартные события устройств и состояния (по умолчанию)
    #[default]
    Normal = 1,
    /// Важные пользовательские действия, триггеры правил, изменения настроек
    High = 2,
    /// Системные алармы, аварии оборудования, инциденты безопасности
    Critical = 3,
}

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
/// Поддерживает приоритезацию, время жизни (TTL), трассировку и паттерн Request-Reply RPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventMessage {
    /// Уникальный идентификатор сообщения (UUID v4)
    pub id: Uuid,
    /// Топик/тема события (например, `"devices.switch1.metrics"`, `"users.created"`, `"alarms.critical"`)
    pub topic: String,
    /// Тип доставки ([`EventType::Telemetry`] или [`EventType::Reliable`])
    #[serde(default)]
    pub event_type: EventType,
    /// Приоритет сообщения ([`EventPriority`])
    #[serde(default)]
    pub priority: EventPriority,
    /// Источник события (`"core"` или идентификатор модуля-издателя)
    pub source: String,
    /// Полезная нагрузка события в формате JSON Value
    pub payload: serde_json::Value,
    /// Опциональный бинарный payload (для high-frequency телеметрии, MessagePack или Protobuf)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub binary_payload: Option<Vec<u8>>,
    /// Время создания события (UTC)
    pub timestamp: DateTime<Utc>,
    /// Время истечения актуальности (TTL). Если None — бессрочно (для постоянного аудита)
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub expires_at: Option<DateTime<Utc>>,
    /// Идентификатор корреляции для Request-Reply RPC
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub correlation_id: Option<Uuid>,
    /// Топик ответа для двунаправленного Request-Reply RPC
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reply_to: Option<String>,
}

impl EventMessage {
    /// Создать новое эфемеренное событие телеметрии
    ///
    /// По умолчанию имеет приоритет [`EventPriority::Normal`] и срок актуальности 30 минут.
    pub fn telemetry(topic: impl Into<String>, source: impl Into<String>, payload: serde_json::Value) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            topic: topic.into(),
            event_type: EventType::Telemetry,
            priority: EventPriority::Normal,
            source: source.into(),
            payload,
            binary_payload: None,
            timestamp: now,
            expires_at: Some(now + Duration::minutes(30)),
            correlation_id: None,
            reply_to: None,
        }
    }

    /// Создать новое надежное системное событие
    ///
    /// По умолчанию имеет приоритет [`EventPriority::High`] и бессрочное хранение (`expires_at: None`).
    pub fn reliable(topic: impl Into<String>, source: impl Into<String>, payload: serde_json::Value) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            topic: topic.into(),
            event_type: EventType::Reliable,
            priority: EventPriority::High,
            source: source.into(),
            payload,
            binary_payload: None,
            timestamp: now,
            expires_at: None,
            correlation_id: None,
            reply_to: None,
        }
    }

    /// Установить приоритет события
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Установить относительное время жизни события (TTL)
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.expires_at = Some(self.timestamp + ttl);
        self
    }

    /// Установить точное время истечения актуальности
    pub fn with_expires_at(mut self, expires_at: Option<DateTime<Utc>>) -> Self {
        self.expires_at = expires_at;
        self
    }

    /// Установить параметры корреляции для Request-Reply RPC
    pub fn with_correlation(mut self, correlation_id: Uuid, reply_to: Option<String>) -> Self {
        self.correlation_id = Some(correlation_id);
        self.reply_to = reply_to;
        self
    }

    /// Проверить, истек ли срок актуальности события относительно текущего момента времени
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            exp < Utc::now()
        } else {
            false
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
