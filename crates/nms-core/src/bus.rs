use crate::exceptions::NmsError;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock, RwLock};
use tokio::sync::broadcast;
use tracing::{error, info};

/// Структура сообщения системного события
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seq_id: Option<i64>,
    pub topic: String,
    pub payload: serde_json::Value,
    pub sender: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl SystemEvent {
    /// Создание нового системного события
    pub fn new(
        topic: impl Into<String>,
        payload: serde_json::Value,
        sender: impl Into<String>,
    ) -> Self {
        Self {
            seq_id: None,
            topic: topic.into(),
            payload,
            sender: sender.into(),
            target_user_id: None,
            created_at: None,
        }
    }

    /// Добавление целевого пользователя (target_user_id)
    pub fn with_target_user(mut self, target_user_id: impl Into<String>) -> Self {
        self.target_user_id = Some(target_user_id.into());
        self
    }
}

/// Трейт для удобного преобразования аргументов в SystemEvent при публикации
pub trait IntoSystemEvent {
    fn into_system_event(self) -> SystemEvent;
}

impl IntoSystemEvent for SystemEvent {
    fn into_system_event(self) -> SystemEvent {
        self
    }
}

impl IntoSystemEvent for &SystemEvent {
    fn into_system_event(self) -> SystemEvent {
        self.clone()
    }
}

impl IntoSystemEvent for (&str, serde_json::Value) {
    fn into_system_event(self) -> SystemEvent {
        SystemEvent::new(self.0, self.1, "system")
    }
}

impl IntoSystemEvent for (&str, serde_json::Value, &str) {
    fn into_system_event(self) -> SystemEvent {
        SystemEvent::new(self.0, self.1, self.2)
    }
}

/// Статистика шины событий NMS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusStats {
    pub receiver_count: usize,
    pub callback_count: usize,
    pub callback_patterns: Vec<String>,
}

/// Асинхронный/синхронный замыкаемый обработчик событий
pub type EventCallback = Arc<dyn Fn(&SystemEvent) + Send + Sync + 'static>;

/// Кэшированная структура данных подписчика (1-в-1 с Python Subscriber)
#[derive(Clone)]
pub struct Subscriber {
    pub pattern: String,
    pub callback: EventCallback,
    pub has_wildcard: bool,
}

impl Subscriber {
    pub fn new(pattern: impl Into<String>, callback: EventCallback) -> Self {
        let pattern = pattern.into();
        let has_wildcard = pattern.contains('*') || pattern.contains('+') || pattern.contains('#');
        Self {
            pattern,
            callback,
            has_wildcard,
        }
    }
}

/// Вспомогательная функция проверки количества параметров обработчика (1-в-1 с Python _inspect_subscriber_params)
pub fn _inspect_subscriber_params() -> usize {
    1
}

/// Проверка соответствия названия топика (topic) маске подписки (pattern)
/// Поддерживает символы wildcard:
/// - '*' или '#': полное совпадение с любым топиком
/// - '+' или '*': 1 любой сегмент топика
/// - '#' в конце маски (segment.#): совпадает со всеми хвостовыми сегментами
pub fn match_topic(pattern: &str, topic: &str) -> bool {
    let p_clean = pattern.trim();
    let t_clean = topic.trim();

    if p_clean == "*" || p_clean == "#" || p_clean == t_clean {
        return true;
    }

    if !p_clean.contains('*') && !p_clean.contains('+') && !p_clean.contains('#') {
        return false;
    }

    let p_parts: Vec<&str> = p_clean.split('.').collect();
    let t_parts: Vec<&str> = t_clean.split('.').collect();

    if let Some(&"#") = p_parts.last() {
        let prefix_parts = &p_parts[..p_parts.len() - 1];
        if t_parts.len() < prefix_parts.len() {
            return false;
        }
        return prefix_parts
            .iter()
            .zip(t_parts.iter())
            .all(|(&p, &t)| p == "*" || p == "+" || p == t);
    }

    if p_parts.len() != t_parts.len() {
        return false;
    }

    p_parts
        .iter()
        .zip(t_parts.iter())
        .all(|(&p, &t)| p == "*" || p == "+" || p == t)
}

/// Шина событий ядра NMS на базе broadcast канала и менеджера локальных подписок
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<SystemEvent>,
    subscribers: Arc<RwLock<Vec<Subscriber>>>,
}

