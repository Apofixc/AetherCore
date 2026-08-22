//! # Тесты гибридной шины событий EventBus и Reliable Event Journal

use aethercore_common::models::events::EventMessage;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;

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
