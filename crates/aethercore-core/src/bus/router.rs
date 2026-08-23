//! # Маршрутизатор топиков на префиксном дереве (TopicTrie)
//!
//! Поддерживает подписки по маскам топиков:
//! - `*` (одиночный wildcard) — сопоставляет ровно один сегмент топика.
//! - `#` (многоуровневый wildcard) — сопоставляет произвольное количество сегментов в хвосте.
//!
//! Предоставляет потокобезопасный [`TopicRouter`] и RAII-дескриптор [`SubscriptionHandle`],
//! который автоматически отписывается при выходе из области видимости (`Drop`).

use aethercore_common::models::events::EventMessage;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use tracing::trace;

/// Уникальный идентификатор активной подписки
pub type SubscriptionId = u64;

/// Генератор монотонных идентификаторов подписок
static SUB_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Вместимость буфера очереди отдельного подписчика
pub const SUBSCRIBER_QUEUE_CAPACITY: usize = 1024;

/// Узел префиксного дерева маршрутизации топиков
#[derive(Debug, Default)]
struct TopicTrieNode {
    /// Подписчики, чей точный шаблон совпадает с этим узлом
    subscribers: HashSet<SubscriptionId>,
    /// Подписчики с многоуровневым суффиксом `#` на этом узле
    multi_wildcard_subscribers: HashSet<SubscriptionId>,
    /// Одиночный wildcard `*`
    single_wildcard: Option<Box<TopicTrieNode>>,
    /// Точные дочерние сегменты
    children: HashMap<String, TopicTrieNode>,
}

impl TopicTrieNode {
    fn insert(&mut self, segments: &[&str], sub_id: SubscriptionId) {
        if segments.is_empty() {
            self.subscribers.insert(sub_id);
            return;
        }

        let first = segments[0];
        let rest = &segments[1..];

        if first == "#" {
            self.multi_wildcard_subscribers.insert(sub_id);
            // '#' поглощает все последующие уровни, глубже спускаться не требуется
            return;
        }

        if first == "*" {
            let node = self.single_wildcard.get_or_insert_with(Default::default);
            node.insert(rest, sub_id);
        } else {
            let node = self.children.entry(first.to_string()).or_default();
            node.insert(rest, sub_id);
        }
    }

    fn remove(&mut self, segments: &[&str], sub_id: SubscriptionId) -> bool {
        if segments.is_empty() {
            self.subscribers.remove(&sub_id);
            return self.is_empty();
        }

        let first = segments[0];
        let rest = &segments[1..];

        if first == "#" {
            self.multi_wildcard_subscribers.remove(&sub_id);
            return self.is_empty();
        }

        if first == "*" {
            if let Some(node) = self.single_wildcard.as_mut() {
                if node.remove(rest, sub_id) {
                    self.single_wildcard = None;
                }
            }
        } else if let Some(node) = self.children.get_mut(first) {
            if node.remove(rest, sub_id) {
                self.children.remove(first);
            }
        }

        self.is_empty()
    }

    fn match_segments(&self, segments: &[&str], matched: &mut HashSet<SubscriptionId>) {
        // Все подписчики с '#' на текущем узле ловят этот топик и всю вложенность
        matched.extend(&self.multi_wildcard_subscribers);

        if segments.is_empty() {
            matched.extend(&self.subscribers);
            return;
        }

        let first = segments[0];
        let rest = &segments[1..];

        // 1. Точное совпадение сегмента
        if let Some(child) = self.children.get(first) {
            child.match_segments(rest, matched);
        }

        // 2. Совпадение по одиночному wildcard '*'
        if let Some(single) = self.single_wildcard.as_ref() {
            single.match_segments(rest, matched);
        }
    }

    fn is_empty(&self) -> bool {
        self.subscribers.is_empty()
            && self.multi_wildcard_subscribers.is_empty()
            && self.single_wildcard.as_ref().map_or(true, |n| n.is_empty())
            && self.children.is_empty()
    }
}

/// Состояние отдельного подписчика в роутере
#[derive(Debug)]
struct SubscriberEntry {
    tx: mpsc::Sender<EventMessage>,
    patterns: HashSet<String>,
}

/// Внутреннее состояние потокобезопасного роутера топиков
#[derive(Default, Debug)]
struct TopicRouterInner {
    trie: TopicTrieNode,
    subscribers: HashMap<SubscriptionId, SubscriberEntry>,
}

/// Маршрутизатор топиков событий платформы
#[derive(Clone, Default, Debug)]
pub struct TopicRouter {
    inner: Arc<RwLock<TopicRouterInner>>,
}

