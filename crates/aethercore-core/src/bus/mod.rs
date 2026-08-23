//! # Высокопроизводительная гибридная шина событий платформы (EventBus)
//!
//! Предоставляет:
//! - **Двухуровневое хранилище**: ультрабыстрый L1 In-Memory кольцевой буфер ([`EventRingBuffer`]) для нулевого I/O в UI + надежное L2 SQLite хранилище ([`EventStorage`]) с коротким TTL и автоочисткой.
//! - **Маршрутизация топиков**: префиксное дерево ([`TopicRouter`]) с поддержкой MQTT-подобных масок `*` (один сегмент) и `#` (произвольный хвост топика).
//! - **Жизненный цикл подписок**: RAII-дескрипторы ([`SubscriptionHandle`]) с автоматической отпиской при выходе из области видимости (`Drop`) и динамическим управлением темами на лету (`add_topic` / `remove_topic`).
//! - **Приоритеты и защита от голодания**: взвешенные очереди Weighted Fair Queuing ([`PriorityQueueSender`]) с квотами 8:4:2:1, гарантирующие доставку низкоприоритетных событий при шторме алармов.
//! - **Устойчивость и дедупликация**: фильтрация повторов по UUID ([`EventDeduplicator`]) и конвейер перехватчиков ([`InterceptorPipeline`]).
//! - **In-Process Request-Reply RPC**: асинхронный синхронизированный вызов команд между модулями ядра и плагинами.

pub mod dedup;
pub mod interceptor;
pub mod queue;
pub mod ring;
pub mod router;
pub mod stats;
pub mod storage;

pub use dedup::EventDeduplicator;
pub use interceptor::{EventInterceptor, InterceptorAction, InterceptorPipeline, MaskingInterceptor};
pub use queue::{create_priority_queue, PriorityQueueReceiver, PriorityQueueSender};
pub use ring::EventRingBuffer;
pub use router::{SubscriptionHandle, SubscriptionId, TopicRouter};
pub use stats::{BusMetrics, BusStats};
pub use storage::EventStorage;

use crate::db::Db;
use aethercore_common::error::{AppError, Result};
use aethercore_common::models::events::{EventMessage, EventType, ReliableEventRecord};
use chrono::Duration;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tracing::{debug, error, trace};
use uuid::Uuid;

/// Экземпляр гибридной шины событий платформы
///
/// Объединяет оперативную память L1 (RingBuffer) для горячей телеметрии и персистентную базу данных L2 (SQLite)
/// для надежных событий с поддержкой взвешенных очередей (WFQ), масок топиков и автоочистки устаревших записей.
#[derive(Clone, Debug)]
pub struct EventBus {
    router: TopicRouter,
    queue_tx: PriorityQueueSender,
    ring: EventRingBuffer,
    dedup: EventDeduplicator,
    interceptors: InterceptorPipeline,
    storage: Option<EventStorage>,
    metrics: BusMetrics,
}

impl EventBus {
    /// Инициализировать шину событий с подключением к базе данных SQLite для L2 хранилища
    ///
    /// Создает префиксный роутер, L1 RingBuffer на 2048 событий, очередь приоритетов WFQ
    /// и запускает асинхронный воркер записи в базу данных с микро-батчингом.
    ///
    /// # Аргументы
    /// * `db` — Пул подключений к базе данных платформы ([`Db`]).
    ///
    /// # Примеры
    /// ```rust,no_run
    /// use aethercore_core::bus::EventBus;
    /// use aethercore_core::db::Db;
    ///
    /// # async fn run(db: Db) {
    /// let event_bus = EventBus::new(db);
    /// # }
    /// ```
    pub fn new(db: Db) -> Self {
        Self::with_options(Some(db), ring::DEFAULT_RING_CAPACITY)
    }

    /// Инициализировать полностью In-Memory шину событий без дискового хранилища L2
    ///
    /// Идеально подходит для тестов, легковесных микросервисов и изолированных окружений.
    ///
    /// # Примеры
    /// ```rust
    /// use aethercore_core::bus::EventBus;
    ///
    /// # async fn run() {
    /// let event_bus = EventBus::in_memory();
    /// # }
    /// ```
    pub fn in_memory() -> Self {
        Self::with_options(None, ring::DEFAULT_RING_CAPACITY)
    }

