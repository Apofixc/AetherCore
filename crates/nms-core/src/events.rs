// Модуль управления событиями реального времени, журнала событий и WebSocket рассылок (1-в-1 с backend/core/events.py)

use crate::bus::{SystemEvent, EVENT_BUS};
use crate::exceptions::NmsError;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info};

pub const MAX_CONNECTIONS_PER_USER: usize = 10;
pub const SEND_TIMEOUT_SECONDS: f64 = 2.0;
pub const HEARTBEAT_INTERVAL: f64 = 30.0;
pub const HEARTBEAT_TIMEOUT: f64 = 60.0;
pub const BATCH_INTERVAL: f64 = 0.1; // 100ms

fn current_time_sec() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Запись события в персистентный журнал SQLite. Возвращает seq_id (1-в-1 с Python record_event_in_db)
pub async fn record_event_in_db(
    pool: &sqlx::SqlitePool,
    event_type: &str,
    payload_json: &str,
    target_user_id: Option<&str>,
    topic: Option<&str>,
) -> Result<i64, NmsError> {
    let payload_val: serde_json::Value =
        serde_json::from_str(payload_json).unwrap_or(serde_json::Value::Null);
    crate::db::record_event_in_db(pool, event_type, &payload_val, target_user_id, topic)
        .await
        .map_err(|e| {
            error!("Failed to record WS event in SQLite journal: {}", e);
            NmsError::Internal {
                message: format!("Failed to record WS event in SQLite journal: {}", e),
                details: serde_json::json!({}),
            }
        })
}

/// Элемент очереди пакетной записи событий
pub struct EventJournalQueueItem {
    pub event_type: String,
    pub payload_json: String,
    pub target_user_id: Option<String>,
    pub topic: Option<String>,
    pub responder: oneshot::Sender<i64>,
}

/// Асинхронная пакетная очередь для высокопроизводительной записи событий в SQLite (1-в-1 с Python EventJournalQueue)
#[derive(Clone)]
pub struct EventJournalQueue {
    pub flush_interval: f64,
    pub max_batch_size: usize,
    sender: Arc<RwLock<Option<mpsc::Sender<EventJournalQueueItem>>>>,
}

impl EventJournalQueue {
    pub fn new(flush_interval: f64, max_batch_size: usize) -> Self {
        Self {
            flush_interval,
            max_batch_size,
            sender: Arc::new(RwLock::new(None)),
        }
    }

    pub fn ensure_started(&self, pool: sqlx::SqlitePool) {
        let mut guard = match self.sender.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        if guard.is_none() {
            let (tx, mut rx) = mpsc::channel::<EventJournalQueueItem>(10000);
            *guard = Some(tx);

            let flush_interval = self.flush_interval;
            let max_batch_size = self.max_batch_size;

            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(tokio::time::Duration::from_secs_f64(flush_interval));
                let mut batch: Vec<EventJournalQueueItem> = Vec::new();

                loop {
                    tokio::select! {
                        Some(item) = rx.recv() => {
                            batch.push(item);
                            if batch.len() >= max_batch_size {
                                Self::write_batch(&pool, &mut batch).await;
                            }
                        }
                        _ = interval.tick() => {
                            if !batch.is_empty() {
                                Self::write_batch(&pool, &mut batch).await;
                            }
                        }
                    }
                }
            });
        }
    }

    async fn write_batch(pool: &sqlx::SqlitePool, batch: &mut Vec<EventJournalQueueItem>) {
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(err) => {
                error!(
                    "Failed to execute SQLite batch insert (begin transaction): {}",
                    err
                );
                for item in batch.drain(..) {
                    let _ = item.responder.send(0);
                }
                return;
            }
        };

        for item in batch.drain(..) {
            let res = sqlx::query(
                "INSERT INTO system_events_journal (event_type, payload, target_user_id, topic)
                 VALUES (?, ?, ?, ?);",
            )
            .bind(&item.event_type)
            .bind(&item.payload_json)
            .bind(&item.target_user_id)
            .bind(&item.topic)
            .execute(&mut *tx)
            .await;

            let seq_id = match res {
                Ok(r) => r.last_insert_rowid(),
                Err(e) => {
                    error!("Failed to insert WS event item: {}", e);
                    0
                }
            };
            let _ = item.responder.send(seq_id);
        }

        if let Err(err) = tx.commit().await {
            error!("Failed to commit SQLite event journal batch: {}", err);
        }
    }

    pub async fn record_event_async(
        &self,
        pool: &sqlx::SqlitePool,
        event_type: &str,
        payload_json: &str,
        target_user_id: Option<&str>,
        topic: Option<&str>,
        _immediate: bool,
    ) -> Result<i64, NmsError> {
        self.ensure_started(pool.clone());

        let tx = {
            let guard = match self.sender.read() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            guard.clone()
        };

        if let Some(tx) = tx {
            let (resp_tx, resp_rx) = oneshot::channel();
            let item = EventJournalQueueItem {
                event_type: event_type.to_string(),
                payload_json: payload_json.to_string(),
                target_user_id: target_user_id.map(|s| s.to_string()),
                topic: topic.map(|s| s.to_string()),
                responder: resp_tx,
            };

            if tx.send(item).await.is_ok() {
                if let Ok(seq_id) = resp_rx.await {
                    return Ok(seq_id);
                }
            }
        }

        record_event_in_db(pool, event_type, payload_json, target_user_id, topic).await
    }
}

