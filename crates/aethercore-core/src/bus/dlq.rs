//! # Очередь недоставленных и сбойных сообщений (Dead Letter Queue — DLQ)
//!
//! Фиксирует события, обработка или доставка которых завершилась сбоем:
//! - Таймауты RPC-запросов между модулями и плагинами (`RpcTimeout`).
//! - Отсутствие активных подписчиков на целевые события (`NoSubscribers`).
//! - Истечение срока жизни сообщения (`Expired`).
//! - Отклонение конвейером перехватчиков (`Rejected`).
//!
//! Предоставляет API для инспекции причин сбоев и повторной отправки (`re-drive`) сообщений в шину.

use aethercore_common::models::events::EventMessage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Емкость кольцевого буфера очереди недоставленных сообщений по умолчанию
pub const DEFAULT_DLQ_CAPACITY: usize = 512;

/// Причина попадания сообщения в очередь недоставленных (DLQ)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "details")]
pub enum DeadLetterReason {
    /// Истек таймаут ожидания ответа на RPC-запрос между модулями
    RpcTimeout,
    /// Сообщение отправлено, но целевых подписчиков в шине не обнаружено
    NoSubscribers,
    /// Истекло время актуальности события (TTL)
    Expired,
    /// Сообщение отклонено конвейером перехватчиков или политикой безопасности
    Rejected(String),
}

/// Запись сбойного события в очереди недоставленных сообщений
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeadLetter {
    /// Уникальный идентификатор записи в DLQ
    pub id: Uuid,
    /// Исходное сообщение события
    pub event: EventMessage,
    /// Причина фиксации сбоя
    pub reason: DeadLetterReason,
    /// Время попадания в DLQ (UTC)
    pub failed_at: DateTime<Utc>,
}

/// Потокобезопасная очередь недоставленных сообщений
#[derive(Debug, Clone)]
pub struct DeadLetterQueue {
    capacity: usize,
    inner: Arc<RwLock<VecDeque<DeadLetter>>>,
}

impl Default for DeadLetterQueue {
    fn default() -> Self {
        Self::new(DEFAULT_DLQ_CAPACITY)
    }
}

impl DeadLetterQueue {
    /// Создать новую очередь DLQ с заданной емкостью
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            inner: Arc::new(RwLock::new(VecDeque::with_capacity(capacity.min(1024)))),
        }
    }

    fn read_guard(&self) -> std::sync::RwLockReadGuard<'_, VecDeque<DeadLetter>> {
        match self.inner.read() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn write_guard(&self) -> std::sync::RwLockWriteGuard<'_, VecDeque<DeadLetter>> {
        match self.inner.write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    /// Добавить сбойное сообщение в DLQ
    pub fn push(&self, event: EventMessage, reason: DeadLetterReason) -> Uuid {
        let id = Uuid::new_v4();
        let entry = DeadLetter {
            id,
            event,
            reason,
            failed_at: Utc::now(),
        };

        let mut deque = self.write_guard();
        if deque.len() >= self.capacity {
            deque.pop_front();
        }
        deque.push_back(entry);
        id
    }

    /// Получить список последних сбойных сообщений с ограничением количества
    pub fn list(&self, limit: usize) -> Vec<DeadLetter> {
        let deque = self.read_guard();
        deque.iter().rev().take(limit).cloned().collect()
    }

    /// Найти сбойную запись по ее уникальному идентификатору
    pub fn get(&self, id: Uuid) -> Option<DeadLetter> {
        let deque = self.read_guard();
        deque.iter().find(|item| item.id == id).cloned()
    }

    /// Извлечь и удалить запись из DLQ по идентификатору (для повторной отправки re-drive)
    pub fn remove(&self, id: Uuid) -> Option<DeadLetter> {
        let mut deque = self.write_guard();
        if let Some(pos) = deque.iter().position(|item| item.id == id) {
            return deque.remove(pos);
        }
        None
    }

    /// Очистить все записи в очереди DLQ
    pub fn clear(&self) {
        let mut deque = self.write_guard();
        deque.clear();
    }

    /// Текущее количество сбойных сообщений в очереди
    pub fn len(&self) -> usize {
        self.read_guard().len()
    }

    /// Проверить, пуста ли очередь
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
