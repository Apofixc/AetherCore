//! # Высокопроизводительная гибридная шина событий платформы (EventBus)
//!
//! Предоставляет:
//! - **Двухуровневое хранилище**: ультрабыстрый L1 In-Memory кольцевой буфер ([`EventRingBuffer`]) для нулевого I/O в UI + надежное L2 SQLite хранилище ([`EventStorage`]) с коротким TTL и автоочисткой.
//! - **Кэш последних состояний**: LRU-хранилище последних значений топиков ([`RetainedStore`]) с защитой от шторма событий и поддержкой безопасной отдачи по явному запросу.
//! - **Маршрутизация топиков**: префиксное дерево ([`TopicRouter`]) с поддержкой MQTT-подобных масок `*` (один сегмент) и `#` (произвольный хвост топика).
//! - **Контентная фильтрация**: поддержка пользовательских предикатов [`EventFilter`] на уровне подписки для отсечения лишнего трафика до очередей.
//! - **Жизненный цикл подписок**: RAII-дескрипторы ([`SubscriptionHandle`]) с автоматической отпиской при выходе из области видимости (`Drop`) и динамическим управлением темами на лету (`add_topic` / `remove_topic`).
//! - **Приоритеты и защита от голодания**: взвешенные очереди Weighted Fair Queuing ([`PriorityQueueSender`]) с квотами 8:4:2:1, гарантирующие доставку низкоприоритетных событий при шторме алармов.
//! - **Устойчивость и дедупликация**: фильтрация повторов по UUID и бизнес-ключам ([`EventDeduplicator`]) и конвейер перехватчиков ([`InterceptorPipeline`]).
//! - **In-Process Request-Reply & Scatter-Gather RPC**: синхронизированный вызов команд точка-точка (`request`) и параллельный опрос группы сервисов (`request_many`).

pub mod dedup;
pub mod dlq;
pub mod interceptor;
pub mod queue;
pub mod retained;
pub mod ring;
pub mod router;
pub mod stats;
pub mod storage;
pub mod topology;

pub use dedup::EventDeduplicator;
pub use dlq::{DeadLetter, DeadLetterQueue, DeadLetterReason, DEFAULT_DLQ_CAPACITY};
pub use interceptor::{EventInterceptor, InterceptorAction, InterceptorPipeline, MaskingInterceptor};
pub use queue::{create_priority_queue, PriorityQueueReceiver, PriorityQueueSender};
pub use retained::{RetainedStore, DEFAULT_RETAINED_CAPACITY};
pub use ring::EventRingBuffer;
pub use router::{DispatchResult, EventFilter, SubscriptionHandle, SubscriptionId, TopicRouter};
pub use stats::{BusMetrics, BusStats};
pub use storage::EventStorage;
pub use topology::{BusTopologySnapshot, BusTopologyTracker, TopologyEdge, TopologyNode, TopologyNodeType};

use crate::db::Db;
use aethercore_common::error::{AppError, Result};
use aethercore_common::models::events::{EventMessage, EventType, ReliableEventRecord};
use chrono::Duration;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tracing::{debug, error, trace};
use uuid::Uuid;

/// Экземпляр гибридной шины событий платформы
///
/// Объединяет оперативную память L1 (RingBuffer) для горячей телеметрии, персистентную базу данных L2 (SQLite)
/// для надежных событий, LRU-кэш Retained-сообщений, взвешенные очереди приоритетов (WFQ),
/// префиксный маршрутизатор топиков, трекер топологии взаимодействия модулей, очередь DLQ и средства двунаправленного RPC.
#[derive(Clone, Debug)]
pub struct EventBus {
    router: TopicRouter,
    queue_tx: PriorityQueueSender,
    ring: EventRingBuffer,
    dedup: EventDeduplicator,
    retained: RetainedStore,
    interceptors: InterceptorPipeline,
    storage: Option<EventStorage>,
    metrics: BusMetrics,
    topology: BusTopologyTracker,
    dlq: DeadLetterQueue,
}

