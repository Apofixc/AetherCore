//! # Топология и граф потоков данных шины событий (BusTopologyTracker)
//!
//! Отслеживает связи в реальном времени:
//! - **Издатели (Publishers)**: источники сообщений (`core:scheduler`, `plugin:zigbee`, `user:admin`).
//! - **Темы (Topics)**: активные каналы событий.
//! - **Подписчики (Subscribers)**: активные слушатели и плагины.
//!
//! Формирует структуры узлов и ребер графа для интерактивной визуализации в веб-интерфейсе.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::router::SubscriptionId;

/// Тип узла в топологическом графе
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyNodeType {
    /// Источник/издатель событий (модуль, плагин или внешний клиент)
    Publisher,
    /// Тема/канал сообщений
    Topic,
    /// Подписчик/потребитель событий
    Subscriber,
}

/// Узел графа топологии
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopologyNode {
    /// Уникальный идентификатор узла (например, `"pub:plugin:zigbee"`, `"topic:sensors.temp"`, `"sub:1"`)
    pub id: String,
    /// Отображаемое название узла
    pub label: String,
    /// Тип узла
    pub node_type: TopologyNodeType,
    /// Общее количество переданных/принятых сообщений
    pub message_count: u64,
    /// Время последней активности (UTC)
    pub last_active: DateTime<Utc>,
}

/// Ребро (связь) в графе топологии
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopologyEdge {
    /// Идентификатор исходного узла
    pub source_id: String,
    /// Идентификатор целевого узла
    pub target_id: String,
    /// Тип связи: `"publishes"` (Publisher -> Topic) или `"subscribes"` (Topic -> Subscriber)
    pub edge_type: String,
    /// Количество переданных сообщений по данной связи
    pub message_count: u64,
    /// Время последнего события по данной связи (UTC)
    pub last_seen: DateTime<Utc>,
}

/// Моментальный снимок графа топологии для визуализации в UI
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BusTopologySnapshot {
    /// Список узлов графа
    pub nodes: Vec<TopologyNode>,
    /// Список связей между узлами
    pub edges: Vec<TopologyEdge>,
    /// Общее количество активных издателей
    pub publishers_count: usize,
    /// Общее количество активных подписчиков
    pub subscribers_count: usize,
    /// Общее количество зарегистрированных топиков
    pub topics_count: usize,
    /// Временная метка генерации снимка (UTC)
    pub generated_at: DateTime<Utc>,
}

/// Внутреннее состояние издателя
#[derive(Debug, Clone)]
struct PublisherState {
    name: String,
    topics: HashMap<String, u64>,
    total_messages: u64,
    last_active: DateTime<Utc>,
}

/// Внутреннее состояние подписчика
#[derive(Debug, Clone)]
struct SubscriberState {
    name: String,
    patterns: Vec<String>,
    received_count: u64,
    last_active: DateTime<Utc>,
}

/// Потокобезопасный трекер топологии шины событий
#[derive(Debug, Default, Clone)]
pub struct BusTopologyTracker {
    inner: Arc<RwLock<TopologyTrackerInner>>,
}

#[derive(Debug, Default)]
struct TopologyTrackerInner {
    publishers: HashMap<String, PublisherState>,
    subscribers: HashMap<SubscriptionId, SubscriberState>,
    topic_message_counts: HashMap<String, u64>,
    topic_last_active: HashMap<String, DateTime<Utc>>,
}

/// Максимальное количество удерживаемых уникальных топиков в топологии
pub const MAX_TOPOLOGY_TOPICS: usize = 2000;
/// Максимальное количество удерживаемых уникальных издателей в топологии
pub const MAX_TOPOLOGY_PUBLISHERS: usize = 1000;

impl BusTopologyTracker {
    /// Создать новый экземпляр трекера топологии
    pub fn new() -> Self {
        Self::default()
    }

    /// Зафиксировать публикацию сообщения от указанного источника в топик
    pub fn record_publish(&self, source: &str, topic: &str) {
        // Игнорируем временные системные RPC-ответы для предотвращения засорения топологии
        if topic.starts_with("_reply.") {
            return;
        }

        let now = Utc::now();
        if let Ok(mut inner) = self.inner.write() {
            // Защита от неограниченного роста памяти при динамических топиках
            if !inner.topic_message_counts.contains_key(topic)
                && inner.topic_message_counts.len() >= MAX_TOPOLOGY_TOPICS
            {
                if let Some(oldest_topic) = inner
                    .topic_last_active
                    .iter()
                    .min_by_key(|(_, dt)| **dt)
                    .map(|(t, _)| t.clone())
                {
                    inner.topic_message_counts.remove(&oldest_topic);
                    inner.topic_last_active.remove(&oldest_topic);
                    for pub_state in inner.publishers.values_mut() {
                        pub_state.topics.remove(&oldest_topic);
                    }
                }
            }

            // 1. Обновление статистики топика
            let topic_count = inner.topic_message_counts.entry(topic.to_string()).or_insert(0);
            *topic_count += 1;
            inner.topic_last_active.insert(topic.to_string(), now);

            // Защита от неограниченного роста памяти при динамических издателях (users, sessions)
            if !inner.publishers.contains_key(source) && inner.publishers.len() >= MAX_TOPOLOGY_PUBLISHERS {
                if let Some(oldest_publisher) = inner
                    .publishers
                    .iter()
                    .min_by_key(|(_, state)| state.last_active)
                    .map(|(k, _)| k.clone())
                {
                    inner.publishers.remove(&oldest_publisher);
                }
            }

            // 2. Обновление статистики издателя
            let pub_state = inner.publishers.entry(source.to_string()).or_insert_with(|| PublisherState {
                name: source.to_string(),
                topics: HashMap::new(),
                total_messages: 0,
                last_active: now,
            });

            pub_state.total_messages += 1;
            pub_state.last_active = now;
            let topic_pub_count = pub_state.topics.entry(topic.to_string()).or_insert(0);
            *topic_pub_count += 1;
        }
    }

