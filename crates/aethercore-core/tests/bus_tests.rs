//! # Тесты расширенной шины событий EventBus

use aethercore_common::models::events::{EventMessage, EventPriority};
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use std::time::Duration;

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

    // Даем микропаузу воркеру на батч-запись в БД
    tokio::time::sleep(Duration::from_millis(80)).await;

    // 3. Читаем из персистентного журнала
    let journal = bus.query_journal(Some("user."), None, 10).await.unwrap();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].topic, "user.created");
}

#[tokio::test]
async fn test_topic_router_wildcards_and_dynamic_topics() {
    let bus = EventBus::in_memory();

    // Подписка с одиночным wildcard '*'
    let mut sub_single = bus.subscribe_topic("devices.*.status");
    // Подписка с многоуровневым wildcard '#'
    let mut sub_multi = bus.subscribe_topic("sensors.#");

    // 1. Отправляем событие для sub_single
    let ev1 = EventMessage::telemetry("devices.switch1.status", "core", serde_json::json!({"online": true}));
    bus.publish(ev1).await.unwrap();

    let rec1 = sub_single.recv().await.unwrap();
    assert_eq!(rec1.topic, "devices.switch1.status");

    // 2. Отправляем событие для sub_multi
    let ev2 = EventMessage::telemetry("sensors.buildingA.room1.temp", "core", serde_json::json!({"val": 23.5}));
    bus.publish(ev2).await.unwrap();

    let rec2 = sub_multi.recv().await.unwrap();
    assert_eq!(rec2.topic, "sensors.buildingA.room1.temp");

    // 3. Динамически добавляем топик к sub_single
    sub_single.add_topic("alarms.fire");
    let ev3 = EventMessage::telemetry("alarms.fire", "core", serde_json::json!({"zone": 4}));
    bus.publish(ev3).await.unwrap();

    let rec3 = sub_single.recv().await.unwrap();
    assert_eq!(rec3.topic, "alarms.fire");

    // 4. Динамически удаляем топик из sub_single
    sub_single.remove_topic("alarms.fire");
    let ev4 = EventMessage::telemetry("alarms.fire", "core", serde_json::json!({"zone": 5}));
    bus.publish(ev4).await.unwrap();

    // sub_single не должен получить ev4
    tokio::select! {
        _ = sub_single.recv() => panic!("Should not receive removed topic"),
        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
    }
}

#[tokio::test]
async fn test_priority_weighted_fair_queuing() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe();

    // Публикуем 16 Critical событий и 2 Low события
    for i in 0..16 {
        let msg = EventMessage::telemetry("crit", "core", serde_json::json!({"idx": i}))
            .with_priority(EventPriority::Critical);
        bus.publish(msg).await.unwrap();
    }

    for i in 0..2 {
        let msg = EventMessage::telemetry("low", "core", serde_json::json!({"idx": i}))
            .with_priority(EventPriority::Low);
        bus.publish(msg).await.unwrap();
    }

    // Вычитываем сообщения: благодаря WFQ (8 Critical : 1 Low), Low событие появится до того,
    // как закончатся все 16 Critical событий!
    let mut topics_received = Vec::new();
    for _ in 0..18 {
        if let Some(msg) = sub.recv().await {
            topics_received.push(msg.topic);
        }
    }

    assert_eq!(topics_received.len(), 18);
    // Проверяем, что первое Low событие получено не в самом конце, а внутри первой десятки
    let first_low_pos = topics_received.iter().position(|t| t == "low").unwrap();
    assert!(first_low_pos <= 10, "Low event position: {}", first_low_pos);
}

#[tokio::test]
async fn test_deduplication() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe();

    let msg = EventMessage::telemetry("ping.tick", "core", serde_json::json!({"val": 1}));

    // Публикуем одно и то же сообщение 5 раз
    for _ in 0..5 {
        bus.publish(msg.clone()).await.unwrap();
    }

    // Должно быть доставлено ровно 1 сообщение
    let rec = sub.recv().await.unwrap();
    assert_eq!(rec.id, msg.id);

    tokio::select! {
        _ = sub.recv() => panic!("Duplicate event was delivered!"),
        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
    }
}