pub static EVENT_JOURNAL_QUEUE: LazyLock<EventJournalQueue> =
    LazyLock::new(|| EventJournalQueue::new(0.5, 500));

pub fn event_journal_queue() -> &'static EventJournalQueue {
    &EVENT_JOURNAL_QUEUE
}

/// Прунинг (очистка) старых и избыточных записей журнала system_events_journal (1-в-1 с Python prune_system_events_journal)
pub async fn prune_system_events_journal(
    pool: &sqlx::SqlitePool,
    max_age_days: i64,
    max_rows: i64,
) -> Result<u64, NmsError> {
    let result = sqlx::query(
        "DELETE FROM system_events_journal
         WHERE created_at < datetime('now', ?)
            OR seq_id NOT IN (
                SELECT seq_id FROM system_events_journal ORDER BY seq_id DESC LIMIT ?
            )",
    )
    .bind(format!("-{} days", max_age_days))
    .bind(max_rows)
    .execute(pool)
    .await
    .map_err(|e| {
        error!("Failed to prune system_events_journal: {}", e);
        NmsError::Internal {
            message: format!("Failed to prune system_events_journal: {}", e),
            details: serde_json::json!({}),
        }
    })?;

    Ok(result.rows_affected())
}

/// Проверка состояния истории событий и получение досланных записей без ложного resync_required (1-в-1 с Python check_replay_status_from_db)
pub async fn check_replay_status_from_db(
    pool: &sqlx::SqlitePool,
    last_event_id: i64,
    target_user_id: Option<&str>,
    topics: Option<&HashSet<String>>,
    limit: i64,
) -> Result<(String, Vec<serde_json::Value>), NmsError> {
    let row: Option<(Option<i64>, Option<i64>)> =
        sqlx::query_as("SELECT MIN(seq_id), MAX(seq_id) FROM system_events_journal")
            .fetch_optional(pool)
            .await
            .map_err(|e| NmsError::Internal {
                message: e.to_string(),
                details: serde_json::json!({}),
            })?;

    let (min_seq, max_seq) = match row {
        Some((min_s, max_s)) => (min_s.unwrap_or(0), max_s.unwrap_or(0)),
        None => (0, 0),
    };

    if max_seq == 0 || last_event_id >= max_seq {
        return Ok(("replay".to_string(), Vec::new()));
    }

    if min_seq > 0 && last_event_id < (min_seq - 1) {
        return Ok(("resync_required".to_string(), Vec::new()));
    }

    let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>, String)>(
        "SELECT seq_id, event_type, payload, target_user_id, topic, created_at
         FROM system_events_journal
         WHERE seq_id > ?
           AND (target_user_id IS NULL OR ? IS NULL OR target_user_id = ?)
         ORDER BY seq_id ASC
         LIMIT ?;",
    )
    .bind(last_event_id)
    .bind(target_user_id)
    .bind(target_user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| NmsError::Internal {
        message: e.to_string(),
        details: serde_json::json!({}),
    })?;

    let mut result = Vec::new();
    for (seq_id, event_type, payload_str, _target_uid, topic, created_at) in rows {
        if let Some(top_set) = topics {
            if let Some(t) = &topic {
                if !top_set.contains(t) {
                    continue;
                }
            }
        }

        let mut payload_dict: serde_json::Value = serde_json::from_str(&payload_str)
            .unwrap_or_else(|_| serde_json::json!({ "payload": payload_str }));

        if let Some(obj) = payload_dict.as_object_mut() {
            obj.insert("seq_id".to_string(), serde_json::json!(seq_id));
            obj.insert("created_at".to_string(), serde_json::json!(created_at));
            if !obj.contains_key("type") {
                obj.insert("type".to_string(), serde_json::json!(event_type));
            }
        }
        result.push(payload_dict);
    }

    Ok(("replay".to_string(), result))
}

