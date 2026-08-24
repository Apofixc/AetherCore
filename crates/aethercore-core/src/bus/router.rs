//! # Маршрутизатор топиков на префиксном дереве (TopicTrie)
//!
//! Поддерживает подписки по маскам топиков:
//! - `*` (одиночный wildcard) — сопоставляет ровно один сегмент топика.
//! - `#` (многоуровневый wildcard) — сопоставляет произвольное количество сегментов в хвосте.
//!
//! Предоставляет потокобезопасный [`TopicRouter`] и RAII-дескриптор [`SubscriptionHandle`],
//! который автоматически отписывается при выходе из области видимости (`Drop`).

use super::topology::BusTopologyTracker;
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

/// Пользовательский предикат контентной фильтрации событий подписки
pub type EventFilter = Arc<dyn Fn(&EventMessage) -> bool + Send + Sync>;

/// Состояние отдельного подписчика в роутере
#[derive(Clone)]
struct SubscriberEntry {
    tx: mpsc::Sender<EventMessage>,
    patterns: HashSet<String>,
    filter: Option<EventFilter>,
}

impl std::fmt::Debug for SubscriberEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriberEntry")
            .field("patterns", &self.patterns)
            .field("has_filter", &self.filter.is_some())
            .finish()
    }
}

/// Внутреннее состояние потокобезопасного роутера топиков
#[derive(Default, Debug)]
struct TopicRouterInner {
    trie: TopicTrieNode,
    subscribers: HashMap<SubscriptionId, SubscriberEntry>,
    topology: Option<BusTopologyTracker>,
}

/// Результат маршрутизации и доставки события подписчикам
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DispatchResult {
    /// Количество успешно доставленных сообщений подписчикам
    pub delivered: usize,
    /// Количество сброшенных сообщений из-за переполнения очередей подписчиков
    pub dropped: usize,
}

/// Маршрутизатор топиков событий платформы
#[derive(Clone, Default, Debug)]
pub struct TopicRouter {
    inner: Arc<RwLock<TopicRouterInner>>,
}

/// Валидация и разбиение шаблона топика на сегменты
fn parse_topic_segments(pattern: &str) -> Option<Vec<&str>> {
    let pat = pattern.trim();
    if pat.is_empty() {
        return None;
    }
    let segments: Vec<&str> = pat.split('.').collect();
    if segments.iter().any(|s| s.is_empty()) {
        return None;
    }
    for (i, &seg) in segments.iter().enumerate() {
        if seg == "#" && i != segments.len() - 1 {
            return None;
        }
    }
    Some(segments)
}

