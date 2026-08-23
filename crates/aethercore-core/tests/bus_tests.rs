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
async fn test_masking_interceptor() {
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