impl EventBus {
    /// Инициализировать шину событий с подключением к базе данных SQLite для L2 хранилища
    ///
    /// Создает префиксный роутер, L1 RingBuffer на 2048 событий, очередь приоритетов WFQ,
    /// кэш Retained-сообщений и запускает асинхронный воркер записи в базу данных с микро-батчингом.
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
    pub fn in_memory() -> Self {
        Self::with_options(None, ring::DEFAULT_RING_CAPACITY)
    }

    /// Инициализировать шину событий с точной настройкой хранилища и емкости L1 кэша
    ///
    /// # Аргументы
    /// * `db` — Опциональный пул базы данных для L2 хранилища.
    /// * `ring_capacity` — Емкость горячего L1 кольцевого буфера в оперативной памяти (в количестве сообщений).
    pub fn with_options(db: Option<Db>, ring_capacity: usize) -> Self {
        Self::with_full_config(db, None, ring_capacity)
    }

    /// Инициализировать шину событий с пользовательской конфигурацией персистентного хранилища L2
    ///
    /// # Аргументы
    /// * `db` — Пул базы данных SQLite.
    /// * `storage_config` — Настройки емкости журнала и возраста автоочистки ([`storage::EventStorageConfig`]).
    /// * `ring_capacity` — Емкость буфера оперативной памяти L1.
    pub fn with_storage_config(
        db: Db,
        storage_config: storage::EventStorageConfig,
        ring_capacity: usize,
    ) -> Self {
        Self::with_full_config(Some(db), Some(storage_config), ring_capacity)
    }

    /// Полный конструктор шины событий со всеми параметрами конфигурации
    pub fn with_full_config(
        db: Option<Db>,
        storage_config: Option<storage::EventStorageConfig>,
        ring_capacity: usize,
    ) -> Self {
        let topology = BusTopologyTracker::new();
        let dlq = DeadLetterQueue::default();
        let router = TopicRouter::new().with_topology(topology.clone());
        let (queue_tx, mut queue_rx) = create_priority_queue();
        let ring = EventRingBuffer::new(ring_capacity);
        let dedup = EventDeduplicator::default();
        let retained = RetainedStore::default();
        let interceptors = InterceptorPipeline::new();
        let storage = db.map(|d| match storage_config {
            Some(cfg) => EventStorage::with_config(d, cfg),
            None => EventStorage::new(d),
        });
        let metrics = BusMetrics::default();

        let bus = Self {
            router: router.clone(),
            queue_tx,
            ring: ring.clone(),
            dedup: dedup.clone(),
            retained: retained.clone(),
            interceptors: interceptors.clone(),
            storage: storage.clone(),
            metrics: metrics.clone(),
            topology: topology.clone(),
            dlq: dlq.clone(),
        };

        // Запуск фонового диспетчера взвешенных очередей
        let dispatch_router = router.clone();
        let dispatch_ring = ring.clone();
        let dispatch_storage = storage.clone();
        let dispatch_metrics = metrics.clone();
        let dispatch_topology = topology.clone();
        let dispatch_dlq = dlq.clone();
        let dispatch_retained = retained.clone();

        tokio::spawn(async move {
            debug!("EventBus weighted fair queuing dispatcher started");
            while let Some(event) = queue_rx.dequeue().await {
                let start = std::time::Instant::now();

                // 0. Проверяем TTL сообщения перед любой обработкой и доставкой
                if event.is_expired() {
                    trace!("Event '{}' expired (TTL), moved to DLQ", event.topic);
                    dispatch_dlq.push(event.clone(), DeadLetterReason::Expired);
                    continue;
                }

                // 1. Фиксируем активность в топологии
                dispatch_topology.record_publish(&event.source, &event.topic);

                // 2. Сохраняем в горячий L1 кольцевой буфер
                dispatch_ring.push(event.clone());

                // 3. Сохраняем Retained-состояние
                if event.retain {
                    dispatch_retained.put(event.clone());
                }

                // 4. Если событие типа Reliable — ставим в очередь сохранения L2 (non-blocking)
                if event.event_type == EventType::Reliable {
                    if let Some(ref st) = dispatch_storage {
                        if let Err(e) = st.try_persist(event.clone()) {
                            error!("Failed to enqueue reliable event to L2 storage: {}", e);
                        }
                    }
                }

                // 5. Доставляем всем подходящим подписчикам через TopicRouter
                let res = dispatch_router.dispatch(&event);
                trace!(
                    "Dispatched event '{}' to {} subscribers (dropped {})",
                    event.topic, res.delivered, res.dropped
                );

                dispatch_metrics.0.record_published(event.priority);
                dispatch_metrics.0.record_dropped_count(res.dropped);
                dispatch_metrics.0.record_dispatch_latency(start.elapsed());
            }
            debug!("EventBus dispatcher terminated");
        });

        bus
    }