#[tokio::test]
async fn test_request_reply_rpc() {
    let bus = EventBus::in_memory();

    // Эмулируем сервис-обработчик запросов
    let server_bus = bus.clone();
    let mut server_sub = server_bus.subscribe_topic("service.echo");

    tokio::spawn(async move {
        while let Some(req) = server_sub.recv().await {
            let echo_val = req.payload.get("msg").unwrap().clone();
            let _ = server_bus.reply_to(&req, serde_json::json!({"reply": echo_val})).await;
        }
    });

    // Клиент отправляет RPC-запрос
    let reply = bus
        .request(
            "service.echo",
            serde_json::json!({"msg": "Hello AetherCore"}),
            Duration::from_secs(1),
        )
        .await
        .unwrap();

    assert_eq!(reply.payload.get("reply").unwrap(), "Hello AetherCore");
}

#[tokio::test]
async fn test_default_bus_does_not_mutate_payload() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe();

    let msg = EventMessage::telemetry(
        "auth.login",
        "auth",
        serde_json::json!({
            "username": "admin",
            "password": "super_secret_password",
            "token": "secret_jwt_token"
        }),
    );

    bus.publish(msg).await.unwrap();

    let rec = sub.recv().await.unwrap();
    assert_eq!(rec.payload.get("username").unwrap(), "admin");
    assert_eq!(rec.payload.get("password").unwrap(), "super_secret_password");
    assert_eq!(rec.payload.get("token").unwrap(), "secret_jwt_token");
}

#[tokio::test]
async fn test_masking_interceptor_explicit() {
    use aethercore_core::bus::MaskingInterceptor;
    use std::sync::Arc;

    let mut bus = EventBus::in_memory();
    bus.add_interceptor(Arc::new(MaskingInterceptor::default()));
    let mut sub = bus.subscribe();

    let msg = EventMessage::telemetry(
        "auth.login",
        "auth",
        serde_json::json!({
            "username": "admin",
            "password": "super_secret_password",
            "token": "secret_jwt_token"
        }),
    );

    bus.publish(msg).await.unwrap();

    let rec = sub.recv().await.unwrap();
    assert_eq!(rec.payload.get("username").unwrap(), "admin");
    assert_eq!(rec.payload.get("password").unwrap(), "***");
    assert_eq!(rec.payload.get("token").unwrap(), "***");
}

#[tokio::test]
async fn test_l1_ring_buffer_and_stats() {
    let bus = EventBus::in_memory();

    for i in 0..10 {
        let msg = EventMessage::telemetry("metric.cpu", "monitor", serde_json::json!({"usage": i}));
        bus.publish(msg).await.unwrap();
    }

    // Даем микросекунды диспетчеру на обработку
    tokio::time::sleep(Duration::from_millis(50)).await;

    let history = bus.query_history(Some("metric."), 5).await.unwrap();
    assert_eq!(history.len(), 5);

    let stats = bus.stats();
    assert_eq!(stats.published_total, 10);
    assert_eq!(stats.ring_buffer_len, 10);
}

#[tokio::test]
async fn test_predicate_filter() {
    let bus = EventBus::in_memory();

    // Подписка только на критические алармы с zone == 1
    let mut filtered_sub = bus
        .subscribe_topic("alarms.*")
        .with_filter(|ev| {
            ev.payload.get("zone").and_then(|z| z.as_i64()) == Some(1)
        });

    // 1. Сообщение для zone 2 — должно быть отфильтровано
    let ev_zone2 = EventMessage::telemetry(
        "alarms.fire",
        "sensor-1",
        serde_json::json!({"zone": 2}),
    );
    bus.publish(ev_zone2).await.unwrap();

    // 2. Сообщение для zone 1 — должно пройти
    let ev_zone1 = EventMessage::telemetry(
        "alarms.fire",
        "sensor-2",
        serde_json::json!({"zone": 1}),
    );
    bus.publish(ev_zone1).await.unwrap();

    let received = filtered_sub.recv().await.unwrap();
    assert_eq!(received.payload.get("zone").unwrap(), 1);
    assert_eq!(received.source, "sensor-2");
}

#[tokio::test]
async fn test_dedup_by_business_key() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe();

    // Отправляем два разных сообщения (разные UUID), но с одинаковым business dedup_key
    let msg1 = EventMessage::telemetry(
        "devices.sw1.alarm",
        "sw1",
        serde_json::json!({"state": "down"}),
    )
    .with_dedup_key("link_down:sw1:port0");

    let msg2 = EventMessage::telemetry(
        "devices.sw1.alarm",
        "sw1",
        serde_json::json!({"state": "down_repeated"}),
    )
    .with_dedup_key("link_down:sw1:port0");

    bus.publish(msg1).await.unwrap();
    bus.publish(msg2).await.unwrap();

    let received = sub.recv().await.unwrap();
    assert_eq!(received.payload.get("state").unwrap(), "down");

    // Второе сообщение должно быть отброшено дедупликатором
    tokio::select! {
        _ = sub.recv() => panic!("Duplicate message should have been dropped!"),
        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
    }
}

