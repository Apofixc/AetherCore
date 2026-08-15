// Паритет с test_notifications_fixes.py / test_notifications_improvements.py:
// надежная доставка событий через персистентный Event Journal (SQLite)

use nms_core::db::{get_missed_events_from_db, init_db_pool, record_event_in_db};
use serde_json::json;

async fn test_pool() -> sqlx::SqlitePool {
    let dir = std::env::temp_dir().join(format!("nms-journal-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    init_db_pool(&dir.join("test.db")).await.unwrap()
}

#[tokio::test]
async fn test_event_persisted_in_journal() {
    let pool = test_pool().await;
    let seq = record_event_in_db(
        &pool,
        "poller.device.down",
        &json!({"device": "sw-01"}),
        None,
        Some("poller.device.down"),
    )
    .await
    .unwrap();
    assert!(seq > 0);
}

#[tokio::test]
async fn test_missed_events_replay_after_seq() {
    let pool = test_pool().await;
    let first = record_event_in_db(&pool, "m.a", &json!({"n": 1}), None, Some("m.a"))
        .await
        .unwrap();
    let second = record_event_in_db(&pool, "m.b", &json!({"n": 2}), None, Some("m.b"))
        .await
        .unwrap();
    assert!(second > first);

    // Подписчик, отставший после first, получает только последующие события
    let missed = get_missed_events_from_db(&pool, first, None, None, 100)
        .await
        .unwrap();
    assert_eq!(missed.len(), 1);

    // Актуальный подписчик не получает повторов
    let none = get_missed_events_from_db(&pool, second, None, None, 100)
        .await
        .unwrap();
    assert!(none.is_empty());
}

#[tokio::test]
async fn test_targeted_events_filtered_by_user() {
    let pool = test_pool().await;
    record_event_in_db(
        &pool,
        "m.private",
        &json!({}),
        Some("42"),
        Some("m.private"),
    )
    .await
    .unwrap();
    record_event_in_db(&pool, "m.public", &json!({}), None, Some("m.public"))
        .await
        .unwrap();

    // Чужой пользователь видит только широковещательные события
    let for_other = get_missed_events_from_db(&pool, 0, Some("7"), None, 100)
        .await
        .unwrap();
    assert_eq!(for_other.len(), 1);
    // Целевой пользователь видит оба
    let for_target = get_missed_events_from_db(&pool, 0, Some("42"), None, 100)
        .await
        .unwrap();
    assert_eq!(for_target.len(), 2);
}