impl TopicRouter {
    /// Создать новый экземпляр роутера топиков
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(TopicRouterInner::default())),
        }
    }

    /// Создать новую подписку на указанные темы и шаблоны масок
    ///
    /// # Аргументы
    /// * `patterns` — Набор шаблонов топиков (например, `&["devices.*.status", "alarms.#"]`).
    ///
    /// # Возвращаемое значение
    /// RAII-дескриптор [`SubscriptionHandle`], удаляющий подписку при `Drop`.
    pub fn subscribe(&self, patterns: &[&str]) -> SubscriptionHandle {
        let sub_id = SUB_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel(SUBSCRIBER_QUEUE_CAPACITY);

        let mut patterns_set = HashSet::new();
        {
            let mut guard = self.inner.write().unwrap();
            for pattern in patterns {
                let pat_str = pattern.trim();
                if !pat_str.is_empty() {
                    patterns_set.insert(pat_str.to_string());
                    let segments: Vec<&str> = pat_str.split('.').collect();
                    guard.trie.insert(&segments, sub_id);
                }
            }

            guard.subscribers.insert(
                sub_id,
                SubscriberEntry {
                    tx,
                    patterns: patterns_set,
                },
            );
        }

        SubscriptionHandle {
            id: sub_id,
            rx,
            router: self.clone(),
        }
    }

    /// Динамически добавить шаблон топика к активной подписке
    ///
    /// # Аргументы
    /// * `sub_id` — Идентификатор активной подписки ([`SubscriptionId`]).
    /// * `pattern` — Добавляемый шаблон топика.
    pub fn add_topic(&self, sub_id: SubscriptionId, pattern: &str) {
        let pat_str = pattern.trim();
        if pat_str.is_empty() {
            return;
        }

        let mut guard = self.inner.write().unwrap();
        if let Some(entry) = guard.subscribers.get_mut(&sub_id) {
            if entry.patterns.insert(pat_str.to_string()) {
                let segments: Vec<&str> = pat_str.split('.').collect();
                guard.trie.insert(&segments, sub_id);
            }
        }
    }

    /// Динамически удалить шаблон топика из активной подписки
    ///
    /// # Аргументы
    /// * `sub_id` — Идентификатор активной подписки ([`SubscriptionId`]).
    /// * `pattern` — Удаляемый шаблон топика.
    pub fn remove_topic(&self, sub_id: SubscriptionId, pattern: &str) {
        let pat_str = pattern.trim();
        if pat_str.is_empty() {
            return;
        }

        let mut guard = self.inner.write().unwrap();
        if let Some(entry) = guard.subscribers.get_mut(&sub_id) {
            if entry.patterns.remove(pat_str) {
                let segments: Vec<&str> = pat_str.split('.').collect();
                guard.trie.remove(&segments, sub_id);
            }
        }
    }

    /// Полностью удалить подписчика (вызывается автоматически из [`SubscriptionHandle::drop`])
    ///
    /// # Аргументы
    /// * `sub_id` — Идентификатор удаляемой подписки.
    pub fn unsubscribe(&self, sub_id: SubscriptionId) {
        let mut guard = self.inner.write().unwrap();
        if let Some(entry) = guard.subscribers.remove(&sub_id) {
            for pattern in &entry.patterns {
                let segments: Vec<&str> = pattern.split('.').collect();
                guard.trie.remove(&segments, sub_id);
            }
        }
    }

    /// Отправить событие всем подходящим подписчикам по дереву топиков
    ///
    /// # Аргументы
    /// * `event` — Доставляемое событие платформы ([`EventMessage`]).
    ///
    /// # Возвращаемое значение
    /// Количество подписчиков, в чьи очереди было успешно помещено событие.
    pub fn dispatch(&self, event: &EventMessage) -> usize {
        let segments: Vec<&str> = event.topic.split('.').collect();
        let mut target_ids = HashSet::new();

        {
            let guard = self.inner.read().unwrap();
            guard.trie.match_segments(&segments, &mut target_ids);
        }

        if target_ids.is_empty() {
            return 0;
        }

        let mut delivered = 0;
        let guard = self.inner.read().unwrap();
        for id in target_ids {
            if let Some(entry) = guard.subscribers.get(&id) {
                // Пытаемся отправить без блокировки (non-blocking try_send)
                // Если буфер подписчика переполнен, фиксируем lag
                match entry.tx.try_send(event.clone()) {
                    Ok(_) => delivered += 1,
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        trace!("Subscriber queue full for sub_id {}, dropped event {}", id, event.id);
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        trace!("Subscriber queue closed for sub_id {}", id);
                    }
                }
            }
        }

        delivered
    }

    /// Получить текущее количество активных зарегистрированных подписчиков
    pub fn subscriber_count(&self) -> usize {
        self.inner.read().unwrap().subscribers.len()
    }
}

/// RAII-дескриптор подписки на события шины
///
/// Автоматически удаляет регистрацию топиков из маршрутизатора при выходе из области видимости (`Drop`).
pub struct SubscriptionHandle {
    id: SubscriptionId,
    rx: mpsc::Receiver<EventMessage>,
    router: TopicRouter,
}

impl SubscriptionHandle {
    /// Получить уникальный идентификатор подписки
    pub fn id(&self) -> SubscriptionId {
        self.id
    }

    /// Асинхронно прочитать следующее событие из очереди
    ///
    /// Возвращает `None`, если шина событий была остановлена.
    pub async fn recv(&mut self) -> Option<EventMessage> {
        self.rx.recv().await
    }

    /// Попробовать прочитать следующее событие без блокировки
    pub fn try_recv(&mut self) -> std::result::Result<EventMessage, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }

    /// Динамически добавить топик или маску к данной подписке
    ///
    /// # Аргументы
    /// * `pattern` — Шаблон топика (например, `"alarms.fire"`).
    pub fn add_topic(&self, pattern: impl AsRef<str>) {
        self.router.add_topic(self.id, pattern.as_ref());
    }

    /// Динамически удалить топик или маску из данной подписки
    ///
    /// # Аргументы
    /// * `pattern` — Шаблон топика для удаления.
    pub fn remove_topic(&self, pattern: impl AsRef<str>) {
        self.router.remove_topic(self.id, pattern.as_ref());
    }
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        self.router.unsubscribe(self.id);
    }
}
