//! # Метрики и статистика шины событий (Bus Observability)

use aethercore_common::models::events::EventPriority;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Снимок текущего состояния и метрик производительности шины событий
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BusStats {
    /// Общее количество опубликованных событий
    pub published_total: u64,
    /// Количество критических событий (Critical)
    pub critical_total: u64,
    /// Количество событий высокого приоритета (High)
    pub high_total: u64,
    /// Количество обычных событий (Normal)
    pub normal_total: u64,
    /// Количество низкоприоритетных событий (Low)
    pub low_total: u64,
    /// Количество активных подписчиков
    pub active_subscribers: usize,
    /// Текущий размер горячего L1 кольцевого буфера
    pub ring_buffer_len: usize,
    /// Количество сброшенных/пропущенных сообщений из-за переполнения очередей
    pub dropped_total: u64,
}

/// Внутренние атомарные счетчики метрик
#[derive(Default, Debug)]
pub struct MetricsCollector {
    published_total: AtomicU64,
    critical_total: AtomicU64,
    high_total: AtomicU64,
    normal_total: AtomicU64,
    low_total: AtomicU64,
    dropped_total: AtomicU64,
}

impl MetricsCollector {
    /// Учесть опубликованное событие с заданным приоритетом
    pub fn record_published(&self, priority: EventPriority) {
        self.published_total.fetch_add(1, Ordering::Relaxed);
        match priority {
            EventPriority::Critical => self.critical_total.fetch_add(1, Ordering::Relaxed),
            EventPriority::High => self.high_total.fetch_add(1, Ordering::Relaxed),
            EventPriority::Normal => self.normal_total.fetch_add(1, Ordering::Relaxed),
            EventPriority::Low => self.low_total.fetch_add(1, Ordering::Relaxed),
        };
    }

    /// Учесть сброшенное сообщение из-за переполнения очередей
    pub fn record_dropped(&self) {
        self.dropped_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Сформировать снимок текущей статистики
    pub fn snapshot(&self, active_subscribers: usize, ring_len: usize) -> BusStats {
        BusStats {
            published_total: self.published_total.load(Ordering::Relaxed),
            critical_total: self.critical_total.load(Ordering::Relaxed),
            high_total: self.high_total.load(Ordering::Relaxed),
            normal_total: self.normal_total.load(Ordering::Relaxed),
            low_total: self.low_total.load(Ordering::Relaxed),
            active_subscribers,
            ring_buffer_len: ring_len,
            dropped_total: self.dropped_total.load(Ordering::Relaxed),
        }
    }
}

/// Потокобезопасная ссылка на сборщик метрик
#[derive(Clone, Default, Debug)]
pub struct BusMetrics(pub Arc<MetricsCollector>);
