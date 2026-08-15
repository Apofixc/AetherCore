//! # Гибридная шина событий платформы (Event Bus & Reliable Event Journal)
//!
//! Включает два контура доставки сообщений:
//! 1. **Live Telemetry Bus**: высокочастотный in-memory pub/sub (`tokio::sync::broadcast`) для метрик и вебсокетов.
//! 2. **Reliable Event Journal**: гарантированная доставка через очередь `tokio::sync::mpsc` с персистентной записью в SQLite WAL.

use crate::db::Db;
use chrono::{DateTime, Utc};
use nms_common::error::{AppError, Result};
use nms_common::models::events::{EventMessage, EventType, ReliableEventRecord};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info};

/// Размер буфера кольцевой очереди broadcast
const BROADCAST_CAPACITY: usize = 1024;
/// Размер буфера персистентной очереди MPSC
const JOURNAL_QUEUE_CAPACITY: usize = 4096;

/// Экземпляр гибридной шины событий
#[derive(Debug, Clone)]
pub struct EventBus {
    /// Broadcast sender для передачи событий в реальном времени
    broadcast_tx: broadcast::Sender<EventMessage>,
    /// MPSC sender для постановки событий в персистентную очередь журнала
    journal_tx: mpsc::Sender<EventMessage>,
    /// Ссылка на базу данных для выборки журнала
    db: Db,
}

impl EventBus {
    /// Инициализировать шину событий и запустить фоновый воркер журнала
    pub fn new(db: Db) -> Self {
        let (broadcast_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let (journal_tx, mut journal_rx) = mpsc::channel(JOURNAL_QUEUE_CAPACITY);

        let bus = Self {
            broadcast_tx: broadcast_tx.clone(),
            journal_tx,
            db: db.clone(),
        };

        // Фоновый таск для гарантированной записи системных событий в SQLite WAL
        let worker_db = db.clone();
        tokio::spawn(async move {
            debug!("Event Journal background worker started");
            while let Some(event) = journal_rx.recv().await {
                if let Err(e) = record_event_to_db(&worker_db, &event).await {
                    error!(
                        "Failed to persist event '{}' (id: {}) to journal: {}",
                        event.topic, event.id, e
                    );
                }
            }
            info!("Event Journal worker terminated");
        });

        bus
    }

    /// Опубликовать событие в шину
    pub async fn publish(&self, event: EventMessage) -> Result<()> {
        // 1. Отправляем в Live Broadcast канал (если есть подписчики)
        let _ = self.broadcast_tx.send(event.clone());

        // 2. Если событие типа Reliable — ставим в очередь персистентной записи
        if event.event_type == EventType::Reliable {
            self.journal_tx
                .send(event)
                .await
                .map_err(|e| AppError::Internal {
                    details: format!("Failed to enqueue event for persistent journal: {}", e),
                })?;
        }

        Ok(())
    }

    /// Подписаться на поток событий в реальном времени
    pub fn subscribe(&self) -> broadcast::Receiver<EventMessage> {
        self.broadcast_tx.subscribe()
    }

    /// Запросить исторические события из журнала с фильтрацией
    pub async fn query_journal(
        &self,
        topic_filter: Option<&str>,
        after_id: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ReliableEventRecord>> {
        let limit = limit.min(1000).max(1) as i64;
        let after_id = after_id.unwrap_or(0);

        let query_sql = match topic_filter {
            Some(prefix) if !prefix.is_empty() => {
                let pattern = format!("{}%", prefix);
                sqlx::query_as::<_, (i64, String, String, String, String, String)>(
                    r#"
                    SELECT id, event_uuid, topic, source, payload_json, created_at
                    FROM event_journal
                    WHERE id > ? AND topic LIKE ?
                    ORDER BY id ASC
                    LIMIT ?
                    "#,
                )
                .bind(after_id)
                .bind(pattern)
                .bind(limit)
                .fetch_all(self.db.reader())
                .await
            }
            _ => {
                sqlx::query_as::<_, (i64, String, String, String, String, String)>(
                    r#"
                    SELECT id, event_uuid, topic, source, payload_json, created_at
                    FROM event_journal
                    WHERE id > ?
                    ORDER BY id ASC
                    LIMIT ?
                    "#,
                )
                .bind(after_id)
                .bind(limit)
                .fetch_all(self.db.reader())
                .await
            }
        };

        let rows = query_sql.map_err(|e| AppError::Database {
            details: format!("Failed to query event journal: {}", e),
        })?;

        let mut records = Vec::with_capacity(rows.len());
        for (id, uuid_str, topic, source, payload_json, created_at_str) in rows {
            let event_uuid = uuid::Uuid::parse_str(&uuid_str).unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            records.push(ReliableEventRecord {
                id,
                event_uuid,
                topic,
                source,
                payload_json,
                created_at,
            });
        }

        Ok(records)
    }
}

/// Запись события в таблицу SQLite
async fn record_event_to_db(db: &Db, event: &EventMessage) -> Result<()> {
    let payload_str = serde_json::to_string(&event.payload).map_err(|e| AppError::Validation {
        field: "payload".into(),
        details: e.to_string(),
    })?;

    sqlx::query(
        r#"
        INSERT INTO event_journal (event_uuid, topic, source, payload_json, created_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(event.id.to_string())
    .bind(&event.topic)
    .bind(&event.source)
    .bind(payload_str)
    .bind(event.timestamp.to_rfc3339())
    .execute(db.writer())
    .await
    .map_err(|e| AppError::Database {
        details: e.to_string(),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_live_and_reliable() {
        let db = Db::init_in_memory().await.unwrap();
        let bus = EventBus::new(db);

        let mut rx = bus.subscribe();

        // 1. Отправляем Live Telemetry событие
        let live_ev = EventMessage::telemetry("ping.tick", "core", serde_json::json!({"ms": 12}));
        bus.publish(live_ev.clone()).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.topic, "ping.tick");

        // 2. Отправляем Reliable системное событие
        let rel_ev = EventMessage::reliable(
            "user.created",
            "core",
            serde_json::json!({"username": "admin"}),
        );
        bus.publish(rel_ev.clone()).await.unwrap();

        let received_rel = rx.recv().await.unwrap();
        assert_eq!(received_rel.topic, "user.created");

        // Даем микропаузу воркеру на запись в БД
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // 3. Читаем из персистентного журнала
        let journal = bus.query_journal(Some("user."), None, 10).await.unwrap();
        assert_eq!(journal.len(), 1);
        assert_eq!(journal[0].topic, "user.created");
    }
}
