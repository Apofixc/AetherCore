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

    fn insert_key(&mut self, key: DedupKey, now: Instant) {
        // Если емкость исчерпана — вытесняем самый старый элемент
        if self.queue.len() >= self.capacity {
            if let Some((old_key, _)) = self.queue.pop_front() {
                self.records.remove(&old_key);
            }
        }

        self.records.insert(key.clone(), now);
        self.queue.push_back((key, now));
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

    fn read_guard(&self) -> std::sync::RwLockReadGuard<'_, DedupInner> {
        match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn write_guard(&self) -> std::sync::RwLockWriteGuard<'_, DedupInner> {
        match self.inner.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    /// Проверить, является ли событие дубликатом, без фиксации ключей
    pub fn is_duplicate(&self, event: &EventMessage) -> bool {
        let now = Instant::now();
        let mut guard = self.write_guard();
        guard.clean_expired(now);

        let custom_dup = event
            .dedup_key
            .as_ref()
            .map_or(false, |k| guard.records.contains_key(&DedupKey::Custom(k.clone())));
        let uuid_dup = guard.records.contains_key(&DedupKey::Uuid(event.id));

        custom_dup || uuid_dup
    }

    /// Зафиксировать ключи события в окне дедупликатора
    pub fn record(&self, event: &EventMessage) {
        let now = Instant::now();
        let mut guard = self.write_guard();
        guard.clean_expired(now);

        if let Some(ref custom_key) = event.dedup_key {
            guard.insert_key(DedupKey::Custom(custom_key.clone()), now);
        }
        guard.insert_key(DedupKey::Uuid(event.id), now);
    }

    /// Удалить ключи события из дедупликатора (откат при ошибке публикации)
    pub fn remove_event(&self, event: &EventMessage) {
        let mut guard = self.write_guard();
        if let Some(ref custom_key) = event.dedup_key {
            let key = DedupKey::Custom(custom_key.clone());
            guard.records.remove(&key);
            guard.queue.retain(|(k, _)| k != &key);
        }
        let uuid_key = DedupKey::Uuid(event.id);
        guard.records.remove(&uuid_key);
        guard.queue.retain(|(k, _)| k != &uuid_key);
    }

    /// Проверить, является ли событие дубликатом по business key или UUID, и зафиксировать его
    pub fn is_duplicate_event_or_record(&self, event: &EventMessage) -> bool {
        let now = Instant::now();
        let mut guard = self.write_guard();
        guard.clean_expired(now);

        // Фаза 1: Проверка на дубликат без модификации состояния
        let custom_dup = event
            .dedup_key
            .as_ref()
            .map_or(false, |k| guard.records.contains_key(&DedupKey::Custom(k.clone())));
        let uuid_dup = guard.records.contains_key(&DedupKey::Uuid(event.id));

        if custom_dup || uuid_dup {
            return true;
        }

        // Фаза 2: Фиксация новых ключей в скользящем окне
        if let Some(ref custom_key) = event.dedup_key {
            guard.insert_key(DedupKey::Custom(custom_key.clone()), now);
        }
        guard.insert_key(DedupKey::Uuid(event.id), now);

        false
    }

    /// Проверить, является ли событие дубликатом по UUID, и зафиксировать его
    pub fn is_duplicate_or_record(&self, id: &Uuid) -> bool {
        let now = Instant::now();
        let mut guard = self.write_guard();
        guard.clean_expired(now);

        let key = DedupKey::Uuid(*id);
        if guard.records.contains_key(&key) {
            return true;
        }

        guard.insert_key(key, now);
        false
    }

    /// Текущее количество активных записей в дедупликаторе
    pub fn len(&self) -> usize {
        self.read_guard().records.len()
    }

    /// Проверить, пуст ли дедупликатор
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