impl TopicRouter {
    /// Создать новый экземпляр роутера топиков
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(TopicRouterInner::default())),
        }
    }

    fn read_guard(&self) -> std::sync::RwLockReadGuard<'_, TopicRouterInner> {
        match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn write_guard(&self) -> std::sync::RwLockWriteGuard<'_, TopicRouterInner> {
        match self.inner.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    /// Привязать трекер топологии к роутеру
    pub fn with_topology(self, topology: BusTopologyTracker) -> Self {
        self.write_guard().topology = Some(topology);
        self
    }

    /// Установить трекер топологии
    pub fn set_topology(&self, topology: BusTopologyTracker) {
        self.write_guard().topology = Some(topology);
    }

    /// Установить отображаемое имя подписчика в топологии
    pub fn set_subscriber_name(&self, sub_id: SubscriptionId, name: String) {
        let guard = self.read_guard();
        if let Some(ref top) = guard.topology {
            top.set_subscriber_name(sub_id, name);
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
        self.subscribe_filtered(patterns, None)
    }

    /// Создать новую подписку с предикатом контентной фильтрации
    pub fn subscribe_filtered(
        &self,
        patterns: &[&str],
        filter: Option<EventFilter>,
    ) -> SubscriptionHandle {
        let sub_id = SUB_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel(SUBSCRIBER_QUEUE_CAPACITY);

        let mut patterns_vec = Vec::new();
        {
            let mut guard = self.write_guard();
            let mut patterns_set = HashSet::new();

            for pat in patterns {
                if let Some(segments) = parse_topic_segments(pat) {
                    let pat_str = pat.trim().to_string();
                    patterns_set.insert(pat_str.clone());
                    patterns_vec.push(pat_str);
                    guard.trie.insert(&segments, sub_id);
                }
            }

            if let Some(ref top) = guard.topology {
                top.register_subscriber(sub_id, None, &patterns_vec);
            }

            guard.subscribers.insert(
                sub_id,
                SubscriberEntry {
                    tx,
                    patterns: patterns_set,
                    filter: filter.clone(),
                },
            );
        }

        SubscriptionHandle {
            id: sub_id,
            name: None,
            rx,
            router: self.clone(),
            filter,
            throttle_state: HashMap::new(),
        }
    }

    /// Установить предикат контентной фильтрации для активной подписки
    pub fn set_filter(&self, sub_id: SubscriptionId, filter: Option<EventFilter>) {
        let mut guard = self.write_guard();
        if let Some(entry) = guard.subscribers.get_mut(&sub_id) {
            entry.filter = filter;
        }
    }

    /// Динамически добавить шаблон топика к активной подписке
    ///
    /// # Аргументы
    /// * `sub_id` — Идентификатор активной подписки ([`SubscriptionId`]).
    /// * `pattern` — Добавляемый шаблон топика.
    pub fn add_topic(&self, sub_id: SubscriptionId, pattern: &str) {
        if let Some(segments) = parse_topic_segments(pattern) {
            let pat_str = pattern.trim().to_string();
            let mut guard = self.write_guard();
            if let Some(entry) = guard.subscribers.get_mut(&sub_id) {
                if entry.patterns.insert(pat_str.clone()) {
                    guard.trie.insert(&segments, sub_id);
                    if let Some(ref top) = guard.topology {
                        top.add_subscriber_pattern(sub_id, pat_str);
                    }
                }
            }
        }
    }

    /// Динамически удалить шаблон топика из активной подписки
    ///
    /// # Аргументы
    /// * `sub_id` — Идентификатор активной подписки ([`SubscriptionId`]).
    /// * `pattern` — Удаляемый шаблон топика.
    pub fn remove_topic(&self, sub_id: SubscriptionId, pattern: &str) {
        if let Some(segments) = parse_topic_segments(pattern) {
            let pat_str = pattern.trim();
            let mut guard = self.write_guard();
            if let Some(entry) = guard.subscribers.get_mut(&sub_id) {
                if entry.patterns.remove(pat_str) {
                    guard.trie.remove(&segments, sub_id);
                    if let Some(ref top) = guard.topology {
                        top.remove_subscriber_pattern(sub_id, pat_str);
                    }
                }
            }
        }
    }

    /// Полностью удалить подписчика (вызывается автоматически из [`SubscriptionHandle::drop`])
    ///
    /// # Аргументы
    /// * `sub_id` — Идентификатор удаляемой подписки.
    pub fn unsubscribe(&self, sub_id: SubscriptionId) {
        let mut guard = self.write_guard();
        if let Some(entry) = guard.subscribers.remove(&sub_id) {
            for pattern in &entry.patterns {
                if let Some(segments) = parse_topic_segments(pattern) {
                    guard.trie.remove(&segments, sub_id);
                }
            }
            if let Some(ref top) = guard.topology {
                top.unregister_subscriber(sub_id);
            }
        }
    }

    /// Отправить событие всем подходящим подписчикам по дереву топиков с применением фильтров
    ///
    /// # Аргументы
    /// * `event` — Доставляемое событие платформы ([`EventMessage`]).
    ///
    /// # Возвращаемое значение
    /// Результат доставки ([`DispatchResult`]) с числом доставленных и сброшенных сообщений.
    pub fn dispatch(&self, event: &EventMessage) -> DispatchResult {
        let segments: Vec<&str> = event.topic.split('.').filter(|s| !s.is_empty()).collect();
        let mut target_ids = HashSet::new();

        let (targets, topology_tracker) = {
            let guard = self.read_guard();
            guard.trie.match_segments(&segments, &mut target_ids);

            if target_ids.is_empty() {
                return DispatchResult::default();
            }

            let top = guard.topology.clone();
            let mut list = Vec::with_capacity(target_ids.len());
            for id in target_ids {
                if let Some(entry) = guard.subscribers.get(&id) {
                    list.push((id, entry.tx.clone(), entry.filter.clone()));
                }
            }
            (list, top)
        };

        let mut delivered = 0;
        let mut dropped = 0;

        for (id, tx, filter) in targets {
            // Проверяем предикат контентной фильтрации подписчика вне блокировки роутера
            if let Some(ref predicate) = filter {
                if !predicate(event) {
                    continue;
                }
            }

            // Пытаемся отправить без блокировки (non-blocking try_send)
            match tx.try_send(event.clone()) {
                Ok(_) => {
                    delivered += 1;
                    if let Some(ref top) = topology_tracker {
                        top.record_delivery(id);
                    }
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    dropped += 1;
                    trace!("Subscriber queue full for sub_id {}, dropped event {}", id, event.id);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    dropped += 1;
                    trace!("Subscriber queue closed for sub_id {}, counted as dropped", id);
                }
            }
        }

        DispatchResult { delivered, dropped }
    }

    /// Получить текущее количество активных зарегистрированных подписчиков
    pub fn subscriber_count(&self) -> usize {
        self.read_guard().subscribers.len()
    }
}

