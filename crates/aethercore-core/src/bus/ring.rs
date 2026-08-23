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
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(16),
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(capacity))),
        }
    }

    /// Добавить событие в кольцевой буфер.
    ///
    /// Если буфер заполнен, самое старое событие вытесняется и возвращается (`Some(ev)`),
    /// что позволяет направить его в L2 хранилище (Spillover).
    pub fn push(&self, event: EventMessage) -> Option<EventMessage> {
        let mut guard = self.inner.write().unwrap();
        let evicted = if guard.len() >= self.capacity {
            guard.pop_front()
        } else {
            None
        };
        guard.push_back(event);
        evicted
    }

    /// Запросить непросроченные события из буфера памяти с опциональным префиксным фильтром
    pub fn query(&self, topic_filter: Option<&str>, limit: usize) -> Vec<EventMessage> {
        let guard = self.inner.read().unwrap();
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
        self.inner.read().unwrap().len()
    }

    /// Проверить, пуст ли буфер
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Очистить буфер памяти
    pub fn clear(&self) {
        self.inner.write().unwrap().clear();
    }
}
