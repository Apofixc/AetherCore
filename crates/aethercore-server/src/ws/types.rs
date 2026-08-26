//! # Типы протокола WebSocket-шлюза
//!
//! Определяет входящие команды клиента ([`WsClientCommand`]), исходящие сообщения
//! сервера ([`WsServerMessage`]) и кодек форматирования ([`WsCodecFormat`]) для поддержки
//! текстового JSON и компактного бинарного MessagePack.

use aethercore_common::models::events::{EventMessage, EventPriority, EventType};
use axum::extract::ws::Message;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Допустимые форматы сериализации данных в WebSocket-соединении
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WsCodecFormat {
    /// Стандартный текстовый JSON (`aethercore.json`)
    #[default]
    Json,
    /// Компактный бинарный MessagePack (`aethercore.msgpack`)
    MessagePack,
}

impl WsCodecFormat {
    /// Десериализовать входящий WebSocket-фрейм в тип команды
    pub fn decode<T: for<'de> Deserialize<'de>>(&self, msg: &Message) -> Result<T, String> {
        match (self, msg) {
            (WsCodecFormat::Json, Message::Text(text)) => {
                serde_json::from_str(text).map_err(|e| format!("JSON decode error: {}", e))
            }
            (WsCodecFormat::MessagePack, Message::Binary(bin)) => {
                rmp_serde::from_slice(bin).map_err(|e| format!("MessagePack decode error: {}", e))
            }
            // Автоматическое переключение, если формат совпадает с типом сообщения
            (_, Message::Text(text)) => {
                serde_json::from_str(text).map_err(|e| format!("JSON decode error: {}", e))
            }
            (_, Message::Binary(bin)) => {
                rmp_serde::from_slice(bin).map_err(|e| format!("MessagePack decode error: {}", e))
            }
            _ => Err("Unsupported frame type for decode".to_string()),
        }
    }

    /// Сериализовать исходящую структуру в WebSocket-сообщение
    pub fn encode<T: Serialize>(&self, value: &T) -> Result<Message, String> {
        match self {
            WsCodecFormat::Json => {
                let s = serde_json::to_string(value)
                    .map_err(|e| format!("JSON encode error: {}", e))?;
                Ok(Message::Text(s.into()))
            }
            WsCodecFormat::MessagePack => {
                let b = rmp_serde::to_vec_named(value)
                    .map_err(|e| format!("MessagePack encode error: {}", e))?;
                Ok(Message::Binary(b.into()))
            }
        }
    }
}

/// Query-параметры подключения к WebSocket потоку событий
#[derive(Debug, Deserialize)]
pub struct WsAuthQuery {
    /// Опциональный JWT токен доступа (`?token=<jwt>`)
    pub token: Option<String>,
    /// Начальные темы подписки через запятую (например, `?topics=devices.*,system.#`)
    pub topics: Option<String>,
    /// Запросить ли сохраненные Retained-состояния топиков при подключении (`?retained=true`)
    pub retained: Option<bool>,
}