    /// Опубликовать событие в шину
    ///
    /// Атомарно проверяет и резервирует событие в дедупликаторе (защита от TOCTOU-гонок),
    /// выполняет цепочку перехватчиков `pre_publish` и ставит сообщение в очередь диспетчера.
    /// При ошибке интерцептора или очереди бронь дедупликатора автоматически снимается для повторных попыток (retry).
    ///
    /// # Аргументы
    /// * `event` — Публикуемое событие платформы ([`EventMessage`]).
    ///
    /// # Ошибки
    /// Возвращает [`AppError`], если перехватчик отклонил публикацию или очередь переполнена/закрыта.
    pub async fn publish(&self, mut event: EventMessage) -> Result<()> {
        // 1. Атомарная проверка и резервация в окне дедупликатора под локом (без TOCTOU)
        if self.dedup.is_duplicate_event_or_record(&event) {
            trace!("Ignoring duplicate event {}", event.id);
            return Ok(());
        }

        // 2. Выполнение конвейера перехватчиков (pre-publish)
        let action = match self.interceptors.execute_pre(&mut event).await {
            Ok(act) => act,
            Err(err) => {
                // При ошибке выполнения перехватчика откатываем бронь дедупликатора
                self.dedup.remove_event(&event);
                return Err(err);
            }
        };

        match action {
            InterceptorAction::Continue => {}
            InterceptorAction::DropSilently => return Ok(()),
            InterceptorAction::Reject(err) => {
                // При отклонении перехватчиком откатываем бронь для возможности retry
                self.dedup.remove_event(&event);
                return Err(err);
            }
        }

        // 3. Постановка во взвешенную очередь диспетчера
        if let Err(err) = self.queue_tx.enqueue(event.clone()).await {
            // При сбое постановки в очередь откатываем бронь
            self.dedup.remove_event(&event);
            return Err(err);
        }

        // 4. Выполнение конвейера перехватчиков (post-publish)
        self.interceptors.execute_post(&event).await;

        Ok(())
    }

    /// Опубликовать типизированное эфемеренное событие телеметрии
    ///
    /// # Аргументы
    /// * `topic` — Тема события.
    /// * `source` — Идентификатор источника (например, `"plugin:zigbee"`).
    /// * `payload` — Сериализуемая ссылка на данные.
    pub async fn publish_typed<T: Serialize>(
        &self,
        topic: impl Into<String>,
        source: impl Into<String>,
        payload: &T,
    ) -> Result<Uuid> {
        let val = serde_json::to_value(payload)
            .map_err(|e| AppError::validation("payload", format!("Serialization error: {}", e)))?;
        let event = EventMessage::telemetry(topic, source, val);
        let id = event.id;
        self.publish(event).await?;
        Ok(id)
    }