#[tokio::test]
async fn test_retained_messages_and_safe_subscription() {
    let bus = EventBus::in_memory();

    // Публикуем Retained-сообщения
    let msg_dev1 = EventMessage::telemetry(
        "devices.switch1.state",
        "agent",
        serde_json::json!({"status": "online", "ports": 24}),
    )
    .with_retain(true);

    let msg_dev2 = EventMessage::telemetry(
        "devices.switch2.state",
        "agent",
        serde_json::json!({"status": "offline"}),
    )
    .with_retain(true);

    // Обычное сообщение без retain
    let msg_temp = EventMessage::telemetry(
        "devices.switch1.temp",
        "agent",
        serde_json::json!({"temp": 45}),
    )
    .with_retain(false);

    bus.publish(msg_dev1).await.unwrap();
    bus.publish(msg_dev2).await.unwrap();
    bus.publish(msg_temp).await.unwrap();

    // Даем микропаузу воркеру на обработку опубликованных сообщений
    tokio::time::sleep(Duration::from_millis(30)).await;

    // 1. Проверяем get_retained
    let retained_sw1 = bus.get_retained("devices.switch1.state", 10);
    assert_eq!(retained_sw1.len(), 1);
    assert_eq!(retained_sw1[0].payload.get("status").unwrap(), "online");

    let retained_all_devs = bus.get_retained("devices.*.state", 10);
    assert_eq!(retained_all_devs.len(), 2);

    // 2. Безопасная подписка с получением сохраненного состояния
    let (mut sub, initial) = bus.subscribe_with_retained("devices.switch1.state", 5);
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].payload.get("ports").unwrap(), 24);

    // Последующее live-событие доставляется как обычно
    let live_ev = EventMessage::telemetry(
        "devices.switch1.state",
        "agent",
        serde_json::json!({"status": "online", "ports": 48}),
    );
    bus.publish(live_ev).await.unwrap();

    let live_rec = sub.recv().await.unwrap();
    assert_eq!(live_rec.payload.get("ports").unwrap(), 48);

    assert_eq!(bus.stats().retained_messages_len, 2);
}

#[tokio::test]
async fn test_scatter_gather_rpc() {
    let bus = EventBus::in_memory();

    // Эмулируем три обработчика микросервисов
    for i in 1..=3 {
        let bus_clone = bus.clone();
        let mut sub = bus_clone.subscribe_topic("cluster.status");
        tokio::spawn(async move {
            if let Some(req) = sub.recv().await {
                let _ = bus_clone
                    .reply_to(
                        &req,
                        serde_json::json!({"node_id": i, "status": "healthy"}),
                    )
                    .await;
            }
        });
    }

    // Делаем Scatter-Gather RPC запрос с ожиданием 3 ответов
    let responses = bus
        .request_many(
            "cluster.status",
            serde_json::json!({"ping": true}),
            Duration::from_secs(1),
            3,
        )
        .await
        .unwrap();

    assert_eq!(responses.len(), 3);
}

#[tokio::test]
async fn test_bus_topology_tracking() {
    let bus = EventBus::in_memory();

    // 1. Создаем именованную подписку
    let mut _sub = bus.subscribe_named("plugin:notifications", &["scheduler.#", "alarms.#"]);

    // 2. Публикуем события из разных источников
    let ev1 = EventMessage::telemetry("scheduler.task.started", "core:scheduler", serde_json::json!({"task": "backup"}));
    let ev2 = EventMessage::telemetry("sensors.temp", "plugin:zigbee", serde_json::json!({"val": 21.5}));
    let ev3 = EventMessage::telemetry("sensors.temp", "plugin:zigbee", serde_json::json!({"val": 22.0}));

    bus.publish(ev1).await.unwrap();
    bus.publish(ev2).await.unwrap();
    bus.publish(ev3).await.unwrap();

    // Даем микропаузу воркеру на диспетчеризацию
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 3. Получаем снимок топологии
    let topology = bus.topology();
    assert_eq!(topology.publishers_count, 2); // core:scheduler и plugin:zigbee
    assert_eq!(topology.subscribers_count, 1); // plugin:notifications
    assert!(topology.topics_count >= 2); // scheduler.task.started и sensors.temp

    // Проверяем наличие узлов
    let pub_nodes: Vec<_> = topology.nodes.iter().filter(|n| n.node_type == aethercore_core::bus::TopologyNodeType::Publisher).collect();
    assert_eq!(pub_nodes.len(), 2);

    let sub_nodes: Vec<_> = topology.nodes.iter().filter(|n| n.node_type == aethercore_core::bus::TopologyNodeType::Subscriber).collect();
    assert_eq!(sub_nodes.len(), 1);
    assert_eq!(sub_nodes[0].label, "plugin:notifications");

    // Проверяем ребра публикации
    let zigbee_edge = topology.edges.iter().find(|e| e.source_id == "pub:plugin:zigbee" && e.target_id == "topic:sensors.temp").unwrap();
    assert_eq!(zigbee_edge.message_count, 2);
}