/// Получение списка пропущенных событий из SQLite базы по last_event_id (1-в-1 с Python get_missed_events_from_db)
pub async fn get_missed_events_from_db(
    pool: &sqlx::SqlitePool,
    last_event_id: i64,
    target_user_id: Option<&str>,
    topics: Option<&HashSet<String>>,
    limit: i64,
) -> Result<Vec<serde_json::Value>, NmsError> {
    let (_, events) =
        check_replay_status_from_db(pool, last_event_id, target_user_id, topics, limit).await?;
    Ok(events)
}

/// Структура метаданных активного WebSocket подключения
#[derive(Clone)]
pub struct ActiveConnectionInfo {
    pub conn_id: usize,
    pub user_id: Option<String>,
    pub jti: Option<String>,
    pub exp: Option<f64>,
    pub connected_at: f64,
    pub last_pong_time: f64,
    pub topics: HashSet<String>,
    pub protocol_format: String,
    pub tx: mpsc::UnboundedSender<String>,
}

/// Элемент очереди батчинга событий
#[derive(Clone)]
pub struct BatchQueueItem {
    pub data: serde_json::Value,
    pub target_user_id: Option<String>,
    pub topic: Option<String>,
}

/// Менеджер WebSocket соединений с поддержкой рассылки, Heartbeat и Replay (1-в-1 с Python ConnectionManager)
#[derive(Clone)]
pub struct ConnectionManager {
    next_conn_id: Arc<RwLock<usize>>,
    active_connections: Arc<RwLock<HashMap<usize, ActiveConnectionInfo>>>,
    batch_queue: Arc<RwLock<Vec<BatchQueueItem>>>,
    metrics: Arc<RwLock<(u64, u64, u64)>>, // (total_sent, total_received, total_dropped)
    bg_tasks_started: Arc<RwLock<bool>>,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            next_conn_id: Arc::new(RwLock::new(1)),
            active_connections: Arc::new(RwLock::new(HashMap::new())),
            batch_queue: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(RwLock::new((0, 0, 0))),
            bg_tasks_started: Arc::new(RwLock::new(false)),
        }
    }

    /// Очистить соединения из active_connections, если превысили таймаут (1-в-1 с Python _prune_dead_connections)
    pub fn _prune_dead_connections(&self) {
        let now = current_time_sec();
        let mut dead = Vec::new();
        if let Ok(guard) = self.active_connections.read() {
            for (conn_id, info) in guard.iter() {
                if now - info.last_pong_time > HEARTBEAT_TIMEOUT {
                    dead.push(*conn_id);
                }
            }
        }
        for conn_id in dead {
            self.disconnect(conn_id);
        }
    }

    pub fn prune_dead_connections(&self) {
        self._prune_dead_connections();
    }

    /// Регистрация подключения сокета с версией subprotocol, формата msgpack/json, авто-отбраковкой и LRU вытеснением (1-в-1 с Python connect)
    pub fn connect(
        &self,
        user_id: Option<String>,
        jti: Option<String>,
        exp: Option<f64>,
        protocol_format: String,
        tx: mpsc::UnboundedSender<String>,
    ) -> Option<usize> {
        self._prune_dead_connections();

        let conn_id = {
            let mut guard = match self.next_conn_id.write() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            let id = *guard;
            *guard += 1;
            id
        };

        if let Some(uid) = &user_id {
            let mut user_conns: Vec<(usize, f64)> = Vec::new();
            if let Ok(guard) = self.active_connections.read() {
                for (id, info) in guard.iter() {
                    if info.user_id.as_deref() == Some(uid.as_str()) {
                        user_conns.push((*id, info.connected_at));
                    }
                }
            }

            if user_conns.len() >= MAX_CONNECTIONS_PER_USER {
                info!(
                    "Connection limit ({}) reached for user {}. Evicting oldest stale connection.",
                    MAX_CONNECTIONS_PER_USER, uid
                );
                user_conns
                    .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                if let Some((oldest_id, _)) = user_conns.first() {
                    self.disconnect(*oldest_id);
                }
            }
        }

        let now = current_time_sec();
        let info = ActiveConnectionInfo {
            conn_id,
            user_id: user_id.clone(),
            jti,
            exp,
            connected_at: now,
            last_pong_time: now,
            topics: HashSet::new(),
            protocol_format: protocol_format.clone(),
            tx,
        };

        if let Ok(mut guard) = self.active_connections.write() {
            guard.insert(conn_id, info);
            info!(
                "WebSocket client connected (conn_id={}, user_id={:?}, format={}, total={})",
                conn_id,
                user_id,
                protocol_format,
                guard.len()
            );
        }

        Some(conn_id)
    }

    /// Закрыть все открытые сокеты при остановке сервера (1-в-1 с Python close_all)
    pub fn close_all(&self, _code: u16, _reason: &str) {
        if let Ok(mut guard) = self.active_connections.write() {
            guard.clear();
        }
    }

    /// Отключение сокета и очистка метаданных (1-в-1 с Python disconnect)
    pub fn disconnect(&self, conn_id: usize) {
        if let Ok(mut guard) = self.active_connections.write() {
            if let Some(info) = guard.remove(&conn_id) {
                info!(
                    "WebSocket client disconnected (conn_id={}, user_id={:?}, total={})",
                    conn_id,
                    info.user_id,
                    guard.len()
                );
            }
        }
    }

    /// Обновить timestamp последнего PONG / активности сокета (1-в-1 с Python update_pong)
    pub fn update_pong(&self, conn_id: usize) {
        if let Ok(mut guard) = self.active_connections.write() {
            if let Some(info) = guard.get_mut(&conn_id) {
                info.last_pong_time = current_time_sec();
            }
        }
        if let Ok(mut guard) = self.metrics.write() {
            guard.1 += 1;
        }
    }

    /// Подписать подключение на топик (1-в-1 с Python subscribe_topic)
    pub fn subscribe_topic(&self, conn_id: usize, topic: String) {
        if let Ok(mut guard) = self.active_connections.write() {
            if let Some(info) = guard.get_mut(&conn_id) {
                if !topic.is_empty() {
                    info.topics.insert(topic);
                }
            }
        }
    }

    /// Отписать подключение от топика (1-в-1 с Python unsubscribe_topic)
    pub fn unsubscribe_topic(&self, conn_id: usize, topic: &str) {
        if let Ok(mut guard) = self.active_connections.write() {
            if let Some(info) = guard.get_mut(&conn_id) {
                info.topics.remove(topic);
            }
        }
    }

    pub fn set_loop(&self) {}
    pub fn update_loop_if_needed(&self) {}

    /// Получить текущие метрики WebSocket соединений (1-в-1 с Python get_metrics)
    pub fn get_metrics(&self) -> serde_json::Value {
        let active_len = self.active_connections.read().map(|g| g.len()).unwrap_or(0);
        let m = self.metrics.read().map(|g| *g).unwrap_or((0, 0, 0));
        serde_json::json!({
            "active_connections": active_len,
            "total_sent": m.0,
            "total_received": m.1,
            "total_dropped": m.2,
        })
    }

    pub fn _ensure_background_tasks(&self, pool: Option<sqlx::SqlitePool>) {
        let mut guard = match self.bg_tasks_started.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if !*guard {
            *guard = true;

            let self_hb = self.clone();
            let pool_hb = pool.clone();
            tokio::spawn(async move {
                self_hb._heartbeat_loop(pool_hb).await;
            });

            let self_batch = self.clone();
            tokio::spawn(async move {
                self_batch._batch_flush_loop().await;
            });

            if let Some(p) = pool {
                let self_prune = self.clone();
                tokio::spawn(async move {
                    self_prune._prune_loop(p).await;
                });
            }
        }
    }

    pub fn ensure_background_tasks(&self, pool: Option<sqlx::SqlitePool>) {
        self._ensure_background_tasks(pool);
    }

    /// Фоновый цикл Heartbeat: периодический ping, проверка exp и закрытие зависших сокетов (1-в-1 с Python _heartbeat_loop)
    pub async fn _heartbeat_loop(&self, _pool: Option<sqlx::SqlitePool>) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs_f64(HEARTBEAT_INTERVAL)).await;
            let now = current_time_sec();
            let mut stale = Vec::new();
            let mut expired = Vec::new();
            let mut pings = Vec::new();

            if let Ok(guard) = self.active_connections.read() {
                for (conn_id, info) in guard.iter() {
                    if let Some(exp) = info.exp {
                        if now > exp {
                            expired.push(*conn_id);
                            continue;
                        }
                    }
                    if now - info.last_pong_time > HEARTBEAT_TIMEOUT {
                        stale.push(*conn_id);
                    } else {
                        pings.push((*conn_id, info.tx.clone()));
                    }
                }
            }

            for conn_id in expired {
                self.disconnect(conn_id);
            }
            for conn_id in stale {
                self.disconnect(conn_id);
            }
            for (_conn_id, tx) in pings {
                let ping_msg = serde_json::json!({"type": "ping"}).to_string();
                let _ = tx.send(ping_msg);
            }
        }
    }

    pub fn _safe_send(&self, tx: &mpsc::UnboundedSender<String>, payload: &str) {
        if tx.send(payload.to_string()).is_ok() {
            if let Ok(mut g) = self.metrics.write() {
                g.0 += 1;
            }
        } else if let Ok(mut g) = self.metrics.write() {
            g.2 += 1;
        }
    }

    /// Мгновенная рассылка срочных/критических событий (1-в-1 с Python broadcast_immediate)
    pub async fn broadcast_immediate(
        &self,
        pool: Option<&sqlx::SqlitePool>,
        mut data: serde_json::Value,
        target_user_id: Option<&str>,
        topic: Option<&str>,
    ) {
        let event_type = data
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("event")
            .to_string();
        let payload_str = data.to_string();

        if let Some(p) = pool {
            if let Ok(seq_id) = event_journal_queue()
                .record_event_async(p, &event_type, &payload_str, target_user_id, topic, true)
                .await
            {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("seq_id".to_string(), serde_json::json!(seq_id));
                }
            }
        }

        let message = data.to_string();
        let target_str = target_user_id.map(|s| s.to_string());

        if let Ok(guard) = self.active_connections.read() {
            for info in guard.values() {
                if let Some(t_user) = &target_str {
                    if info.user_id.as_ref() != Some(t_user) {
                        continue;
                    }
                }
                if let Some(top) = topic {
                    if !info.topics.contains(top) {
                        continue;
                    }
                }
                self._safe_send(&info.tx, &message);
            }
        }
    }

    /// Добавление события в очередь пакетной рассылки (1-в-1 с Python broadcast_batched)
    pub async fn broadcast_batched(
        &self,
        pool: Option<&sqlx::SqlitePool>,
        mut data: serde_json::Value,
        target_user_id: Option<&str>,
        topic: Option<&str>,
    ) {
        let event_type = data
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("event")
            .to_string();
        let payload_str = data.to_string();

        if let Some(p) = pool {
            if let Ok(seq_id) = event_journal_queue()
                .record_event_async(p, &event_type, &payload_str, target_user_id, topic, false)
                .await
            {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("seq_id".to_string(), serde_json::json!(seq_id));
                }
            }
        }

        if let Ok(mut guard) = self.batch_queue.write() {
            guard.push(BatchQueueItem {
                data,
                target_user_id: target_user_id.map(|s| s.to_string()),
                topic: topic.map(|s| s.to_string()),
            });
        }
    }

    /// Фоновый цикл отправки накопившихся сообщений каждые 100 мс (1-в-1 с Python _batch_flush_loop)
    pub async fn _batch_flush_loop(&self) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs_f64(BATCH_INTERVAL)).await;

            let items = {
                let mut guard = match self.batch_queue.write() {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                };
                if guard.is_empty() {
                    continue;
                }
                let items = guard.clone();
                guard.clear();
                items
            };

            if let Ok(guard) = self.active_connections.read() {
                for info in guard.values() {
                    let mut user_events = Vec::new();
                    let mut seen_telemetry_keys = HashSet::new();

                    for item in items.iter().rev() {
                        if let Some(target) = &item.target_user_id {
                            if info.user_id.as_ref() != Some(target) {
                                continue;
                            }
                        }
                        if let Some(t) = &item.topic {
                            if !info.topics.contains(t) {
                                continue;
                            }
                        }

                        let t_key = item
                            .data
                            .get("telemetry_key")
                            .and_then(|v| v.as_str())
                            .or_else(|| {
                                if item.data.get("type").and_then(|v| v.as_str())
                                    == Some("telemetry")
                                {
                                    item.data.get("key").and_then(|v| v.as_str())
                                } else {
                                    None
                                }
                            });

                        if let Some(key) = t_key {
                            if seen_telemetry_keys.contains(key) {
                                continue;
                            }
                            seen_telemetry_keys.insert(key.to_string());
                        }

                        user_events.push(item.data.clone());
                    }

                    user_events.reverse();

                    if !user_events.is_empty() {
                        let batch_msg = serde_json::json!({
                            "type": "batch",
                            "events": user_events,
                        })
                        .to_string();
                        self._safe_send(&info.tx, &batch_msg);
                    }
                }
            }
        }
    }

    /// Фоновая периодическая очистка журнала событий каждые 6 часов (1-в-1 с Python _prune_loop)
    pub async fn _prune_loop(&self, pool: sqlx::SqlitePool) {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(21600)).await;
            let _ = prune_system_events_journal(&pool, 7, 50000).await;
        }
    }

    /// Досылка пропущенных сообщений по last_event_id (1-в-1 с Python send_replay)
    pub async fn send_replay(
        &self,
        pool: &sqlx::SqlitePool,
        tx: &mpsc::UnboundedSender<String>,
        conn_id: usize,
        last_event_id: i64,
        user_id: Option<&str>,
    ) {
        let topics = self
            .active_connections
            .read()
            .ok()
            .and_then(|g| g.get(&conn_id).map(|info| info.topics.clone()));

        match check_replay_status_from_db(pool, last_event_id, user_id, topics.as_ref(), 500).await
        {
            Ok((status, missed)) if status == "replay" => {
                let replay_msg = serde_json::json!({
                    "type": "replay",
                    "last_event_id": last_event_id,
                    "events": missed,
                })
                .to_string();
                self._safe_send(tx, &replay_msg);
            }
            _ => {
                let resync_msg = serde_json::json!({
                    "type": "resync_required",
                    "message": "Gap detected in event journal due to pruning",
                })
                .to_string();
                self._safe_send(tx, &resync_msg);
            }
        }
    }

    pub fn connect_user(&self, user_id: &str) -> bool {
        let (tx, _) = mpsc::unbounded_channel();
        self.connect(
            Some(user_id.to_string()),
            None,
            None,
            "json".to_string(),
            tx,
        )
        .is_some()
    }

    pub fn disconnect_user(&self, user_id: &str) {
        let mut target_id = None;
        if let Ok(guard) = self.active_connections.read() {
            for (id, info) in guard.iter() {
                if info.user_id.as_deref() == Some(user_id) {
                    target_id = Some(*id);
                    break;
                }
            }
        }
        if let Some(id) = target_id {
            self.disconnect(id);
        }
    }
}