    /// Зарегистрировать активную подписку
    pub fn register_subscriber(&self, sub_id: SubscriptionId, name: Option<String>, patterns: &[String]) {
        let now = Utc::now();
        let display_name = name.unwrap_or_else(|| format!("sub#{}", sub_id));
        let filtered_patterns: Vec<String> = patterns
            .iter()
            .filter(|p| !p.starts_with("_reply."))
            .cloned()
            .collect();

        if let Ok(mut inner) = self.inner.write() {
            inner.subscribers.insert(
                sub_id,
                SubscriberState {
                    name: display_name,
                    patterns: filtered_patterns,
                    received_count: 0,
                    last_active: now,
                },
            );
        }
    }

    /// Обновить имя существующей подписки
    pub fn set_subscriber_name(&self, sub_id: SubscriptionId, name: String) {
        if let Ok(mut inner) = self.inner.write() {
            if let Some(sub) = inner.subscribers.get_mut(&sub_id) {
                sub.name = name;
            }
        }
    }

    /// Добавить шаблон топика к зарегистрированной подписке
    pub fn add_subscriber_pattern(&self, sub_id: SubscriptionId, pattern: String) {
        if pattern.starts_with("_reply.") {
            return;
        }
        if let Ok(mut inner) = self.inner.write() {
            if let Some(sub) = inner.subscribers.get_mut(&sub_id) {
                if !sub.patterns.contains(&pattern) {
                    sub.patterns.push(pattern);
                }
            }
        }
    }

    /// Удалить шаблон топика из зарегистрированной подписки
    pub fn remove_subscriber_pattern(&self, sub_id: SubscriptionId, pattern: &str) {
        if let Ok(mut inner) = self.inner.write() {
            if let Some(sub) = inner.subscribers.get_mut(&sub_id) {
                sub.patterns.retain(|p| p != pattern);
            }
        }
    }

    /// Удалить подписку при закрытии/деструкции (`Drop`)
    pub fn unregister_subscriber(&self, sub_id: SubscriptionId) {
        if let Ok(mut inner) = self.inner.write() {
            inner.subscribers.remove(&sub_id);
        }
    }

    /// Зафиксировать доставку сообщения подписчику
    pub fn record_delivery(&self, sub_id: SubscriptionId) {
        let now = Utc::now();
        if let Ok(mut inner) = self.inner.write() {
            if let Some(sub) = inner.subscribers.get_mut(&sub_id) {
                sub.received_count += 1;
                sub.last_active = now;
            }
        }
    }

    /// Получить моментальный снимок топологического графа для UI
    pub fn snapshot(&self) -> BusTopologySnapshot {
        let now = Utc::now();
        let inner = match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };

        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // 1. Узлы издателей и ребра публикации в топики
        for (pub_id, pub_state) in &inner.publishers {
            let node_id = format!("pub:{}", pub_id);
            nodes.push(TopologyNode {
                id: node_id.clone(),
                label: pub_state.name.clone(),
                node_type: TopologyNodeType::Publisher,
                message_count: pub_state.total_messages,
                last_active: pub_state.last_active,
            });

            for (topic, count) in &pub_state.topics {
                edges.push(TopologyEdge {
                    source_id: node_id.clone(),
                    target_id: format!("topic:{}", topic),
                    edge_type: "publishes".to_string(),
                    message_count: *count,
                    last_seen: pub_state.last_active,
                });
            }
        }

        // 2. Узлы топиков
        for (topic, count) in &inner.topic_message_counts {
            let last_active = inner.topic_last_active.get(topic).copied().unwrap_or(now);
            nodes.push(TopologyNode {
                id: format!("topic:{}", topic),
                label: topic.clone(),
                node_type: TopologyNodeType::Topic,
                message_count: *count,
                last_active,
            });
        }

        // 3. Узлы подписчиков и ребра подписок
        for (sub_id, sub_state) in &inner.subscribers {
            let node_id = format!("sub:{}", sub_id);
            nodes.push(TopologyNode {
                id: node_id.clone(),
                label: sub_state.name.clone(),
                node_type: TopologyNodeType::Subscriber,
                message_count: sub_state.received_count,
                last_active: sub_state.last_active,
            });

            for pattern in &sub_state.patterns {
                let target_topic_node = format!("topic:{}", pattern);
                edges.push(TopologyEdge {
                    source_id: target_topic_node,
                    target_id: node_id.clone(),
                    edge_type: "subscribes".to_string(),
                    message_count: sub_state.received_count,
                    last_seen: sub_state.last_active,
                });
            }
        }

        BusTopologySnapshot {
            publishers_count: inner.publishers.len(),
            subscribers_count: inner.subscribers.len(),
            topics_count: inner.topic_message_counts.len(),
            nodes,
            edges,
            generated_at: now,
        }
    }
}