#[tokio::test]
async fn test_subscription_throttle() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe_topic("sensors.high_freq");

    // Отправляем 5 сообщений без задержки
    for i in 0..5 {
        let msg = EventMessage::telemetry(
            "sensors.high_freq",
            "plugin:sensor",
            serde_json::json!({"seq": i}),
        );
        bus.publish(msg).await.unwrap();
    }

    // Первое сообщение читается сразу
    let first = sub.recv_throttled(Duration::from_millis(100)).await.unwrap();
    assert_eq!(first.payload.get("seq").unwrap(), 0);

    // Поскольку остальные сообщения пришли с интервалом < 100мс, они должны быть отброшены
    tokio::select! {
        _ = sub.recv_throttled(Duration::from_millis(100)) => panic!("Should be throttled"),
        _ = tokio::time::sleep(Duration::from_millis(30)) => {}
    }

    // Через 110мс отправляем еще одно — оно должно успешно пройти
    tokio::time::sleep(Duration::from_millis(110)).await;
    let next_msg = EventMessage::telemetry(
        "sensors.high_freq",
        "plugin:sensor",
        serde_json::json!({"seq": 99}),
    );
    bus.publish(next_msg).await.unwrap();

    let passed = sub.recv_throttled(Duration::from_millis(100)).await.unwrap();
    assert_eq!(passed.payload.get("seq").unwrap(), 99);
}

