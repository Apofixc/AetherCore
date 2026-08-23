//! # Дедупликация входящих событий по UUID (In-Memory Sliding Window)
//!
//! Защищает от дублирования сетевых пакетов, повторов брокеров и плагинов.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Емкость скользящего окна дедупликации по умолчанию
const DEFAULT_DEDUP_CAPACITY: usize = 10_000;
/// Время жизни записи в окне дедупликации по умолчанию
const DEFAULT_DEDUP_TTL: Duration = Duration::from_secs(60);

#[derive(Debug)]
struct DedupInner {
    records: HashMap<Uuid, Instant>,
    queue: VecDeque<(Uuid, Instant)>,
    capacity: usize,
    ttl: Duration,
}

impl DedupInner {
    fn clean_expired(&mut self, now: Instant) {
        while let Some((_, ts)) = self.queue.front() {
            if now.duration_since(*ts) > self.ttl {
                if let Some((id, _)) = self.queue.pop_front() {
                    self.records.remove(&id);
                }
            } else {
                break;
            }
        }
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
    /// * `capacity` — Максимальное количество удерживаемых идентификаторов UUID.
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

    /// Проверить, является ли событие дубликатом, и зафиксировать его, если оно новое
    ///
    /// # Аргументы
    /// * `id` — Уникальный идентификатор события ([`Uuid`]).
    ///
    /// # Возвращаемое значение
    /// Возвращает `true`, если событие УЖЕ было обработано ранее (дубликат),
    /// или `false`, если это новое сообщение (зафиксировано в дедупликаторе).
    pub fn is_duplicate_or_record(&self, id: &Uuid) -> bool {
        let now = Instant::now();
        let mut guard = self.inner.write().unwrap();

        guard.clean_expired(now);

        if guard.records.contains_key(id) {
            return true;
        }

        // Если емкость исчерпана — вытесняем самый старый элемент
        if guard.queue.len() >= guard.capacity {
            if let Some((old_id, _)) = guard.queue.pop_front() {
                guard.records.remove(&old_id);
            }
        }

        guard.records.insert(*id, now);
        guard.queue.push_back((*id, now));
        false
    }
}
