//! # Кэш последних значений топиков (Retained Messages Store)
//!
//! Обеспечивает безопасное хранение последних актуальных состояний топиков с защитой
//! от переполнения памяти (LRU-вытеснение) и проверкой срока актуальности (TTL).

use aethercore_common::models::events::EventMessage;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use tracing::trace;

/// Лимит сохраненных retained-сообщений по умолчанию
pub const DEFAULT_RETAINED_CAPACITY: usize = 500;
/// Максимальное количество сообщений, отдаваемых при одной подписке
pub const MAX_INITIAL_RETAINED_LIMIT: usize = 100;

#[derive(Debug)]
struct RetainedInner {
    records: HashMap<String, EventMessage>,
    lru_queue: VecDeque<String>,
    capacity: usize,
}

impl RetainedInner {
    fn new(capacity: usize) -> Self {
        Self {
            records: HashMap::with_capacity(capacity.min(256)),
            lru_queue: VecDeque::with_capacity(capacity.min(256)),
            capacity: capacity.max(1),
        }
    }

    fn put(&mut self, event: EventMessage) {
        let topic = event.topic.clone();

        // Если событие уже просрочено по TTL — удаляем из кэша
        if event.is_expired() {
            self.remove(&topic);
            return;
        }

        if self.records.contains_key(&topic) {
            // Перемещаем топик в хвост очереди LRU
            self.lru_queue.retain(|t| t != &topic);
            self.lru_queue.push_back(topic.clone());
        } else {
            // Если емкость исчерпана — вытесняем самый старый топик
            while self.records.len() >= self.capacity {
                if let Some(old_topic) = self.lru_queue.pop_front() {
                    self.records.remove(&old_topic);
                } else {
                    break;
                }
            }
            self.lru_queue.push_back(topic.clone());
        }

        self.records.insert(topic, event);
    }

    fn remove(&mut self, topic: &str) -> Option<EventMessage> {
        self.lru_queue.retain(|t| t != topic);
        self.records.remove(topic)
    }

    fn get_matching(&self, pattern: &str, limit: usize) -> Vec<EventMessage> {
        if limit == 0 {
            return Vec::new();
        }
        let limit = limit.min(MAX_INITIAL_RETAINED_LIMIT);
        let mut results = Vec::new();

        // 1. Точное совпадение
        if !pattern.contains('*') && !pattern.contains('#') {
            if let Some(ev) = self.records.get(pattern) {
                if !ev.is_expired() {
                    results.push(ev.clone());
                }
            }
            return results;
        }

        // 2. Сопоставление по маскам топика
        let pattern_segments: Vec<&str> = pattern.split('.').collect();

        for (topic, ev) in &self.records {
            if results.len() >= limit {
                break;
            }

            if ev.is_expired() {
                continue;
            }

            let topic_segments: Vec<&str> = topic.split('.').collect();
            if match_topic_segments(&pattern_segments, &topic_segments) {
                results.push(ev.clone());
            }
        }

        results
    }
}

/// Сопоставление сегментов топика с поддержкой wildcards `*` и `#`
fn match_topic_segments(pattern: &[&str], topic: &[&str]) -> bool {
    let mut p_idx = 0;
    let mut t_idx = 0;

    while p_idx < pattern.len() && t_idx < topic.len() {
        let p = pattern[p_idx];
        if p == "#" {
            // '#' в конце сопоставляет все оставшиеся сегменты
            return true;
        }

        if p == "*" || p == topic[t_idx] {
            p_idx += 1;
            t_idx += 1;
        } else {
            return false;
        }
    }

    if p_idx < pattern.len() && pattern[p_idx] == "#" {
        return true;
    }

    p_idx == pattern.len() && t_idx == topic.len()
}

/// Потокобезопасное хранилище последних значений топиков
#[derive(Clone, Debug)]
pub struct RetainedStore {
    inner: Arc<RwLock<RetainedInner>>,
}

impl Default for RetainedStore {
    fn default() -> Self {
        Self::new(DEFAULT_RETAINED_CAPACITY)
    }
}

impl RetainedStore {
    /// Создать хранилище с заданной емкостью
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(RetainedInner::new(capacity))),
        }
    }

    fn read_guard(&self) -> std::sync::RwLockReadGuard<'_, RetainedInner> {
        match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn write_guard(&self) -> std::sync::RwLockWriteGuard<'_, RetainedInner> {
        match self.inner.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    /// Сохранить последнее актуальное состояние топика
    pub fn put(&self, event: EventMessage) {
        trace!("Retaining message for topic '{}'", event.topic);
        self.write_guard().put(event);
    }

    /// Удалить сохраненное состояние для топика
    pub fn remove(&self, topic: &str) -> Option<EventMessage> {
        self.write_guard().remove(topic)
    }

    /// Запросить сохраненные сообщения, подходящие под указанный топик или маску
    pub fn get_matching(&self, pattern: &str, limit: usize) -> Vec<EventMessage> {
        self.read_guard().get_matching(pattern, limit)
    }

    /// Текущее количество сохраненных состояний топиков
    pub fn len(&self) -> usize {
        self.read_guard().records.len()
    }

    /// Проверить, пусто ли хранилище
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Очистить кэш
    pub fn clear(&self) {
        let mut guard = self.write_guard();
        guard.records.clear();
        guard.lru_queue.clear();
    }
}

