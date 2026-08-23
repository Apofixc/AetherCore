//! # Диспетчер взвешенных очередей приоритетов (Weighted Fair Queuing)
//!
//! Обеспечивает приоритетную доставку сообщений с гарантированной защитой от голодания
//! (Starvation Prevention) низкоприоритетных потоков.

use aethercore_common::error::{AppError, Result};
use aethercore_common::models::events::{EventMessage, EventPriority};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Вместимость очередей приоритетов
const QUEUE_CAPACITY: usize = 2048;

/// Весовые квоты обработки очередей за один раунд диспетчера
const CRITICAL_WEIGHT: usize = 8;
const HIGH_WEIGHT: usize = 4;
const NORMAL_WEIGHT: usize = 2;
const LOW_WEIGHT: usize = 1;

/// Взвешенный диспетчер очередей приоритетов
#[derive(Debug)]
pub struct PriorityQueueSender {
    critical_tx: mpsc::Sender<EventMessage>,
    high_tx: mpsc::Sender<EventMessage>,
    normal_tx: mpsc::Sender<EventMessage>,
    low_tx: mpsc::Sender<EventMessage>,
    queue_len: Arc<AtomicUsize>,
}

impl Clone for PriorityQueueSender {
    fn clone(&self) -> Self {
        Self {
            critical_tx: self.critical_tx.clone(),
            high_tx: self.high_tx.clone(),
            normal_tx: self.normal_tx.clone(),
            low_tx: self.low_tx.clone(),
            queue_len: self.queue_len.clone(),
        }
    }
}

impl PriorityQueueSender {
    /// Поместить сообщение в соответствующую очередь приоритета
    ///
    /// # Аргументы
    /// * `event` — Событие платформы с указанным [`EventPriority`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::internal`], если канал соответствующего приоритета закрыт.
    pub async fn enqueue(&self, event: EventMessage) -> Result<()> {
        let sender = match event.priority {
            EventPriority::Critical => &self.critical_tx,
            EventPriority::High => &self.high_tx,
            EventPriority::Normal => &self.normal_tx,
            EventPriority::Low => &self.low_tx,
        };

        sender.send(event).await.map_err(|e| {
            AppError::internal(format!("Priority queue channel closed: {}", e))
        })?;

        self.queue_len.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Текущая суммарная длина очередей по всем уровням приоритета
    pub fn len(&self) -> usize {
        self.queue_len.load(Ordering::Relaxed)
    }

    /// Проверить, пуста ли очередь диспетчера
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Приемная часть взвешенного диспетчера
#[derive(Debug)]
pub struct PriorityQueueReceiver {
    critical_rx: mpsc::Receiver<EventMessage>,
    high_rx: mpsc::Receiver<EventMessage>,
    normal_rx: mpsc::Receiver<EventMessage>,
    low_rx: mpsc::Receiver<EventMessage>,
    queue_len: Arc<AtomicUsize>,
    critical_quota: usize,
    high_quota: usize,
    normal_quota: usize,
    low_quota: usize,
}

impl PriorityQueueReceiver {
    /// Извлечь следующее событие согласно алгоритму Weighted Fair Queuing (WFQ)
    ///
    /// Гарантирует выборку по квотам 8 Critical : 4 High : 2 Normal : 1 Low за раунд.
    pub async fn dequeue(&mut self) -> Option<EventMessage> {
        // Если все квоты раунда исчерпаны — начинаем новый раунд
        if self.critical_quota == 0
            && self.high_quota == 0
            && self.normal_quota == 0
            && self.low_quota == 0
        {
            self.critical_quota = CRITICAL_WEIGHT;
            self.high_quota = HIGH_WEIGHT;
            self.normal_quota = NORMAL_WEIGHT;
            self.low_quota = LOW_WEIGHT;
        }

        // 1. Critical
        if self.critical_quota > 0 {
            match self.critical_rx.try_recv() {
                Ok(msg) => {
                    self.critical_quota -= 1;
                    self.queue_len.fetch_sub(1, Ordering::Relaxed);
                    return Some(msg);
                }
                Err(_) => {
                    self.critical_quota = 0; // Очередь пуста, переходим к следующему уровню
                }
            }
        }

        // 2. High
        if self.high_quota > 0 {
            match self.high_rx.try_recv() {
                Ok(msg) => {
                    self.high_quota -= 1;
                    self.queue_len.fetch_sub(1, Ordering::Relaxed);
                    return Some(msg);
                }
                Err(_) => {
                    self.high_quota = 0;
                }
            }
        }

        // 3. Normal
        if self.normal_quota > 0 {
            match self.normal_rx.try_recv() {
                Ok(msg) => {
                    self.normal_quota -= 1;
                    self.queue_len.fetch_sub(1, Ordering::Relaxed);
                    return Some(msg);
                }
                Err(_) => {
                    self.normal_quota = 0;
                }
            }
        }

        // 4. Low
        if self.low_quota > 0 {
            match self.low_rx.try_recv() {
                Ok(msg) => {
                    self.low_quota -= 1;
                    self.queue_len.fetch_sub(1, Ordering::Relaxed);
                    return Some(msg);
                }
                Err(_) => {
                    self.low_quota = 0;
                }
            }
        }

        // Если за круг ни одной очереди не удалось вычитать — сбрасываем квоты и ждем в select
        self.critical_quota = CRITICAL_WEIGHT;
        self.high_quota = HIGH_WEIGHT;
        self.normal_quota = NORMAL_WEIGHT;
        self.low_quota = LOW_WEIGHT;

        tokio::select! {
            Some(msg) = self.critical_rx.recv() => {
                self.critical_quota = self.critical_quota.saturating_sub(1);
                self.queue_len.fetch_sub(1, Ordering::Relaxed);
                Some(msg)
            }
            Some(msg) = self.high_rx.recv() => {
                self.high_quota = self.high_quota.saturating_sub(1);
                self.queue_len.fetch_sub(1, Ordering::Relaxed);
                Some(msg)
            }
            Some(msg) = self.normal_rx.recv() => {
                self.normal_quota = self.normal_quota.saturating_sub(1);
                self.queue_len.fetch_sub(1, Ordering::Relaxed);
                Some(msg)
            }
            Some(msg) = self.low_rx.recv() => {
                self.low_quota = self.low_quota.saturating_sub(1);
                self.queue_len.fetch_sub(1, Ordering::Relaxed);
                Some(msg)
            }
            else => None,
        }
    }
}

/// Создать пару очередей взвешенного диспетчера приоритетов
pub fn create_priority_queue() -> (PriorityQueueSender, PriorityQueueReceiver) {
    let (critical_tx, critical_rx) = mpsc::channel(QUEUE_CAPACITY);
    let (high_tx, high_rx) = mpsc::channel(QUEUE_CAPACITY);
    let (normal_tx, normal_rx) = mpsc::channel(QUEUE_CAPACITY);
    let (low_tx, low_rx) = mpsc::channel(QUEUE_CAPACITY);

    let queue_len = Arc::new(AtomicUsize::new(0));

    let sender = PriorityQueueSender {
        critical_tx,
        high_tx,
        normal_tx,
        low_tx,
        queue_len: queue_len.clone(),
    };

    let receiver = PriorityQueueReceiver {
        critical_rx,
        high_rx,
        normal_rx,
        low_rx,
        queue_len,
        critical_quota: CRITICAL_WEIGHT,
        high_quota: HIGH_WEIGHT,
        normal_quota: NORMAL_WEIGHT,
        low_quota: LOW_WEIGHT,
    };

    (sender, receiver)
}