/// RAII-дескриптор подписки на события шины
///
/// Автоматически удаляет регистрацию топиков из маршрутизатора при выходе из области видимости (`Drop`).
pub struct SubscriptionHandle {
    id: SubscriptionId,
    name: Option<String>,
    rx: mpsc::Receiver<EventMessage>,
    router: TopicRouter,
    filter: Option<EventFilter>,
    throttle_state: HashMap<String, std::time::Instant>,
}

impl SubscriptionHandle {
    /// Получить уникальный идентификатор подписки
    pub fn id(&self) -> SubscriptionId {
        self.id
    }

    /// Получить опциональное имя подписки/плагина
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Привязать имя подписчика для идентификации в графе топологии
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        let name_str = name.into();
        self.router.set_subscriber_name(self.id, name_str.clone());
        self.name = Some(name_str);
        self
    }

    /// Асинхронно прочитать следующее событие из очереди
    ///
    /// Возвращает `None`, если шина событий была остановлена.
    pub async fn recv(&mut self) -> Option<EventMessage> {
        self.rx.recv().await
    }

    /// Прочитать следующее событие с ограничением частоты по топикам (Throttling)
    ///
    /// Пропускает события не чаще одного раза в `min_interval` для каждого уникального топика,
    /// защищая подписчика от перегрузки при высокочастотных всплесках телеметрии.
    pub async fn recv_throttled(&mut self, min_interval: std::time::Duration) -> Option<EventMessage> {
        const MAX_THROTTLE_ENTRIES: usize = 1024;

        while let Some(msg) = self.rx.recv().await {
            let now = std::time::Instant::now();

            // Защита от неограниченного роста памяти: очистка устаревших топиков
            if self.throttle_state.len() >= MAX_THROTTLE_ENTRIES {
                self.throttle_state
                    .retain(|_, last_time| now.duration_since(*last_time) < min_interval);
            }

            let should_emit = match self.throttle_state.get(&msg.topic) {
                Some(&last_time) => now.duration_since(last_time) >= min_interval,
                None => true,
            };

            if should_emit {
                self.throttle_state.insert(msg.topic.clone(), now);
                return Some(msg);
            }
        }
        None
    }

    /// Прочитать событие со сглаживанием всплесков и дребезга (Debouncing)
    ///
    /// При поступлении серии частых обновлений накапливает их и возвращает финальное
    /// установившееся значение после периода затишья `quiet_period` либо по истечении максимального времени ожидания (10 * quiet_period).
    pub async fn recv_debounced(&mut self, quiet_period: std::time::Duration) -> Option<EventMessage> {
        self.recv_debounced_max_wait(quiet_period, quiet_period.saturating_mul(10)).await
    }

    /// Прочитать событие со сглаживанием всплесков и заданным максимальным временем ожидания
    ///
    /// Гарантирует доставку промежуточного состояния не позднее `max_wait` даже при непрерывном шторме событий без пауз.
    pub async fn recv_debounced_max_wait(
        &mut self,
        quiet_period: std::time::Duration,
        max_wait: std::time::Duration,
    ) -> Option<EventMessage> {
        let mut latest_msg = self.rx.recv().await?;
        let start = tokio::time::Instant::now();

        loop {
            let max_remaining = max_wait.saturating_sub(start.elapsed());
            if max_remaining.is_zero() {
                return Some(latest_msg);
            }
            let sleep_dur = quiet_period.min(max_remaining);

            tokio::select! {
                next_opt = self.rx.recv() => {
                    match next_opt {
                        Some(next_msg) => {
                            latest_msg = next_msg;
                        }
                        None => {
                            // Канал закрылся, отдаем последнее накопленное сообщение
                            return Some(latest_msg);
                        }
                    }
                }
                _ = tokio::time::sleep(sleep_dur) => {
                    return Some(latest_msg);
                }
            }
        }
    }

    /// Асинхронно прочитать следующее событие и автоматически десериализовать его payload
    ///
    /// # Типы
    /// * `T` — Целевая структура данных, реализующая [`serde::de::DeserializeOwned`].
    pub async fn recv_typed<T: serde::de::DeserializeOwned>(
        &mut self,
    ) -> Option<std::result::Result<T, serde_json::Error>> {
        let msg = self.recv().await?;
        Some(serde_json::from_value(msg.payload))
    }

    /// Попробовать прочитать следующее событие без блокировки
    pub fn try_recv(&mut self) -> std::result::Result<EventMessage, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }

    /// Установить предикат контентной фильтрации событий
    pub fn with_filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&EventMessage) -> bool + Send + Sync + 'static,
    {
        let filter: EventFilter = Arc::new(predicate);
        self.router.set_filter(self.id, Some(filter.clone()));
        self.filter = Some(filter);
        self
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