#[tokio::test]
async fn test_subscription_debounce() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe_topic("controls.dimmer");

    // Эмулируем быстрое вращение диммера (серия быстрых обновлений каждые 5 мс)
    let bus_clone = bus.clone();
    tokio::spawn(async move {
        for val in 1..=5 {
            let msg = EventMessage::telemetry(
                "controls.dimmer",
                "ui:slider",
                serde_json::json!({"brightness": val * 20}),
            );
            let _ = bus_clone.publish(msg).await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Дебаунс 50 мс должен выдать только финальное стабилизировавшееся значение (100)
    let debounced_msg = sub.recv_debounced(Duration::from_millis(50)).await.unwrap();
    assert_eq!(debounced_msg.payload.get("brightness").unwrap(), 100);
}

#[tokio::test]
async fn test_dead_letter_queue_and_redrive() {
    let bus = EventBus::in_memory();

    // 1. RPC запрос к несуществующему обработчику завершается таймаутом и попадает в DLQ
    let err = bus
        .request("unresponsive.service", serde_json::json!({"action": "ping"}), Duration::from_millis(30))
        .await;
    assert!(err.is_err());

    // 2. Проверяем наличие записи в DLQ
    let dead_letters = bus.dead_letters(10);
    assert_eq!(dead_letters.len(), 1);
    assert_eq!(dead_letters[0].event.topic, "unresponsive.service");
    assert_eq!(
        dead_letters[0].reason,
        aethercore_core::bus::DeadLetterReason::RpcTimeout
    );

    let dlq_id = dead_letters[0].id;

    // 3. Запускаем обработчик топика
    let mut service_sub = bus.subscribe_topic("unresponsive.service");

    // 4. Выполняем повторный запуск (re-drive) из DLQ
    bus.redrive_dead_letter(dlq_id).await.unwrap();

    let redriven = service_sub.recv().await.unwrap();
    assert_eq!(redriven.topic, "unresponsive.service");
    assert_eq!(redriven.payload.get("action").unwrap(), "ping");

    // 5. После redrive запись удалена из DLQ
    assert_eq!(bus.dead_letters(10).len(), 0);
}

#[tokio::test]
async fn test_typed_pub_sub() {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct ClimateTelemetry {
        temperature: f64,
        humidity: f64,
        room: String,
    }

    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe_topic("climate.living_room");

    let payload = ClimateTelemetry {
        temperature: 23.4,
        humidity: 45.0,
        room: "Living Room".to_string(),
    };

    bus.publish_typed("climate.living_room", "plugin:climate", &payload)
        .await
        .unwrap();

    let received: ClimateTelemetry = sub.recv_typed::<ClimateTelemetry>().await.unwrap().unwrap();
    assert_eq!(received, payload);
}

#[tokio::test]
async fn test_l2_eviction_double_persist_safety() {
    let db = Db::init_in_memory().await.unwrap();
    // Инициализируем шину с малым размером кольцевого буфера L1 (16 записей)
    let bus = EventBus::with_options(Some(db.clone()), 16);

    // Публикуем 25 надежных событий (первые 9 будут вытеснены из L1)
    for i in 0..25 {
        let msg = EventMessage::reliable(
            "test.reliable.safety",
            "tester",
            serde_json::json!({"seq": i}),
        );
        bus.publish(msg).await.unwrap();
    }

    // Даем микропаузу воркеру на батч-сброс в SQLite
    tokio::time::sleep(Duration::from_millis(200)).await;

    // В журнале должно быть ровно 25 записей без отката транзакций
    let journal = bus.query_journal(Some("test.reliable.safety"), None, 100).await.unwrap();
    assert_eq!(journal.len(), 25);
}

#[tokio::test]
async fn test_query_history_chronological_ordering() {
    let db = Db::init_in_memory().await.unwrap();
    let bus = EventBus::with_options(Some(db.clone()), 16);

    for i in 0..25 {
        let msg = EventMessage::reliable(
            "test.history.order",
            "tester",
            serde_json::json!({"seq": i}),
        );
        bus.publish(msg).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(150)).await;

    // Запрашиваем 20 последних событий
    let history = bus.query_history(Some("test.history.order"), 20).await.unwrap();
    assert_eq!(history.len(), 20);

    let seqs: Vec<i64> = history.iter().map(|e| e.payload.get("seq").unwrap().as_i64().unwrap()).collect();
    assert_eq!(seqs[0], 5, "Oldest element in 20-window should be seq 5");
    assert_eq!(seqs[19], 24, "Newest element should be seq 24");
}

#[tokio::test]
async fn test_dedup_no_poisoning_on_uuid_collision() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe();

    let shared_uuid = uuid::Uuid::new_v4();

    // 1. Сообщение 1: shared_uuid + key_A
    let mut msg1 = EventMessage::telemetry("topic.dedup.safe", "tester", serde_json::json!({"v": 1}));
    msg1.id = shared_uuid;
    msg1.dedup_key = Some("key_A".to_string());
    bus.publish(msg1).await.unwrap();

    let rec1 = sub.recv().await.unwrap();
    assert_eq!(rec1.payload.get("v").unwrap(), 1);

    // 2. Сообщение 2: shared_uuid + key_B (должно быть отклонено по UUID)
    let mut msg2 = EventMessage::telemetry("topic.dedup.safe", "tester", serde_json::json!({"v": 2}));
    msg2.id = shared_uuid;
    msg2.dedup_key = Some("key_B".to_string());
    bus.publish(msg2).await.unwrap();

    // 3. Сообщение 3: new_uuid + key_B (должно успешно дойти!)
    let mut msg3 = EventMessage::telemetry("topic.dedup.safe", "tester", serde_json::json!({"v": 3}));
    msg3.id = uuid::Uuid::new_v4();
    msg3.dedup_key = Some("key_B".to_string());
    bus.publish(msg3).await.unwrap();

    let rec3 = sub.recv().await.unwrap();
    assert_eq!(rec3.payload.get("v").unwrap(), 3);
}

#[tokio::test]
async fn test_expired_event_routed_to_dlq() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe_topic("expired.test");

    let mut expired_msg = EventMessage::telemetry("expired.test", "tester", serde_json::json!({"state": "too_old"}));
    expired_msg.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(5));

    bus.publish(expired_msg).await.unwrap();

    // Подписчик не должен получить просроченное сообщение
    tokio::select! {
        _ = sub.recv() => panic!("Expired event should not be delivered!"),
        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
    }

    // Сообщение должно быть зафиксировано в DLQ
    let dls = bus.dead_letters(10);
    assert_eq!(dls.len(), 1);
    assert_eq!(dls[0].event.topic, "expired.test");
    assert_eq!(dls[0].reason, aethercore_core::bus::DeadLetterReason::Expired);
}

