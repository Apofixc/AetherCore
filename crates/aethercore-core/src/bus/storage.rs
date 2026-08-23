//! # Адаптер постоянного хранилища L2 (SQLite Storage & Garbage Collector)
//!
//! Обеспечивает микро-батчинг персистентной записи, автоочистку при старте системы,
//! ротацию по TTL и усечение таблицы по лимиту записей.

use crate::db::Db;
use aethercore_common::error::{AppError, Result};
use aethercore_common::models::events::{EventMessage, ReliableEventRecord};
use chrono::{DateTime, Duration, Utc};
use std::time::Duration as StdDuration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace};

/// Максимальное количество записей в одном микро-батче SQLite
const BATCH_SIZE: usize = 50;
/// Максимальное время ожидания микро-батча
const BATCH_TIMEOUT: StdDuration = StdDuration::from_millis(30);
/// Лимит записей в таблице журнала по умолчанию
pub const DEFAULT_MAX_JOURNAL_RECORDS: usize = 5000;

/// Фоновый менеджер постоянного хранилища событий
#[derive(Clone, Debug)]
pub struct EventStorage {
    db: Db,
    tx: mpsc::Sender<EventMessage>,
}

impl EventStorage {
    /// Инициализировать хранилище, выполнить очистку устаревших записей при старте и запустить воркер
    pub fn new(db: Db) -> Self {
        let (tx, mut rx) = mpsc::channel(4096);
        let worker_db = db.clone();

        // 1. Асинхронный воркер микро-батчинга и записи
        tokio::spawn(async move {
            debug!("Event Storage L2 background worker started");

            // Выполняем агрессивную очистку устаревших хвостов прошлого запуска
            if let Err(e) = startup_prune(&worker_db).await {
                error!("Failed to perform startup prune on event_journal: {}", e);
            }

            let mut batch = Vec::with_capacity(BATCH_SIZE);
            let mut last_gc = tokio::time::Instant::now();

            loop {
                // Накапливаем микро-батч
                let timeout = tokio::time::sleep(BATCH_TIMEOUT);
                tokio::pin!(timeout);

                tokio::select! {
                    Some(event) = rx.recv() => {
                        batch.push(event);
                        if batch.len() >= BATCH_SIZE {
                            if let Err(e) = flush_batch(&worker_db, &batch).await {
                                error!("Failed to flush event batch to database: {}", e);
                            }
                            batch.clear();
                        }
                    }
                    _ = &mut timeout => {
                        if !batch.is_empty() {
                            if let Err(e) = flush_batch(&worker_db, &batch).await {
                                error!("Failed to flush event batch to database: {}", e);
                            }
                            batch.clear();
                        }
                    }
                    else => {
                        // Канал закрыт — сбрасываем остаток и выходим
                        if !batch.is_empty() {
                            let _ = flush_batch(&worker_db, &batch).await;
                        }
                        break;
                    }
                }

                // Периодический сборщик мусора (раз в 5 минут)
                if last_gc.elapsed() >= StdDuration::from_secs(300) {
                    if let Err(e) = run_periodic_gc(&worker_db).await {
                        error!("Event Storage periodic GC failed: {}", e);
                    }
                    last_gc = tokio::time::Instant::now();
                }
            }

            info!("Event Storage worker terminated");
        });

        Self { db, tx }
    }

    /// Поставить событие в очередь персистентной записи
    pub async fn persist(&self, event: EventMessage) -> Result<()> {
        self.tx.send(event).await.map_err(|e| {
            AppError::internal(format!("Event storage queue closed: {}", e))
        })
    }

    /// Запросить исторические события из базы данных
    pub async fn query(
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

        let rows = query_sql.map_err(|e| {
            AppError::database(format!("Failed to query event journal: {}", e))
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

    /// Ручная ротация и очистка устаревших записей
    pub async fn prune(&self, max_age: Option<Duration>, max_count: Option<usize>) -> Result<u64> {
        let mut total_deleted = 0u64;

        if let Some(age) = max_age {
            let cutoff = (Utc::now() - age).to_rfc3339();
            let res = sqlx::query("DELETE FROM event_journal WHERE created_at < ?")
                .bind(cutoff)
                .execute(self.db.writer())
                .await
                .map_err(|e| AppError::database(format!("Failed to prune events by age: {}", e)))?;
            total_deleted += res.rows_affected();
        }

        let max_count = max_count.unwrap_or(DEFAULT_MAX_JOURNAL_RECORDS) as i64;
        let res = sqlx::query(
            r#"
            DELETE FROM event_journal
            WHERE id NOT IN (
                SELECT id FROM event_journal ORDER BY id DESC LIMIT ?
            )
            "#,
        )
        .bind(max_count)
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(format!("Failed to prune events by count: {}", e)))?;

        total_deleted += res.rows_affected();
        Ok(total_deleted)
    }
}

/// Агрессивная автоочистка при старте ядра
async fn startup_prune(db: &Db) -> Result<()> {
    let now_str = Utc::now().to_rfc3339();
    let one_day_ago = (Utc::now() - Duration::hours(24)).to_rfc3339();

    sqlx::query(
        r#"
        DELETE FROM event_journal
        WHERE created_at < ?
        "#,
    )
    .bind(one_day_ago)
    .execute(db.writer())
    .await
    .map_err(|e| AppError::database(format!("Startup prune failed: {}", e)))?;

    trace!("Event storage startup prune completed at {}", now_str);
    Ok(())
}

/// Сброс пачки событий в базу данных в рамках транзакции
async fn flush_batch(db: &Db, batch: &[EventMessage]) -> Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    let mut tx = db.writer().begin().await.map_err(|e| {
        AppError::database(format!("Failed to start transaction for event batch: {}", e))
    })?;

    for event in batch {
        let payload_str = serde_json::to_string(&event.payload).unwrap_or_else(|_| "{}".to_string());
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
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::database(format!("Failed to insert event into journal: {}", e)))?;
    }

    tx.commit().await.map_err(|e| {
        AppError::database(format!("Failed to commit event batch transaction: {}", e))
    })?;

    trace!("Flushed {} events to SQLite journal", batch.len());
    Ok(())
}

/// Периодический сборщик мусора
async fn run_periodic_gc(db: &Db) -> Result<()> {
    let max_count = DEFAULT_MAX_JOURNAL_RECORDS as i64;
    sqlx::query(
        r#"
        DELETE FROM event_journal
        WHERE id NOT IN (
            SELECT id FROM event_journal ORDER BY id DESC LIMIT ?
        )
        "#,
    )
    .bind(max_count)
    .execute(db.writer())
    .await
    .map_err(|e| AppError::database(format!("Periodic event GC failed: {}", e)))?;

    Ok(())
}