pub static WS_MANAGER: LazyLock<ConnectionManager> = LazyLock::new(ConnectionManager::new);

pub fn ws_manager() -> &'static ConnectionManager {
    &WS_MANAGER
}

/// Броадкастер событий для WebSockets (1-в-1 с Python EventBroadcaster)
#[derive(Clone, Default)]
pub struct EventBroadcaster;

impl EventBroadcaster {
    pub fn new() -> Self {
        Self
    }

    pub fn broadcast(
        &self,
        pool: Option<&sqlx::SqlitePool>,
        message: &str,
        data_dict: Option<serde_json::Value>,
        target_user_id: Option<&str>,
        topic: Option<&str>,
        immediate: bool,
    ) {
        let data = match data_dict {
            Some(d) => d,
            None => {
                if !message.is_empty() {
                    serde_json::from_str(message).unwrap_or_else(
                        |_| serde_json::json!({"type": "raw_event", "payload": message}),
                    )
                } else {
                    return;
                }
            }
        };

        if data.is_null() {
            return;
        }

        let mgr = ws_manager();
        mgr.ensure_background_tasks(pool.cloned());

        if immediate {
            let pool_owned = pool.cloned();
            let target_owned = target_user_id.map(|s| s.to_string());
            let topic_owned = topic.map(|s| s.to_string());
            tokio::spawn(async move {
                mgr.broadcast_immediate(
                    pool_owned.as_ref(),
                    data,
                    target_owned.as_deref(),
                    topic_owned.as_deref(),
                )
                .await;
            });
        } else {
            let pool_owned = pool.cloned();
            let target_owned = target_user_id.map(|s| s.to_string());
            let topic_owned = topic.map(|s| s.to_string());
            tokio::spawn(async move {
                mgr.broadcast_batched(
                    pool_owned.as_ref(),
                    data,
                    target_owned.as_deref(),
                    topic_owned.as_deref(),
                )
                .await;
            });
        }
    }
}