#[tokio::test]
async fn test_subscriber_dropped_metrics() {
    let bus = EventBus::in_memory();
    let _sub = bus.subscribe(); // Буфер 1024, не вычитываем

    // Отправляем 1100 сообщений
    for i in 0..1100 {
        let msg = EventMessage::telemetry("flood.test", "tester", serde_json::json!({"i": i}));
        bus.publish(msg).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(80)).await;

    let stats = bus.stats();
    assert!(stats.dropped_total > 0, "Dropped total should be > 0, got {}", stats.dropped_total);
}

#[tokio::test]
async fn test_topology_ignores_rpc_reply_topics() {
    let bus = EventBus::in_memory();

    let bus_clone = bus.clone();
    let mut srv = bus_clone.subscribe_topic("echo.service");
    tokio::spawn(async move {
        while let Some(req) = srv.recv().await {
            let _ = bus_clone.reply_to(&req, serde_json::json!({"ack": true})).await;
        }
    });

    for i in 0..5 {
        let _ = bus
            .request("echo.service", serde_json::json!({"req": i}), Duration::from_millis(50))
            .await;
    }

    tokio::time::sleep(Duration::from_millis(60)).await;

    let topo = bus.topology();
    let reply_nodes: Vec<_> = topo.nodes.iter().filter(|n| n.label.starts_with("_reply.")).collect();
    assert_eq!(reply_nodes.len(), 0, "Topology should not contain ephemeral _reply. nodes");
}

#[tokio::test]
async fn test_nested_masking_interceptor() {
    use aethercore_core::bus::MaskingInterceptor;
    use std::sync::Arc;

    let mut bus = EventBus::in_memory();
    bus.add_interceptor(Arc::new(MaskingInterceptor::default()));
    let mut sub = bus.subscribe();

    let msg = EventMessage::telemetry(
        "auth.nested",
        "auth",
        serde_json::json!({
            "credentials": {
                "password": "super_secret_password",
                "api_key": "raw_secret_api_key"
            },
            "array_items": [
                {"token": "jwt_secret_token", "name": "service1"}
            ]
        }),
    );

    bus.publish(msg).await.unwrap();

    let rec = sub.recv().await.unwrap();
    assert_eq!(
        rec.payload.pointer("/credentials/password").unwrap(),
        "***"
    );
    assert_eq!(
        rec.payload.pointer("/credentials/api_key").unwrap(),
        "***"
    );
    assert_eq!(
        rec.payload.pointer("/array_items/0/token").unwrap(),
        "***"
    );
    assert_eq!(
        rec.payload.pointer("/array_items/0/name").unwrap(),
        "service1"
    );
}

#[tokio::test]
async fn test_retained_true_lru_eviction() {
    use aethercore_core::bus::RetainedStore;
    let store = RetainedStore::new(3);

    // Добавляем A, B, C
    store.put(EventMessage::telemetry("topic.A", "t", serde_json::json!({"v": 1})).with_retain(true));
    store.put(EventMessage::telemetry("topic.B", "t", serde_json::json!({"v": 1})).with_retain(true));
    store.put(EventMessage::telemetry("topic.C", "t", serde_json::json!({"v": 1})).with_retain(true));

    // Обновляем A и B -> теперь C становится самым старым в LRU
    store.put(EventMessage::telemetry("topic.A", "t", serde_json::json!({"v": 2})).with_retain(true));
    store.put(EventMessage::telemetry("topic.B", "t", serde_json::json!({"v": 2})).with_retain(true));

    // Добавляем D -> должен быть вытеснен C, а не A!
    store.put(EventMessage::telemetry("topic.D", "t", serde_json::json!({"v": 1})).with_retain(true));

    assert!(!store.get_matching("topic.A", 1).is_empty(), "Topic A should be retained");
    assert!(!store.get_matching("topic.B", 1).is_empty(), "Topic B should be retained");
    assert!(store.get_matching("topic.C", 1).is_empty(), "Topic C should have been evicted by LRU");
    assert!(!store.get_matching("topic.D", 1).is_empty(), "Topic D should be retained");
}

#[tokio::test]
async fn test_zero_limit_queries() {
    let db = Db::init_in_memory().await.unwrap();
    let bus = EventBus::new(db);

    let ev = EventMessage::reliable("test.zero", "core", serde_json::json!({"x": 1})).with_retain(true);
    bus.publish(ev).await.unwrap();

    tokio::time::sleep(Duration::from_millis(60)).await;

    assert_eq!(bus.query_history(None, 0).await.unwrap().len(), 0);
    assert_eq!(bus.query_journal(None, None, 0).await.unwrap().len(), 0);
    assert_eq!(bus.get_retained("test.zero", 0).len(), 0);
}