    /// Инициализировать шину событий с точной настройкой хранилища и емкости L1 кэша
    ///
    /// # Аргументы
    /// * `db` — Опциональный пул базы данных для L2 хранилища.
    /// * `ring_capacity` — Емкость горячего L1 кольцевого буфера в оперативной памяти (в количестве сообщений).
    pub fn with_options(db: Option<Db>, ring_capacity: usize) -> Self {
        let router = TopicRouter::new();
        let (queue_tx, mut queue_rx) = create_priority_queue();
        let ring = EventRingBuffer::new(ring_capacity);
        let dedup = EventDeduplicator::default();
        let mut interceptors = InterceptorPipeline::new();
        interceptors.register(Arc::new(MaskingInterceptor::default()));
        let storage = db.map(EventStorage::new);
        let metrics = BusMetrics::default();

        let bus = Self {
            router: router.clone(),
            queue_tx,
            ring: ring.clone(),
            dedup: dedup.clone(),
            interceptors: interceptors.clone(),
            storage: storage.clone(),
            metrics: metrics.clone(),
        };

        // Запуск фонового диспетчера взвешенных очередей
        let dispatch_router = router.clone();
        let dispatch_ring = ring.clone();
        let dispatch_storage = storage.clone();
        let dispatch_metrics = metrics.clone();

        tokio::spawn(async move {
            debug!("EventBus weighted fair queuing dispatcher started");
            while let Some(event) = queue_rx.dequeue().await {
                // 1. Сохраняем в горячий L1 кольцевой буфер
                let evicted = dispatch_ring.push(event.clone());

                // 2. Если вытеснено надежное событие и есть L2 хранилище — сбрасываем в L2 Spillover
                if let Some(evicted_msg) = evicted {
                    if evicted_msg.event_type == EventType::Reliable {
                        if let Some(ref st) = dispatch_storage {
                            let _ = st.persist(evicted_msg).await;
                        }
                    }
                }

                // 3. Если событие типа Reliable — сразу ставим в очередь сохранения L2
                if event.event_type == EventType::Reliable {
                    if let Some(ref st) = dispatch_storage {
                        if let Err(e) = st.persist(event.clone()).await {
                            error!("Failed to enqueue reliable event to L2 storage: {}", e);
                        }
                    }
                }

                // 4. Доставляем всем подходящим подписчикам через TopicRouter
                let delivered = dispatch_router.dispatch(&event);
                trace!("Dispatched event '{}' to {} subscribers", event.topic, delivered);

                dispatch_metrics.0.record_published(event.priority);
            }
            debug!("EventBus dispatcher terminated");
        });

        bus
    }

    /// Опубликовать событие в шину
    ///
    /// Проверяет дедупликацию по UUID, выполняет перехватчики `pre_publish`
    /// и ставит сообщение во взвешенную очередь диспетчера согласно его приоритету.
    ///
    /// # Аргументы
    /// * `event` — Публикуемое событие платформы ([`EventMessage`]).
    ///
    /// # Ошибки
    /// Возвращает [`AppError`], если перехватчик отклонил публикацию или очередь переполнена/закрыта.
    ///
    /// # Примеры
    /// ```rust
    /// use aethercore_core::bus::EventBus;
    /// use aethercore_common::models::events::EventMessage;
    /// use serde_json::json;
    ///
    /// # async fn run() {
    /// let bus = EventBus::in_memory();
    /// let event = EventMessage::telemetry("sensors.temp", "sensor-1", json!({"val": 22.4}));
    /// bus.publish(event).await.unwrap();
    /// # }
    /// ```
    pub async fn publish(&self, mut event: EventMessage) -> Result<()> {
        // Проверка на дубликат по UUID
        if self.dedup.is_duplicate_or_record(&event.id) {
            trace!("Ignoring duplicate event {}", event.id);
            return Ok(());
        }

        // Выполнение конвейера перехватчиков (pre-publish)
        match self.interceptors.execute_pre(&mut event).await? {
            InterceptorAction::Continue => {}
            InterceptorAction::DropSilently => return Ok(()),
            InterceptorAction::Reject(err) => return Err(err),
        }

        // Постановка во взвешенную очередь диспетчера
        self.queue_tx.enqueue(event.clone()).await?;

        // Выполнение конвейера перехватчиков (post-publish)
        self.interceptors.execute_post(&event).await;

        Ok(())
    }

    /// Массовая публикация пакета событий
    ///
    /// Последовательно публикует список событий в шину с оптимизированным контролем ошибок.
    ///
    /// # Аргументы
    /// * `events` — Вектор событий ([`Vec<EventMessage>`]).
    pub async fn publish_batch(&self, events: Vec<EventMessage>) -> Result<()> {
        for event in events {
            self.publish(event).await?;
        }
        Ok(())
    }

