//! # Дедупликация входящих событий по UUID и бизнес-ключам (In-Memory Sliding Window)
//!
//! Защищает от дублирования сетевых пакетов, повторов брокеров, плагинов и штормов алармов.

use aethercore_common::models::events::EventMessage;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Емкость скользящего окна дедупликации по умолчанию
const DEFAULT_DEDUP_CAPACITY: usize = 10_000;
/// Время жизни записи в окне дедупликации по умолчанию
const DEFAULT_DEDUP_TTL: Duration = Duration::from_secs(60);

/// Ключ дедупликации (UUID сообщения или пользовательский бизнес-ключ)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum DedupKey {
    Uuid(Uuid),
    Custom(String),
}

#[derive(Debug)]
struct DedupInner {
    records: HashMap<DedupKey, Instant>,
    queue: VecDeque<(DedupKey, Instant)>,
    capacity: usize,
    ttl: Duration,
}

impl DedupInner {
    fn clean_expired(&mut self, now: Instant) {
        while let Some((_, ts)) = self.queue.front() {
            if now.duration_since(*ts) > self.ttl {
                if let Some((key, _)) = self.queue.pop_front() {
                    self.records.remove(&key);
                }
            } else {
                break;
            }
        }
    }

    fn check_and_insert(&mut self, key: DedupKey, now: Instant) -> bool {
        if self.records.contains_key(&key) {
            return true;
        }

        // Если емкость исчерпана — вытесняем самый старый элемент
        if self.queue.len() >= self.capacity {
            if let Some((old_key, _)) = self.queue.pop_front() {
                self.records.remove(&old_key);
            }
        }

        self.records.insert(key.clone(), now);
        self.queue.push_back((key, now));
        false
    }
}

/// Потокобезопасный дедупликатор событий
#[derive(Clone, Debug)]
pub struct EventDeduplicator {
    inner: Arc<RwLock<DedupInner>>,
}

impl Default for EventDeduplicator {
    fn default() -> Self {
        Self::new(DEFAULT_DEDUP_CAPACITY, DEFAULT_DEDUP_TTL)
    }
}

impl EventDeduplicator {
    /// Создать дедупликатор с заданной емкостью и временем жизни записи
    ///
    /// # Аргументы
    /// * `capacity` — Максимальное количество удерживаемых идентификаторов.
    /// * `ttl` — Время жизни записи в окне дедупликации ([`Duration`]).
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(DedupInner {
                records: HashMap::with_capacity(capacity.min(1000)),
                queue: VecDeque::with_capacity(capacity.min(1000)),
                capacity: capacity.max(100),
                ttl,
            })),
        }
    }

    /// Проверить, является ли событие дубликатом по business key или UUID, и зафиксировать его
    pub fn is_duplicate_event_or_record(&self, event: &EventMessage) -> bool {
        let now = Instant::now();
        let mut guard = self.inner.write().unwrap();
        guard.clean_expired(now);

        // 1. Проверяем бизнес-ключ дедупликации (если задан)
        if let Some(ref custom_key) = event.dedup_key {
            if guard.check_and_insert(DedupKey::Custom(custom_key.clone()), now) {
                return true;
            }
        }

        // 2. Проверяем UUID события
        guard.check_and_insert(DedupKey::Uuid(event.id), now)
    }

    /// Проверить, является ли событие дубликатом по UUID, и зафиксировать его
    pub fn is_duplicate_or_record(&self, id: &Uuid) -> bool {
        let now = Instant::now();
        let mut guard = self.inner.write().unwrap();
        guard.clean_expired(now);
        guard.check_and_insert(DedupKey::Uuid(*id), now)
    }

    /// Текущее количество активных записей в дедупликаторе
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().records.len()
    }

    /// Проверить, пуст ли дедупликатор
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