    /// Опубликовать типизированное гарантированное персистентное событие
    ///
    /// # Аргументы
    /// * `topic` — Тема события.
    /// * `source` — Идентификатор источника.
    /// * `payload` — Сериализуемая ссылка на данные.
    pub async fn publish_reliable_typed<T: Serialize>(
        &self,
        topic: impl Into<String>,
        source: impl Into<String>,
        payload: &T,
    ) -> Result<Uuid> {
        let val = serde_json::to_value(payload)
            .map_err(|e| AppError::validation("payload", format!("Serialization error: {}", e)))?;
        let event = EventMessage::reliable(topic, source, val);
        let id = event.id;
        self.publish(event).await?;
        Ok(id)
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
    pub fn subscribe(&self) -> SubscriptionHandle {
        self.router.subscribe(&["#"])
    }

    /// Создать именованную подписку (удобно для регистрации имени плагина в топологии)
    ///
    /// # Аргументы
    /// * `name` — Отображаемое имя плагина/сервиса (например, `"plugin:notifications"`).
    /// * `patterns` — Срез шаблонов топиков.
    pub fn subscribe_named(
        &self,
        name: impl Into<String>,
        patterns: &[&str],
    ) -> SubscriptionHandle {
        let sub = self.subscribe_topics(patterns);
        sub.with_name(name)
    }

    /// Подписаться на конкретный топик или маску (`*`, `#`)
    ///
    /// # Аргументы
    /// * `pattern` — Шаблон топика (например, `"devices.*.status"` или `"alarms.#"`).
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

    /// Подписаться на топики с предикатом контентной фильтрации
    ///
    /// Позволяет отсекать события по содержимому payload до попадания в очередь подписчика.
    ///
    /// # Аргументы
    /// * `patterns` — Срез шаблонов топиков.
    /// * `predicate` — Замыкание или функция фильтрации `Fn(&EventMessage) -> bool`.
    pub fn subscribe_filtered<F>(&self, patterns: &[&str], predicate: F) -> SubscriptionHandle
    where
        F: Fn(&EventMessage) -> bool + Send + Sync + 'static,
    {
        self.router
            .subscribe_filtered(patterns, Some(Arc::new(predicate)))
    }

    /// Запросить сохраненные Retained-сообщения по топику или шаблону с ограничением количества
    ///
    /// # Аргументы
    /// * `pattern` — Топик или маска (`*`, `#`).
    /// * `limit` — Максимальное количество возвращаемых сообщений.
    pub fn get_retained(&self, pattern: &str, limit: usize) -> Vec<EventMessage> {
        if limit == 0 {
            return Vec::new();
        }
        self.retained.get_matching(pattern, limit)
    }

    /// Создать подписку на топик с получением предварительно сохраненных Retained-состояний
    ///
    /// # Гарантии и семантика
    /// Подписка активируется **до** выборки сохраненных сообщений, что исключает потерю промежуточных
    /// событий между моментом снимка и подпиской.
    ///
    /// **Важно**: сообщения, поступившие в шину во время выполнения этой функции, могут одновременно
    /// присутствовать и в возвращаемом срезе `Vec<EventMessage>`, и в канале `SubscriptionHandle`.
    /// Потребитель должен дедуплицировать начальные сообщения по их `event.id` (UUID).
    ///
    /// # Аргументы
    /// * `pattern` — Шаблон топика для подписки.
    /// * `max_initial_retained` — Максимальное количество сохраненных сообщений для начального состояния.
    ///
    /// # Возвращаемое значение
    /// Кортеж из RAII-подписки [`SubscriptionHandle`] и вектора сохраненных сообщений [`Vec<EventMessage>`].
    pub fn subscribe_with_retained(
        &self,
        pattern: impl AsRef<str>,
        max_initial_retained: usize,
    ) -> (SubscriptionHandle, Vec<EventMessage>) {
        let pat = pattern.as_ref();
        // 1. Сначала создаем подписку, чтобы не потерять сообщения, пришедшие во время выборки
        let sub = self.subscribe_topic(pat);
        // 2. Затем считываем накопленный снимок
        let retained_msgs = self.retained.get_matching(pat, max_initial_retained);
        (sub, retained_msgs)
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
    /// При таймауте фиксирует запрос в очереди DLQ для отладки сбоев сервисов.
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

        self.publish(req_msg.clone()).await?;

        let timeout_fut = tokio::time::sleep(timeout);
        tokio::pin!(timeout_fut);

        loop {
            tokio::select! {
                recv_res = sub.recv() => {
                    match recv_res {
                        Some(reply) => {
                            if reply.correlation_id == Some(correlation_id) {
                                return Ok(reply);
                            }
                        }
                        None => {
                            return Err(AppError::internal(format!(
                                "Subscription channel closed while waiting for RPC response on topic '{}'",
                                topic
                            )));
                        }
                    }
                }
                _ = &mut timeout_fut => {
                    self.dlq.push(req_msg.clone(), DeadLetterReason::RpcTimeout);
                    return Err(AppError::timeout(format!("Request to topic '{}' timed out after {:?}", topic, timeout)));
                }
            }
        }
    }

    /// Асинхронный групповой запрос-ответ (Scatter-Gather RPC)
    ///
    /// Отправляет запрос на топик и ожидает ответы от нескольких обработчиков.
    /// Завершается при получении `expected_count` ответов либо по истечении `timeout`.
    ///
    /// # Аргументы
    /// * `topic` — Тема целевого сервиса или группы сервисов.
    /// * `payload` — JSON полезная нагрузка запроса.
    /// * `timeout` — Максимальное время ожидания ответов.
    /// * `expected_count` — Количество ожидаемых ответов до досрочного завершения.
    ///
    /// # Возвращаемое значение
    /// Список собранных ответов [`Vec<EventMessage>`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::timeout`], если за отведенное время не было получено ни одного ответа.
    pub async fn request_many(
        &self,
        topic: &str,
        payload: serde_json::Value,
        timeout: StdDuration,
        expected_count: usize,
    ) -> Result<Vec<EventMessage>> {
        if expected_count == 0 {
            return Ok(Vec::new());
        }

        let correlation_id = Uuid::new_v4();
        let reply_topic = format!("_reply.{}", correlation_id);

        let mut sub = self.subscribe_topic(&reply_topic);

        let mut req_msg = EventMessage::telemetry(topic, "core.rpc", payload);
        req_msg = req_msg.with_correlation(correlation_id, Some(reply_topic));

        self.publish(req_msg.clone()).await?;

        let mut responses = Vec::with_capacity(expected_count);
        let timeout_fut = tokio::time::sleep(timeout);
        tokio::pin!(timeout_fut);

        loop {
            tokio::select! {
                recv_res = sub.recv() => {
                    match recv_res {
                        Some(reply) => {
                            if reply.correlation_id == Some(correlation_id) {
                                responses.push(reply);
                                if responses.len() >= expected_count {
                                    return Ok(responses);
                                }
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
                _ = &mut timeout_fut => {
                    break;
                }
            }
        }

        if responses.is_empty() {
            self.dlq.push(req_msg, DeadLetterReason::RpcTimeout);
            Err(AppError::timeout(format!(
                "Scatter-gather request to topic '{}' timed out with 0 responses after {:?}",
                topic, timeout
            )))
        } else {
            Ok(responses)
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
    /// # Ограничения архитектуры (Архитектурный компромисс)
    /// Таблица L2 `event_journal` хранит облегченную схему записей (`id, uuid, topic, source, payload_json, created_at`).
    /// При реконструкции исторических событий из БД поля `priority` (`Normal`), `dedup_key` (`None`), `correlation_id` (`None`)
    /// заполняются значениями по умолчанию.
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
        if limit == 0 {
            return Ok(Vec::new());
        }

        // 1. Читаем из горячего L1 кольцевого буфера
        let ram_events = self.ring.query(topic_filter, limit);
        if ram_events.len() >= limit || self.storage.is_none() {
            return Ok(ram_events);
        }

        // 2. Если в памяти меньше limit и подключена БД — дочитываем из L2
        if let Some(ref st) = self.storage {
            let needed = limit.saturating_sub(ram_events.len());
            // ponytail: эвристика выборки с запасом ((needed + ram) * 2).min(1000) компенсирует
            // возможное пересечение свежих записей в БД с уже полученными из L1 RAM буфера
            let query_limit = ((needed + ram_events.len()) * 2).min(1000) as u32;
            let db_records = st.query_recent(topic_filter, query_limit).await?;

            let ram_ids: std::collections::HashSet<_> = ram_events.iter().map(|e| e.id).collect();

            let mut db_events = Vec::new();
            for rec in db_records {
                if ram_ids.contains(&rec.event_uuid) {
                    continue; // Пропускаем дубликат, который еще присутствует в L1 RAM
                }
                let payload = serde_json::from_str(&rec.payload_json).unwrap_or(serde_json::Value::Null);
                let msg = EventMessage {
                    id: rec.event_uuid,
                    topic: rec.topic,
                    event_type: EventType::Reliable,
                    priority: aethercore_common::models::events::EventPriority::Normal,
                    source: rec.source,
                    payload,
                    binary_payload: None,
                    dedup_key: None,
                    retain: false,
                    timestamp: rec.created_at,
                    expires_at: None,
                    correlation_id: None,
                    reply_to: None,
                };
                db_events.push(msg);
            }

            // Берем только недостающее количество с конца db_events
            let take_db_count = limit.saturating_sub(ram_events.len());
            let db_skip = db_events.len().saturating_sub(take_db_count);
            let mut combined: Vec<EventMessage> = db_events.into_iter().skip(db_skip).collect();
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
        if limit == 0 {
            return Ok(Vec::new());
        }

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

    /// Получить моментальный снимок графа топологии шины для визуализатора
    pub fn topology(&self) -> BusTopologySnapshot {
        self.topology.snapshot()
    }

    /// Получить ссылку на трекер топологии
    pub fn topology_tracker(&self) -> BusTopologyTracker {
        self.topology.clone()
    }

    /// Получить список сбойных сообщений из очереди Dead Letter Queue
    pub fn dead_letters(&self, limit: usize) -> Vec<DeadLetter> {
        self.dlq.list(limit)
    }

    /// Получить конкретную запись из DLQ по ее уникальному идентификатору
    pub fn get_dead_letter(&self, id: Uuid) -> Option<DeadLetter> {
        self.dlq.get(id)
    }

    /// Повторно отправить (re-drive) сбойное сообщение из DLQ обратно в шину
    pub async fn redrive_dead_letter(&self, id: Uuid) -> Result<()> {
        if let Some(mut dead_letter) = self.dlq.remove(id) {
            // Обновляем идентификатор, временную метку и сбрасываем TTL для успешного прохождения
            dead_letter.event.id = Uuid::new_v4();
            dead_letter.event.timestamp = chrono::Utc::now();
            dead_letter.event.expires_at = None;
            self.publish(dead_letter.event).await
        } else {
            Err(AppError::not_found("Dead letter entry not found"))
        }
    }

    /// Очистить очередь недоставленных сообщений
    pub fn clear_dead_letters(&self) {
        self.dlq.clear();
    }

    /// Зарегистрировать пользовательский перехватчик событий в конвейере шины
    pub fn add_interceptor(&mut self, interceptor: Arc<dyn EventInterceptor>) {
        self.interceptors.register(interceptor);
    }

    /// Получить снимок метрик и текущего состояния шины событий
    ///
    /// Возвращает счетчики опубликованных событий по приоритетам, число активных подписчиков,
    /// размер буфера памяти, количество retained-сообщений, размер DLQ и показатели задержки.
    pub fn stats(&self) -> BusStats {
        let top_snap = self.topology.snapshot();
        self.metrics.0.snapshot(
            self.router.subscriber_count(),
            self.ring.len(),
            self.retained.len(),
            self.dlq.len(),
            top_snap.publishers_count,
            top_snap.topics_count,
        )
    }
}