    /// Подписаться на все события платформы (маска `#`)
    ///
    /// Возвращает RAII-дескриптор [`SubscriptionHandle`], автоматически отписывающийся при `Drop`.
    ///
    /// # Примеры
    /// ```rust
    /// use aethercore_core::bus::EventBus;
    ///
    /// # async fn run() {
    /// let bus = EventBus::in_memory();
    /// let mut sub = bus.subscribe();
    /// // sub.recv().await ...
    /// # }
    /// ```
    pub fn subscribe(&self) -> SubscriptionHandle {
        self.router.subscribe(&["#"])
    }

    /// Подписаться на конкретный топик или маску (`*`, `#`)
    ///
    /// # Аргументы
    /// * `pattern` — Шаблон топика (например, `"devices.*.status"` или `"alarms.#"`).
    ///
    /// # Примеры
    /// ```rust
    /// use aethercore_core::bus::EventBus;
    ///
    /// # async fn run() {
    /// let bus = EventBus::in_memory();
    /// let mut sub = bus.subscribe_topic("devices.*.metrics");
    /// # }
    /// ```
    pub fn subscribe_topic(&self, pattern: impl AsRef<str>) -> SubscriptionHandle {
        self.router.subscribe(&[pattern.as_ref()])
    }

    /// Подписаться на несколько топиков или шаблонов одновременно
    ///
    /// # Аргументы
    /// * `patterns` — Срез шаблонов топиков (например, `&["system.started", "users.#"]`).
    pub fn subscribe_topics(&self, patterns: &[&str]) -> SubscriptionHandle {
        self.router.subscribe(patterns)
    }

    /// Динамически добавить тему к существующей подписке по ее идентификатору
    ///
    /// Позволяет клиентам (например, WebSocket-соединениям) расширять список прослушиваемых
    /// тем без пересоздания канала подписки.
    ///
    /// # Аргументы
    /// * `sub_id` — Идентификатор активной подписки ([`SubscriptionId`]).
    /// * `pattern` — Добавляемый шаблон топика.
    pub fn add_subscription_topic(&self, sub_id: router::SubscriptionId, pattern: impl AsRef<str>) {
        self.router.add_topic(sub_id, pattern.as_ref());
    }

    /// Динамически удалить тему из существующей подписки по ее идентификатору
    ///
    /// # Аргументы
    /// * `sub_id` — Идентификатор активной подписки ([`SubscriptionId`]).
    /// * `pattern` — Удаляемый шаблон топика.
    pub fn remove_subscription_topic(&self, sub_id: router::SubscriptionId, pattern: impl AsRef<str>) {
        self.router.remove_topic(sub_id, pattern.as_ref());
    }

    /// Синхронный запрос-ответ (In-Process Request-Reply RPC)
    ///
    /// Отправляет запрос на топик и асинхронно ожидает ответное сообщение с совпадающим `correlation_id`.
    ///
    /// # Аргументы
    /// * `topic` — Тема целевого сервиса-обработчика.
    /// * `payload` — JSON полезная нагрузка запроса.
    /// * `timeout` — Максимальная продолжительность ожидания ответа.
    ///
    /// # Возвращаемое значение
    /// Ответное сообщение [`EventMessage`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::timeout`], если ответ не был получен в течение заданного времени.
    pub async fn request(
        &self,
        topic: &str,
        payload: serde_json::Value,
        timeout: StdDuration,
    ) -> Result<EventMessage> {
        let correlation_id = Uuid::new_v4();
        let reply_topic = format!("_reply.{}", correlation_id);

        let mut sub = self.subscribe_topic(&reply_topic);

        let mut req_msg = EventMessage::telemetry(topic, "core.rpc", payload);
        req_msg = req_msg.with_correlation(correlation_id, Some(reply_topic));

        self.publish(req_msg).await?;

        let timeout_fut = tokio::time::sleep(timeout);
        tokio::pin!(timeout_fut);

        loop {
            tokio::select! {
                Some(reply) = sub.recv() => {
                    if reply.correlation_id == Some(correlation_id) {
                        return Ok(reply);
                    }
                }
                _ = &mut timeout_fut => {
                    return Err(AppError::timeout(format!("Request to topic '{}' timed out after {:?}", topic, timeout)));
                }
            }
        }
    }