impl EventBus {
    /// Создание новой шины событий с буфером каналов
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        info!("EventBus initialized with capacity {}", capacity);
        Self {
            sender,
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Публикация события в шину NMS
    /// Флаг is_core блокирует внешнюю публикацию в зарезервированные системные топики core.*
    pub fn publish(&self, event: impl IntoSystemEvent, is_core: bool) -> Result<usize, NmsError> {
        let event = event.into_system_event();
        if event.topic.starts_with("core.") && !is_core {
            return Err(NmsError::PermissionDenied {
                message: format!(
                    "Topics starting with 'core.' are reserved for core system code: {}",
                    event.topic
                ),
            });
        }

        // Вызов зарегистрированных In-Process callback обработчиков с изоляцией падений
        if let Ok(guard) = self.subscribers.read() {
            for sub in guard.iter() {
                if match_topic(&sub.pattern, &event.topic) {
                    let cb_clone = Arc::clone(&sub.callback);
                    let event_clone = event.clone();
                    if let Err(err) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        cb_clone(&event_clone);
                    })) {
                        error!(
                            "Event callback panicked for topic '{}': {:?}",
                            event.topic, err
                        );
                    }
                }
            }
        }

        if self.sender.receiver_count() == 0 {
            return Ok(0);
        }

        match self.sender.send(event) {
            Ok(count) => Ok(count),
            Err(_) => Ok(0),
        }
    }

    /// Публикация события с указанием топика и полезной нагрузки (1-в-1 с Python publish(topic, payload, is_core))
    pub fn publish_topic(
        &self,
        topic: &str,
        payload: serde_json::Value,
        is_core: bool,
    ) -> Result<usize, NmsError> {
        let event = SystemEvent::new(topic, payload, "system");
        self.publish(event, is_core)
    }

    /// Зарегистрировать локальный In-Process callback подписчик (1-в-1 с Python subscribe(pattern, handler))
    pub fn subscribe(&self, pattern: impl Into<String>, callback: EventCallback) {
        let sub = Subscriber::new(pattern, callback);
        if let Ok(mut guard) = self.subscribers.write() {
            guard.push(sub);
        }
    }

    /// Псевдоним подписки callback-обработчика для обратной совместимости
    pub fn subscribe_callback(&self, pattern: impl Into<String>, callback: EventCallback) {
        self.subscribe(pattern, callback);
    }

    /// Получить Tokio broadcast приемник для асинхронных каналов вещания WebSocket/SSE
    pub fn subscribe_receiver(&self) -> broadcast::Receiver<SystemEvent> {
        self.sender.subscribe()
    }

    /// Отписать локальные callback подписчики по маске (1-в-1 с Python unsubscribe(pattern))
    pub fn unsubscribe(&self, pattern: &str) -> bool {
        if let Ok(mut guard) = self.subscribers.write() {
            let initial_len = guard.len();
            guard.retain(|sub| sub.pattern != pattern);
            guard.len() < initial_len
        } else {
            false
        }
    }

    /// Псевдоним отписки callback-обработчика для обратной совместимости
    pub fn unsubscribe_callback(&self, pattern: &str) {
        self.unsubscribe(pattern);
    }

    /// Очистить все зарегистрированные callback подписчики (1-в-1 с Python clear())
    pub fn clear(&self) {
        if let Ok(mut guard) = self.subscribers.write() {
            guard.clear();
        }
    }

    /// Псевдоним очистки подписчиков для обратной совместимости
    pub fn clear_callbacks(&self) {
        self.clear();
    }

    /// Получить статистику шины событий (1-в-1 с Python get_stats())
    pub fn get_stats(&self) -> EventBusStats {
        let (callback_count, callback_patterns) = if let Ok(guard) = self.subscribers.read() {
            let patterns: Vec<String> = guard.iter().map(|s| s.pattern.clone()).collect();
            (guard.len(), patterns)
        } else {
            (0, Vec::new())
        };

        EventBusStats {
            receiver_count: self.sender.receiver_count(),
            callback_count,
            callback_patterns,
        }
    }

    /// Завершение работы шины событий (1-в-1 с Python shutdown())
    pub fn shutdown(&self) {
        self.clear();
        info!("EventBus shutdown completed");
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(2048)
    }
}

/// Глобальный инстанс шины событий (1-в-1 с Python event_bus = EventBus())
pub static EVENT_BUS: LazyLock<EventBus> = LazyLock::new(|| EventBus::new(2048));

/// Вспомогательный геттер к глобальной шине событий
pub fn event_bus() -> &'static EventBus {
    &EVENT_BUS
}