/// Входящая управляющая команда от WebSocket клиента
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WsClientCommand {
    /// In-Band авторизация токеном
    Auth {
        /// JWT токен доступа
        token: String,
    },
    /// Добавить подписку на темы
    Subscribe {
        /// Список топиков или масок подписки
        topics: Vec<String>,
        /// Запросить ли сохраненные Retained-состояния топиков
        #[serde(default)]
        with_retained: bool,
    },
    /// Удалить подписку на темы
    Unsubscribe {
        /// Список топиков или масок для отписки
        topics: Vec<String>,
    },
    /// Публикация события / команды в шину ядра
    Publish {
        /// Опциональный идентификатор сообщения для подтверждения (ACK)
        #[serde(default)]
        msg_id: Option<String>,
        /// Опциональный идентификатор вкладки отправителя (для кросс-вкладочной синхронизации)
        #[serde(default)]
        tab_id: Option<String>,
        /// Тема публикации
        topic: String,
        /// Полезная нагрузка события
        payload: serde_json::Value,
        /// Приоритет события
        #[serde(default)]
        priority: EventPriority,
        /// Сохранять ли последнее состояние в Retained Store
        #[serde(default)]
        retain: bool,
    },
    /// Вызов любого REST API метода ядра через сокет (REST-over-WS)
    Call {
        /// Идентификатор запроса для сопоставления с ответом
        request_id: String,
        /// Опциональный идентификатор вкладки отправителя
        #[serde(default)]
        tab_id: Option<String>,
        /// HTTP-метод (GET, POST, PUT, DELETE)
        method: String,
        /// Путь запроса (например `/api/v1/modules`)
        path: String,
        /// Тело запроса (JSON)
        #[serde(default)]
        body: serde_json::Value,
    },
    /// Запрос снимка сохраненных Retained-состояний
    GetState {
        /// Список масок/паттернов топиков
        patterns: Vec<String>,
        /// Максимальный лимит на топик
        #[serde(default = "default_retained_limit")]
        limit_per_topic: usize,
    },
    /// Возобновление сессии и дочитка пропущенных событий из L1 RingBuffer / L2 SQLite ядра
    Resume {
        /// Временная метка, с которой запросить пропущенные события
        since_timestamp: chrono::DateTime<chrono::Utc>,
        /// Опциональный список топиков (если не задан — берутся активные подписки)
        #[serde(default)]
        topics: Option<Vec<String>>,
        /// Лимит событий
        #[serde(default = "default_retained_limit")]
        limit: usize,
    },
    /// Динамическая настройка фильтров потока на клиенте
    SetFilter {
        /// Минимальный приоритет доставляемых событий
        #[serde(default)]
        min_priority: Option<EventPriority>,
        /// Фильтр по типам событий (Telemetry, Reliable)
        #[serde(default)]
        event_types: Option<Vec<EventType>>,
        /// Фильтр по префиксу источника
        #[serde(default)]
        source: Option<String>,
    },
    /// Heartbeat пинг
    Ping,
}

fn default_retained_limit() -> usize {
    20
}

/// Исходящее системное сообщение WebSocket шлюза
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsServerMessage {
    /// Успешная аутентификация пользователя
    Authenticated {
        /// Идентификатор пользователя
        user_id: Uuid,
        /// Имя пользователя
        username: String,
        /// Назначенные роли
        roles: Vec<String>,
        /// Назначенные права
        permissions: Vec<String>,
    },
    /// Событие шины с монотонным порядковым номером для предотвращения Out-of-Order
    Event {
        /// Монотонный Sequence ID для данного подключения
        seq: u64,
        /// Содержимое события шины
        event: EventMessage,
    },
    /// Пакет пропущенных событий при реконнекте (из L1 RingBuffer / L2 SQLite)
    ReplayBatch {
        /// Список пропущенных событий
        events: Vec<EventMessage>,
    },
    /// Подтверждение успешной публикации команды/события
    Ack {
        /// Идентификатор сообщения
        msg_id: String,
        /// Статус ("ok")
        status: String,
    },
    /// Ответ на вызов REST-over-WS (`action: "call"`)
    Response {
        /// Идентификатор запроса
        request_id: String,
        /// Опциональный идентификатор вкладки получателя
        #[serde(skip_serializing_if = "Option::is_none")]
        tab_id: Option<String>,
        /// HTTP статус-код (200, 201, 400, 404, 500)
        status: u16,
        /// Тело ответа
        body: serde_json::Value,
    },
    /// Пакетный снимок актуальных сохраненных состояний
    StateSnapshot {
        /// Список сохраненных событий
        events: Vec<EventMessage>,
    },
    /// Подтверждение успешной подписки
    Subscribed {
        /// Подписанные темы
        topics: Vec<String>,
    },
    /// Подтверждение отписки
    Unsubscribed {
        /// Отписанные темы
        topics: Vec<String>,
    },
    /// Ответ на пинг
    Pong,
    /// Ошибка протокола или авторизации
    Error {
        /// Символьный код ошибки (UNAUTHORIZED, FORBIDDEN, NOT_FOUND, BAD_REQUEST, RATE_LIMITED)
        code: String,
        /// Описание ошибки
        message: String,
        /// Опциональный request_id для ошибок REST-over-WS вызовов
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
}

/// Информация об активном WebSocket-соединении для реестра и мониторинга
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WsConnectionInfo {
    /// Идентификатор подписки на шине
    pub sub_id: u64,
    /// Идентификатор пользователя (если авторизован)
    pub user_id: Option<Uuid>,
    /// Имя пользователя
    pub username: String,
    /// IP-адрес клиента
    pub client_ip: String,
    /// Время непрерывного подключения в секундах
    pub uptime_secs: u64,
    /// Формат данных (Json или MessagePack)
    pub format: String,
    /// Список активных топиков подписки
    pub topics: Vec<String>,
}