#[tokio::test]
async fn test_debounced_max_wait_under_continuous_flood() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe_topic("flood.topic");

    let bus_clone = bus.clone();
    tokio::spawn(async move {
        // Непрерывная отправка событий каждые 5ms (без пауз)
        for i in 0..50 {
            let _ = bus_clone
                .publish(EventMessage::telemetry("flood.topic", "src", serde_json::json!({"i": i})))
                .await;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });

    // quiet_period = 30ms, max_wait = 50ms
    // Поскольку паузы в 30ms нет, recv_debounced_max_wait обязан отдать промежуточное значение по max_wait (не зависая)
    let start = std::time::Instant::now();
    let msg = sub
        .recv_debounced_max_wait(Duration::from_millis(30), Duration::from_millis(50))
        .await
        .expect("Should receive debounced event within max_wait");

    assert!(start.elapsed() < Duration::from_millis(150));
    assert_eq!(msg.topic, "flood.topic");
}

#[tokio::test]
async fn test_topology_topic_capping() {
    use aethercore_core::bus::BusTopologyTracker;
    let topology = BusTopologyTracker::new();

    // Записываем 2005 уникальных топиков
    for i in 0..2005 {
        topology.record_publish("test_pub", &format!("topic.item_{}", i));
    }

    let snap = topology.snapshot();
    assert!(snap.topics_count <= 2000, "Topics count should be capped at 2000");
}

#[tokio::test]
async fn test_topology_publisher_capping() {
    use aethercore_core::bus::BusTopologyTracker;
    let topology = BusTopologyTracker::new();

    // Записываем 1005 уникальных издателей
    for i in 0..1005 {
        topology.record_publish(&format!("dynamic_pub_{}", i), "shared.topic");
    }

    let snap = topology.snapshot();
    assert!(snap.publishers_count <= 1000, "Publishers count should be capped at 1000");
}

#[tokio::test]
async fn test_retained_not_saved_if_interceptor_drops() {
    use aethercore_core::bus::{EventInterceptor, InterceptorAction};
    use aethercore_common::error::Result;
    use async_trait::async_trait;
    use std::sync::Arc;

    struct DropAllInterceptor;
    #[async_trait]
    impl EventInterceptor for DropAllInterceptor {
        async fn pre_publish(&self, _event: &mut EventMessage) -> Result<InterceptorAction> {
            Ok(InterceptorAction::DropSilently)
        }
    }

    let mut bus = EventBus::in_memory();
    bus.add_interceptor(Arc::new(DropAllInterceptor));

    let msg = EventMessage::telemetry(
        "secret.state",
        "agent",
        serde_json::json!({"secret_data": 123}),
    )
    .with_retain(true);

    bus.publish(msg).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // В RetainedStore не должно сохраниться фантомное отброшенное событие
    let retained = bus.get_retained("secret.state", 10);
    assert_eq!(retained.len(), 0, "Dropped event must not be retained in cache");
}

#[tokio::test]
async fn test_redrive_clears_expired_at_and_delivers() {
    let bus = EventBus::in_memory();
    let mut sub = bus.subscribe_topic("alarms.redrive");

    // Создаем просроченное сообщение
    let mut msg = EventMessage::telemetry("alarms.redrive", "sensor", serde_json::json!({"val": 99}));
    msg.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(10));

    bus.publish(msg).await.unwrap();

    // Ждем перемещения в DLQ
    tokio::time::sleep(Duration::from_millis(50)).await;
    let dead_letters = bus.dead_letters(10);
    assert_eq!(dead_letters.len(), 1);
    let dl_id = dead_letters[0].id;

    // Выполняем redrive - expires_at должен быть сброшен в None и событие доставлено
    bus.redrive_dead_letter(dl_id).await.unwrap();

    let delivered = sub.recv().await.unwrap();
    assert_eq!(delivered.topic, "alarms.redrive");
    assert_eq!(delivered.expires_at, None);
}

