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

/// Экземпляр гибридной шины событий платформы
///
/// Предоставляет двойной контур маршрутизации:
/// - **Broadcast канал**: легковесная pub/sub модель для live-подписчиков (WebSockets, SSE, real-time UI).
/// - **Reliable журнал**: асинхронный MPSC воркер, персистентно сохраняющий важные события в SQLite WAL.
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
    /// Инициализировать шину событий и запустить фоновый воркер персистентного журнала
    ///
    /// Создает broadcast канал с буфером на 1024 сообщения и MPSC очередь на 4096 записей.
    /// Автоматически запускает асинхронную задачу Tokio для фонового сохранения событий в базу данных.
    ///
    /// # Аргументы
    /// * `db` — Пул подключений к базе данных SQLite ([`Db`]).
    ///
    /// # Примеры
    /// ```rust,no_run
    /// use nms_core::bus::EventBus;
    /// use nms_core::db::Db;
    ///
    /// # async fn run(db: Db) {
    /// let event_bus = EventBus::new(db);
    /// # }
    /// ```
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
    ///
    /// Событие безусловно отправляется в live broadcast-канал.
    /// Если `event.event_type == EventType::Reliable`, оно также помещается в очередь
    /// персистентной фиксации в журнале SQLite.
    ///
    /// # Аргументы
    /// * `event` — Публикуемое событие ([`EventMessage`]).
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Internal`](nms_common::error::AppError), если очередь
    /// надежного журнала переполнена или закрыта.
    pub async fn publish(&self, event: EventMessage) -> Result<()> {
        // 1. Отправляем в Live Broadcast канал (если есть подписчики)
        let _ = self.broadcast_tx.send(event.clone());

        // 2. Если событие типа Reliable — ставим в очередь персистентной записи
        if event.event_type == EventType::Reliable {
            self.journal_tx
                .send(event)
                .await
                .map_err(|e| {
                    AppError::internal(format!("Failed to enqueue event for persistent journal: {}", e))
                })?;
        }

        Ok(())
    }

    /// Подписаться на поток событий в реальном времени
    ///
    /// Возвращает асинхронный приемник [`broadcast::Receiver<EventMessage>`],
    /// через который можно читать все входящие события в реальном времени.
    pub fn subscribe(&self) -> broadcast::Receiver<EventMessage> {
        self.broadcast_tx.subscribe()
    }

    /// Запросить исторические события из надежного журнала с пагинацией и фильтрацией
    ///
    /// # Аргументы
    /// * `topic_filter` — Опциональный префикс темы (например, `"device."` или `"system.auth"`).
    /// * `after_id` — ID последней прочитанной записи для пагинации (курсор).
    /// * `limit` — Максимальное число возвращаемых записей (ограничивается диапазоном 1..=1000).
    ///
    /// # Возвращаемое значение
    /// Список сохраненных записей журнала [`ReliableEventRecord`] в порядке возрастания ID.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое выполнения SQL-запроса.
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
}

/// Персистентная запись надежного события в таблицу журнала SQLite `event_journal`
///
/// Сериализует JSON-пейлоад события и сохраняет запись с временной меткой в формате RFC 3339.
///
/// # Аргументы
/// * `db` — Экземпляр базы данных SQLite ([`Db`]).
/// * `event` — Сохраняемое событие ([`EventMessage`]).
///
/// # Ошибки
/// - [`AppError::Validation`](nms_common::error::AppError) — при ошибке сериализации полезной нагрузки в JSON.
/// - [`AppError::Database`](nms_common::error::AppError) — при сбое выполнения SQL-вставки.
async fn record_event_to_db(db: &Db, event: &EventMessage) -> Result<()> {
    let payload_str = serde_json::to_string(&event.payload).map_err(|e| {
        AppError::validation("payload", e.to_string())
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
    .map_err(|e| AppError::database(e.to_string()))?;

    Ok(())
}
