// Unit-тесты для шины событий (bus.rs) и проверки шаблонов подписок (match_topic)

use nms_core::server::{ConnectionManager, MAX_CONNECTIONS_PER_USER};
use nms_core::{
    get_missed_events_from_db, init_db_pool, match_topic, record_event_in_db, EventBus,
    EventJournalQueue, SystemEvent,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[test]
fn test_match_topic_wildcards() {
    assert!(match_topic("*", "device.ping.down"));
    assert!(match_topic("#", "device.ping.down"));
    assert!(match_topic("device.ping.down", "device.ping.down"));
    assert!(!match_topic("device.ping.up", "device.ping.down"));

    // Проверка маски '+' на одном уровне
    assert!(match_topic("device.+.down", "device.ping.down"));
    assert!(match_topic("device.+.down", "device.snmp.down"));
    assert!(!match_topic("device.+.down", "device.ping.snmp.down"));

    // Проверка хвостовой маски '#'
    assert!(match_topic("core.#", "core.system.started"));
    assert!(match_topic("core.#", "core.modules.enabled.test"));
    assert!(!match_topic("core.#", "system.started"));
}

#[tokio::test]
async fn test_event_bus_publish_subscribe() {
    let bus = EventBus::new(100);
    let mut rx = bus.subscribe_receiver();

    let event = SystemEvent::new(
        "device.ping.down",
        serde_json::json!({ "ip": "192.168.1.1" }),
        "ping_collector",
    )
    .with_target_user("user-admin-01");

    let delivered = bus.publish(event.clone(), false).unwrap();
    assert_eq!(delivered, 1);

    let received = rx.recv().await.unwrap();
    assert_eq!(received, event);
    assert_eq!(received.target_user_id.as_deref(), Some("user-admin-01"));
}

#[test]
fn test_core_topic_reservation() {
    let bus = EventBus::new(100);
    let event = SystemEvent::new(
        "core.modules.loaded",
        serde_json::json!({ "module": "test" }),
        "untrusted_plugin",
    );

    // Блокировка публикации в core.* при is_core = false
    assert!(bus.publish(event.clone(), false).is_err());

    // Успешная публикация при is_core = true
    assert!(bus.publish(event, true).is_ok());
}

#[test]
fn test_event_bus_callbacks_and_stats() {
    let bus = EventBus::new(100);
    let called = Arc::new(AtomicBool::new(false));
    let called_clone = Arc::clone(&called);

    bus.subscribe(
        "device.#",
        Arc::new(move |_ev| {
            called_clone.store(true, Ordering::SeqCst);
        }),
    );

    let stats = bus.get_stats();
    assert_eq!(stats.callback_count, 1);
    assert_eq!(stats.callback_patterns, vec!["device.#"]);

    let event = SystemEvent::new("device.ping.down", serde_json::json!({}), "tester");
    let _ = bus.publish(event, false);

    assert!(called.load(Ordering::SeqCst));

    assert!(bus.unsubscribe("device.#"));
    assert_eq!(bus.get_stats().callback_count, 0);

    bus.shutdown();
}

#[tokio::test]
async fn test_connection_manager_limits() {
    let cm = ConnectionManager::new();
    let user_id = "test-user-limit";

    for _ in 0..MAX_CONNECTIONS_PER_USER {
        assert!(cm.add_connection(user_id));
    }

    // 11-е подключение должно быть заблокировано
    assert!(!cm.add_connection(user_id));

    // После закрытия одного сокета можно подключиться вновь
    cm.remove_connection(user_id);
    assert!(cm.add_connection(user_id));
}

#[tokio::test]
async fn test_event_journaling_and_replay() {
    let db_path = std::path::PathBuf::from(":memory:");
    let pool = init_db_pool(&db_path).await.unwrap();

    let seq1 = record_event_in_db(
        &pool,
        "device.down",
        &serde_json::json!({"ip": "10.0.0.1"}),
        Some("usr-1"),
        Some("device.down"),
    )
    .await
    .unwrap();
    let seq2 = record_event_in_db(
        &pool,
        "device.up",
        &serde_json::json!({"ip": "10.0.0.1"}),
        Some("usr-1"),
        Some("device.up"),
    )
    .await
    .unwrap();

    assert!(seq1 > 0);
    assert!(seq2 > seq1);

    let missed = get_missed_events_from_db(&pool, 0, Some("usr-1"), None, 10)
        .await
        .unwrap();
    assert_eq!(missed.len(), 2);
    assert_eq!(missed[0].topic, "device.down");
    assert_eq!(missed[1].topic, "device.up");

    // Выборка со смещением seq_id
    let missed_after_seq1 = get_missed_events_from_db(&pool, seq1, Some("usr-1"), None, 10)
        .await
        .unwrap();
    assert_eq!(missed_after_seq1.len(), 1);
    assert_eq!(missed_after_seq1[0].topic, "device.up");

    // Пакетная запись через EventJournalQueue
    let queue = EventJournalQueue::new(pool.clone(), 50);
    queue
        .enqueue(
            "system.started".to_string(),
            serde_json::json!({"ver": "2.0"}),
            None,
            Some("system.started".to_string()),
        )
        .await;
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    let all_events = get_missed_events_from_db(&pool, 0, None, None, 10)
        .await
        .unwrap();
    assert_eq!(all_events.len(), 3);
}

#[test]
fn test_bus_1to1_api_compatibility() {
    use nms_core::{_inspect_subscriber_params, event_bus, Subscriber, EVENT_BUS};

    // 1. Проверка _inspect_subscriber_params
    assert_eq!(_inspect_subscriber_params(), 1);

    // 2. Проверка создания Subscriber
    let sub = Subscriber::new("core.#", Arc::new(|_ev| {}));
    assert!(sub.has_wildcard);
    assert_eq!(sub.pattern, "core.#");

    // 3. Проверка глобальной шины EVENT_BUS / event_bus()
    assert_eq!(event_bus().get_stats().callback_count, 0);
    EVENT_BUS.subscribe("test.*", Arc::new(|_ev| {}));
    assert_eq!(EVENT_BUS.get_stats().callback_count, 1);
    EVENT_BUS.clear();
    assert_eq!(EVENT_BUS.get_stats().callback_count, 0);
}