#[tokio::test]
async fn test_dedup_does_not_lock_on_interceptor_reject() {
    use aethercore_core::bus::{EventInterceptor, InterceptorAction};
    use aethercore_common::error::{AppError, Result};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct RejectOnceInterceptor {
        should_reject: AtomicBool,
    }

    #[async_trait]
    impl EventInterceptor for RejectOnceInterceptor {
        async fn pre_publish(&self, _event: &mut EventMessage) -> Result<InterceptorAction> {
            if self.should_reject.swap(false, Ordering::SeqCst) {
                Err(AppError::validation("test", "Intentional reject"))
            } else {
                Ok(InterceptorAction::Continue)
            }
        }
    }

    let mut bus = EventBus::in_memory();
    bus.add_interceptor(Arc::new(RejectOnceInterceptor {
        should_reject: AtomicBool::new(true),
    }));
    let mut sub = bus.subscribe();

    let msg = EventMessage::telemetry("retry.topic", "service", serde_json::json!({"attempt": 1}));

    // Попытка 1: интерцептор отклоняет
    let err = bus.publish(msg.clone()).await;
    assert!(err.is_err());

    // Попытка 2 (retry): дедупликатор НЕ должен блокировать сообщение, так как первая попытка не прошла
    let ok = bus.publish(msg.clone()).await;
    assert!(ok.is_ok());

    let rec = sub.recv().await.unwrap();
    assert_eq!(rec.topic, "retry.topic");
}

#[tokio::test]
async fn test_expired_event_step0_dlq_does_not_pollute_ring() {
    let bus = EventBus::in_memory();

    let mut msg = EventMessage::telemetry("ttl.expired", "core", serde_json::json!({"val": 1}));
    msg.expires_at = Some(chrono::Utc::now() - chrono::Duration::seconds(5));

    bus.publish(msg).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // В RingBuffer ничего не должно попасть
    let history = bus.query_history(None, 10).await.unwrap();
    assert_eq!(history.len(), 0);

    // Должно быть в DLQ
    let dlq = bus.dead_letters(10);
    assert_eq!(dlq.len(), 1);
    assert_eq!(dlq[0].reason, aethercore_core::bus::DeadLetterReason::Expired);
}

#[tokio::test]
async fn test_masking_interceptor_case_insensitive_and_suffixes() {
    use aethercore_core::bus::MaskingInterceptor;
    use std::sync::Arc;

    let mut bus = EventBus::in_memory();
    bus.add_interceptor(Arc::new(MaskingInterceptor::default()));
    let mut sub = bus.subscribe();

    let msg = EventMessage::telemetry(
        "auth.tokens",
        "auth",
        serde_json::json!({
            "apiKey": "12345",
            "refresh_token": "secret_refresh_val",
            "client_secret": "my_client_secret",
            "userPassword": "pass",
            "public_id": "safe_val"
        }),
    );

    bus.publish(msg).await.unwrap();

    let rec = sub.recv().await.unwrap();
    assert_eq!(rec.payload.get("apiKey").unwrap(), "***");
    assert_eq!(rec.payload.get("refresh_token").unwrap(), "***");
    assert_eq!(rec.payload.get("client_secret").unwrap(), "***");
    assert_eq!(rec.payload.get("userPassword").unwrap(), "***");
    assert_eq!(rec.payload.get("public_id").unwrap(), "safe_val");
}

#[tokio::test]
async fn test_sql_like_escaping() {
    let db = Db::init_in_memory().await.unwrap();
    let bus = EventBus::new(db);

    let ev1 = EventMessage::reliable("sensor_temp.1", "core", serde_json::json!({"t": 20}));
    let ev2 = EventMessage::reliable("sensor.temp.1", "core", serde_json::json!({"t": 25}));

    bus.publish(ev1).await.unwrap();
    bus.publish(ev2).await.unwrap();

    tokio::time::sleep(Duration::from_millis(80)).await;

    // Запрос по префиксу "sensor_" не должен сопоставлять "sensor." (символ '_' экранирован)
    let journal = bus.query_journal(Some("sensor_"), None, 10).await.unwrap();
    assert_eq!(journal.len(), 1);
    assert_eq!(journal[0].topic, "sensor_temp.1");
}

#[tokio::test]
async fn test_invalid_topic_patterns_validation() {
    let bus = EventBus::in_memory();

    // Некорректные шаблоны топиков (пустые сегменты, '#' не на конце) игнорируются при подписке
    let mut sub = bus.subscribe_topics(&["valid.topic", "a..b", ".start", "mid.#.end"]);

    let ev = EventMessage::telemetry("valid.topic", "core", serde_json::json!({"v": 1}));
    bus.publish(ev).await.unwrap();

    let rec = sub.recv().await.unwrap();
    assert_eq!(rec.topic, "valid.topic");
}