    /// Отправить ответ на входящий RPC-запрос
    ///
    /// Извлекает `reply_to` и `correlation_id` из сообщения запроса и публикует ответ.
    ///
    /// # Аргументы
    /// * `request` — Исходное входящее сообщение запроса ([`EventMessage`]).
    /// * `payload` — JSON данные ответа.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::validation`], если в запросе отсутствует `reply_to` или `correlation_id`.
    pub async fn reply_to(&self, request: &EventMessage, payload: serde_json::Value) -> Result<()> {
        if let (Some(reply_topic), Some(corr_id)) = (&request.reply_to, request.correlation_id) {
            let mut reply_msg = EventMessage::telemetry(reply_topic, "core.rpc", payload);
            reply_msg = reply_msg.with_correlation(corr_id, None);
            self.publish(reply_msg).await?;
            Ok(())
        } else {
            Err(AppError::validation("request", "Missing reply_to or correlation_id in request"))
        }
    }

    /// Запросить историю событий: сначала из L1 RAM RingBuffer, при нехватке — из L2 SQLite
    ///
    /// Обеспечивает прозрачную двухуровневую выборку непросроченных событий.
    ///
    /// # Аргументы
    /// * `topic_filter` — Опциональный префикс темы для фильтрации.
    /// * `limit` — Максимальное количество возвращаемых событий.
    ///
    /// # Возвращаемое значение
    /// Список событий [`Vec<EventMessage>`] в хронологическом порядке.
    pub async fn query_history(
        &self,
        topic_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EventMessage>> {
        // 1. Читаем из горячего L1 кольцевого буфера
        let ram_events = self.ring.query(topic_filter, limit);
        if ram_events.len() >= limit || self.storage.is_none() {
            return Ok(ram_events);
        }

        // 2. Если в памяти меньше limit и подключена БД — дочитываем из L2
        if let Some(ref st) = self.storage {
            let needed = (limit - ram_events.len()) as u32;
            let db_records = st.query(topic_filter, None, needed).await?;

            let mut combined = Vec::with_capacity(ram_events.len() + db_records.len());
            for rec in db_records {
                let payload = serde_json::from_str(&rec.payload_json).unwrap_or(serde_json::Value::Null);
                let msg = EventMessage {
                    id: rec.event_uuid,
                    topic: rec.topic,
                    event_type: EventType::Reliable,
                    priority: aethercore_common::models::events::EventPriority::Normal,
                    source: rec.source,
                    payload,
                    binary_payload: None,
                    timestamp: rec.created_at,
                    expires_at: None,
                    correlation_id: None,
                    reply_to: None,
                };
                combined.push(msg);
            }
            combined.extend(ram_events);
            return Ok(combined);
        }

        Ok(ram_events)
    }

    /// Запросить исторические записи из надежного журнала SQLite
    ///
    /// # Аргументы
    /// * `topic_filter` — Опциональный префикс темы.
    /// * `after_id` — ID последней прочитанной записи для постраничной пагинации (курсор).
    /// * `limit` — Лимит записей в ответе (от 1 до 1000).
    ///
    /// # Возвращаемое значение
    /// Список сохраненных записей журнала [`ReliableEventRecord`].
    pub async fn query_journal(
        &self,
        topic_filter: Option<&str>,
        after_id: Option<i64>,
        limit: u32,
    ) -> Result<Vec<ReliableEventRecord>> {
        if let Some(ref st) = self.storage {
            st.query(topic_filter, after_id, limit).await
        } else {
            Ok(Vec::new())
        }
    }

    /// Выполнить ручную ротацию и очистку устаревших записей журнала SQLite
    ///
    /// # Аргументы
    /// * `max_age` — Опциональный максимальный возраст сохраняемых записей (например, `Duration::days(30)`).
    /// * `max_count` — Опциональное максимальное количество записей в таблице.
    ///
    /// # Возвращаемое значение
    /// Количество удаленных строк из базы данных.
    pub async fn prune_journal(
        &self,
        max_age: Option<Duration>,
        max_count: Option<usize>,
    ) -> Result<u64> {
        if let Some(ref st) = self.storage {
            st.prune(max_age, max_count).await
        } else {
            Ok(0)
        }
    }

    /// Получить снимок метрик и текущего состояния шины событий
    ///
    /// Возвращает счетчики опубликованных событий по приоритетам, число активных подписчиков,
    /// размер буфера памяти и количество сброшенных сообщений.
    pub fn stats(&self) -> BusStats {
        self.metrics
            .0
            .snapshot(self.router.subscriber_count(), self.ring.len())
    }
}