pub static BROADCASTER: LazyLock<EventBroadcaster> = LazyLock::new(EventBroadcaster::new);

pub fn broadcaster() -> &'static EventBroadcaster {
    &BROADCASTER
}

/// Уведомить всех клиентов об изменении настроек модуля (1-в-1 с Python notify_settings_changed)
pub async fn notify_settings_changed(
    pool: Option<&sqlx::SqlitePool>,
    module_id: &str,
    title: Option<&str>,
    body: Option<&str>,
) {
    debug!("Settings changed for module: {}", module_id);
    let payload = serde_json::json!({
        "type": "module_settings_changed",
        "module_id": module_id,
    });
    BROADCASTER.broadcast(pool, "", Some(payload), None, None, true);

    if let Some(pool) = pool {
        let default_title = format!("Изменены настройки модуля '{}'", module_id);
        let default_body = format!(
            "Конфигурация модуля '{}' была успешно обновлена.",
            module_id
        );
        let notif_title = title.unwrap_or(&default_title);
        let notif_body = body.unwrap_or(&default_body);

        if let Ok(users) = sqlx::query_as::<_, (i64,)>("SELECT id FROM users WHERE is_active = 1")
            .fetch_all(pool)
            .await
        {
            for (uid,) in users {
                let _ = crate::notify::notify(
                    pool,
                    crate::notify::NotifyParams {
                        user_id: uid.to_string(),
                        title: notif_title.to_string(),
                        body: notif_body.to_string(),
                        severity: crate::notify::NotificationSeverity::Info,
                        category: "system".to_string(),
                        entity_id: None,
                        module_id: module_id.to_string(),
                        allow_push: false,
                        target_url: None,
                        actions: None,
                        title_template: None,
                    },
                )
                .await;
            }
        }
    }
}

