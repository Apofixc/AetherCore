// Unit-тесты для модуля событий events (1-в-1 API соответствие с Python events.py)

use nms_core::events::{
    broadcaster, bus_ws_bridge, check_replay_status_from_db, prune_system_events_journal,
    record_event_in_db, ws_manager,
};
use nms_core::{
    can_subscribe_to_topic, extract_token_and_subprotocol, get_events_info_handler, init_db_pool,
    MAX_FRAME_SIZE, MAX_JSON_ERRORS, MAX_MESSAGES_PER_SECOND,
};

#[tokio::test]
async fn test_events_constants_and_subprotocol() {
    assert_eq!(MAX_FRAME_SIZE, 65536);
    assert_eq!(MAX_MESSAGES_PER_SECOND, 50);
    assert_eq!(MAX_JSON_ERRORS, 5);

    let (token, subproto) = extract_token_and_subprotocol(Some("bearer, wst_12345"), None);
    assert_eq!(token, Some("wst_12345".to_string()));
    assert_eq!(subproto, Some("bearer".to_string()));

    let (token2, subproto2) = extract_token_and_subprotocol(None, Some("my_token"));
    assert_eq!(token2, Some("my_token".to_string()));
    assert_eq!(subproto2, None);
}

#[tokio::test]
async fn test_get_events_info_endpoint() {
    let info = get_events_info_handler().await;
    assert_eq!(info["status"], "online");
    assert_eq!(info["transport"], "websocket");
}

#[tokio::test]
async fn test_can_subscribe_to_topic() {
    assert!(!can_subscribe_to_topic(None, None, "", true).await);
    assert!(can_subscribe_to_topic(None, None, "device.ping", false).await);
    assert!(!can_subscribe_to_topic(None, None, "device.ping", true).await);
}

#[tokio::test]
async fn test_events_journal_and_replay_status() {
    let db_path = std::path::PathBuf::from(":memory:");
    let pool = init_db_pool(&db_path).await.unwrap();

    let seq1 = record_event_in_db(
        &pool,
        "system.boot",
        "{\"ver\":\"1.0\"}",
        Some("admin"),
        Some("system"),
    )
    .await
    .unwrap();
    assert!(seq1 > 0);

    let (status, missed) = check_replay_status_from_db(&pool, 0, Some("admin"), None, 10)
        .await
        .unwrap();
    assert_eq!(status, "replay");
    assert_eq!(missed.len(), 1);

    let pruned = prune_system_events_journal(&pool, 0, 0).await.unwrap();
    assert!(pruned >= 1);
}

#[tokio::test]
async fn test_connection_manager_and_broadcaster_singletons() {
    let cm = ws_manager();
    assert!(cm.connect_user("usr-test"));
    cm.update_pong(1);
    let metrics = cm.get_metrics();
    assert_eq!(metrics["active_connections"], 1);

    let bc = broadcaster();
    bc.broadcast(
        None,
        "",
        Some(serde_json::json!({"type": "ping", "ok": true})),
        None,
        None,
        true,
    );

    let bridge = bus_ws_bridge();
    bridge.setup();
    bridge.on_bus_event("test.topic", &serde_json::json!({"key": "val"}));
}
