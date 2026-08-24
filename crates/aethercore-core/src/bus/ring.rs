//! # Горячий кольцевой буфер событий в оперативной памяти (L1 RingBuffer)
//!
//! Обеспечивает мгновенную выборку свежих событий для UI и реконнекта клиентов
//! без обращения к дисковому вводу-выводу.

use aethercore_common::models::events::EventMessage;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

/// Емкость кольцевого буфера по умолчанию
pub const DEFAULT_RING_CAPACITY: usize = 2048;

/// Потокобезопасный кольцевой буфер оперативной памяти L1
#[derive(Clone, Debug)]
pub struct EventRingBuffer {
    capacity: usize,
    inner: Arc<RwLock<VecDeque<EventMessage>>>,
}

impl EventRingBuffer {
    /// Создать новый кольцевой буфер заданной емкости
    ///
    /// # Аргументы
    /// * `capacity` — Максимальное количество удерживаемых событий (минимум 16).
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(16),
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
        }
    }

    fn read_guard(&self) -> std::sync::RwLockReadGuard<'_, VecDeque<EventMessage>> {
        match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn write_guard(&self) -> std::sync::RwLockWriteGuard<'_, VecDeque<EventMessage>> {
        match self.inner.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    /// Добавить событие в кольцевой буфер.
    ///
    /// Если буфер заполнен, самое старое событие вытесняется и возвращается (`Some(ev)`),
    /// что позволяет направить его в L2 хранилище (Spillover).
    ///
    /// # Аргументы
    /// * `event` — Добавляемое событие платформы.
    ///
    /// # Возвращаемое значение
    /// Вытесненное старое событие (`Some(EventMessage)`), если буфер был заполнен.
    pub fn push(&self, event: EventMessage) -> Option<EventMessage> {
        let mut guard = self.write_guard();
        let evicted = if guard.len() >= self.capacity {
            guard.pop_front()
        } else {
            None
        };
        guard.push_back(event);
        evicted
    }

    /// Запросить непросроченные события из буфера памяти с опциональным префиксным фильтром
    ///
    /// # Аргументы
    /// * `topic_filter` — Опциональный строковый префикс темы (например, `"devices."`).
    /// * `limit` — Максимальное количество возвращаемых событий.
    ///
    /// # Возвращаемое значение
    /// Список событий в хронологическом порядке (от старых к новым).
    pub fn query(&self, topic_filter: Option<&str>, limit: usize) -> Vec<EventMessage> {
        let guard = self.read_guard();
        let limit = limit.max(1);

        let mut results = Vec::new();
        // Итерируемся с конца (от самых свежих событий)
        for ev in guard.iter().rev() {
            if results.len() >= limit {
                break;
            }

            // Отсекаем протухшие по TTL события
            if ev.is_expired() {
                continue;
            }

            if let Some(prefix) = topic_filter {
                if !prefix.is_empty() && !ev.topic.starts_with(prefix) {
                    continue;
                }
            }

            results.push(ev.clone());
        }

        // Возвращаем в хронологическом порядке (по возрастанию времени)
        results.reverse();
        results
    }

    /// Текущее количество элементов в буфере
    pub fn len(&self) -> usize {
        self.read_guard().len()
    }

    /// Проверить, пуст ли буфер
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Очистить буфер памяти
    pub fn clear(&self) {
        self.write_guard().clear();
    }
}

