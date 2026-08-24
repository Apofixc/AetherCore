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
use tracing::{debug, error, info, trace, warn};

/// Максимальное количество записей в одном микро-батче SQLite
const BATCH_SIZE: usize = 50;
/// Максимальное время ожидания микро-батча после получения первого сообщения
const BATCH_TIMEOUT: StdDuration = StdDuration::from_millis(30);
/// Лимит записей в таблице журнала по умолчанию
pub const DEFAULT_MAX_JOURNAL_RECORDS: usize = 5000;

/// Конфигурация параметров постоянного хранилища L2
#[derive(Debug, Clone)]
pub struct EventStorageConfig {
    /// Максимальное количество записей в журнале
    pub max_records: usize,
    /// Максимальный возраст сохраняемых записей при старте ядра
    pub startup_prune_age: Option<Duration>,
}

impl Default for EventStorageConfig {
    fn default() -> Self {
        Self {
            max_records: DEFAULT_MAX_JOURNAL_RECORDS,
            startup_prune_age: Some(Duration::hours(24)),
        }
    }
}

/// Фоновый менеджер постоянного хранилища событий
#[derive(Clone, Debug)]
pub struct EventStorage {
    db: Db,
    tx: mpsc::Sender<EventMessage>,
    config: EventStorageConfig,
}

impl EventStorage {
    /// Инициализировать хранилище с дефолтной конфигурацией
    pub fn new(db: Db) -> Self {
        Self::with_config(db, EventStorageConfig::default())
    }

    /// Инициализировать хранилище с пользовательской конфигурацией
    pub fn with_config(db: Db, config: EventStorageConfig) -> Self {
        let (tx, mut rx) = mpsc::channel(4096);
        let worker_db = db.clone();
        let worker_config = config.clone();

        // 1. Асинхронный воркер микро-батчинга и записи
        tokio::spawn(async move {
            debug!("Event Storage L2 background worker started");

            // Выполняем автоочистку устаревших записей при старте ядра
            if let Some(prune_age) = worker_config.startup_prune_age {
                if let Err(e) = startup_prune(&worker_db, prune_age).await {
                    error!("Failed to perform startup prune on event_journal: {}", e);
                }
            }

            let mut batch = Vec::with_capacity(BATCH_SIZE);
            let mut last_gc = tokio::time::Instant::now();

            // Цикл батчинга без пустых wake-up: ожидаем первое событие по rx.recv()
            while let Some(first_event) = rx.recv().await {
                batch.push(first_event);

                // Добираем пачку до BATCH_SIZE с таймаутом BATCH_TIMEOUT
                while batch.len() < BATCH_SIZE {
                    match tokio::time::timeout(BATCH_TIMEOUT, rx.recv()).await {
                        Ok(Some(ev)) => batch.push(ev),
                        _ => break,
                    }
                }

                if let Err(e) = flush_batch(&worker_db, &batch).await {
                    error!("Failed to flush event batch to database: {}", e);
                }
                batch.clear();

                // Периодический сборщик мусора (раз в 5 минут при наличии активности)
                if last_gc.elapsed() >= StdDuration::from_secs(300) {
                    if let Err(e) = run_periodic_gc(&worker_db, worker_config.max_records).await {
                        error!("Event Storage periodic GC failed: {}", e);
                    }
                    last_gc = tokio::time::Instant::now();
                }
            }

            info!("Event Storage worker terminated");
        });

        Self { db, tx, config }
    }

    /// Поставить событие в очередь персистентной записи (неблокирующая отправка)
    pub fn try_persist(&self, event: EventMessage) -> Result<()> {
        self.tx.try_send(event).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => {
                warn!("Event storage queue is full, reliable event dropped from L2 queue");
                AppError::internal("Event storage queue is full")
            }
            mpsc::error::TrySendError::Closed(_) => {
                AppError::internal("Event storage queue closed")
            }
        })
    }

    /// Поставить событие в очередь персистентной записи
    pub async fn persist(&self, event: EventMessage) -> Result<()> {
        self.try_persist(event)
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
                let pattern = format!("{}%", escape_sql_like(prefix));
                sqlx::query_as::<_, (i64, String, String, String, String, String)>(
                    r#"
                    SELECT id, event_uuid, topic, source, payload_json, created_at
                    FROM event_journal
                    WHERE id > ? AND topic LIKE ? ESCAPE '\'
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

    /// Запросить самые свежие исторические записи (хвост истории)
    pub async fn query_recent(
        &self,
        topic_filter: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ReliableEventRecord>> {
        let limit = limit.min(1000).max(1) as i64;

        let query_sql = match topic_filter {
            Some(prefix) if !prefix.is_empty() => {
                let pattern = format!("{}%", escape_sql_like(prefix));
                sqlx::query_as::<_, (i64, String, String, String, String, String)>(
                    r#"
                    SELECT id, event_uuid, topic, source, payload_json, created_at
                    FROM event_journal
                    WHERE topic LIKE ? ESCAPE '\'
                    ORDER BY id DESC
                    LIMIT ?
                    "#,
                )
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
                    ORDER BY id DESC
                    LIMIT ?
                    "#,
                )
                .bind(limit)
                .fetch_all(self.db.reader())
                .await
            }
        };

        let rows = query_sql.map_err(|e| {
            AppError::database(format!("Failed to query recent events from journal: {}", e))
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

        // Переворачиваем в хронологический порядок
        records.reverse();
        Ok(records)
    }

    /// Ручная ротация и очистка устаревших записей журнала
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

        let max_count = max_count.unwrap_or(self.config.max_records) as i64;
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

/// Экранирование специальных символов SQL LIKE (`%`, `_`, `\`)
fn escape_sql_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Автоочистка устаревших записей при старте ядра
async fn startup_prune(db: &Db, max_age: Duration) -> Result<()> {
    let now_str = Utc::now().to_rfc3339();
    let cutoff = (Utc::now() - max_age).to_rfc3339();

    sqlx::query(
        r#"
        DELETE FROM event_journal
        WHERE created_at < ?
        "#,
    )
    .bind(cutoff)
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
            INSERT OR IGNORE INTO event_journal (event_uuid, topic, source, payload_json, created_at)
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
async fn run_periodic_gc(db: &Db, max_count: usize) -> Result<()> {
    let max_records = max_count as i64;
    sqlx::query(
        r#"
        DELETE FROM event_journal
        WHERE id NOT IN (
            SELECT id FROM event_journal ORDER BY id DESC LIMIT ?
        )
        "#,
    )
    .bind(max_records)
    .execute(db.writer())
    .await
    .map_err(|e| AppError::database(format!("Periodic event GC failed: {}", e)))?;

    Ok(())
}