/// Мост между внутрипроцессной шиной EventBus и WebSocket клиентов (1-в-1 с Python EventBusWsBridge)
#[derive(Clone)]
pub struct EventBusWsBridge {
    pub allowed_patterns: Vec<String>,
    pub allow_core: bool,
    subscribed: Arc<RwLock<bool>>,
}

impl EventBusWsBridge {
    pub fn new(allowed_patterns: Option<Vec<String>>, allow_core: bool) -> Self {
        Self {
            allowed_patterns: allowed_patterns.unwrap_or_else(|| vec!["#".to_string()]),
            allow_core,
            subscribed: Arc::new(RwLock::new(false)),
        }
    }

    pub fn setup(&self) {
        let mut guard = match self.subscribed.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if *guard {
            return;
        }

        let allow_core = self.allow_core;
        for pattern in &self.allowed_patterns {
            let p = pattern.clone();
            EVENT_BUS.subscribe(
                p,
                Arc::new(move |event| {
                    if event.topic.starts_with("core.") && !allow_core {
                        return;
                    }
                    let ws_payload = serde_json::json!({
                        "type": "bus_event",
                        "topic": event.topic,
                        "payload": event.payload,
                    });
                    BROADCASTER.broadcast(
                        None,
                        "",
                        Some(ws_payload),
                        None,
                        Some(&event.topic),
                        true,
                    );
                }),
            );
        }
        *guard = true;
    }

    pub fn on_bus_event(&self, topic: &str, payload: &serde_json::Value) {
        if topic.starts_with("core.") && !self.allow_core {
            return;
        }
        let ws_payload = serde_json::json!({
            "type": "bus_event",
            "topic": topic,
            "payload": payload,
        });
        BROADCASTER.broadcast(None, "", Some(ws_payload), None, Some(topic), true);
    }
}

pub static BUS_WS_BRIDGE: LazyLock<EventBusWsBridge> =
    LazyLock::new(|| EventBusWsBridge::new(None, false));

pub fn bus_ws_bridge() -> &'static EventBusWsBridge {
    &BUS_WS_BRIDGE
}
