//! # Интеграционные тесты WebSocket-шлюза (`/ws/events`)

use aethercore_common::config::AppConfig;
use aethercore_common::models::events::{EventMessage, EventPriority};
use aethercore_common::models::user::JwtClaims;
use aethercore_core::auth::JwtManager;
use aethercore_core::bus::EventBus;
use aethercore_core::db::Db;
use aethercore_core::plugins::PluginManager;
use aethercore_core::services::{AuditService, LoggerService, NotifyService, SessionService};
use aethercore_core::users::UserService;
use aethercore_server::state::AppState;
use aethercore_server::ws::session::WsSession;
use aethercore_server::ws::types::{WsClientCommand, WsCodecFormat, WsServerMessage};
use axum::extract::ws::Message;
use std::time::Instant;
use uuid::Uuid;

async fn setup_test_state() -> AppState {
    let db = Db::init_in_memory().await.expect("DB in memory failed");
    let bus = EventBus::new(db.clone());
    let jwt_manager = JwtManager::new("test-secret-key-12345", 3600);
    let user_service = UserService::new(db.clone());
    let session_service = SessionService::new(db.clone());
    let audit_service = AuditService::new(db.clone());
    let logger_service = LoggerService::new();
    let notify_service = NotifyService::new();
    let plugin_manager = PluginManager::new(db.clone(), bus.clone());
    let scheduler_service = std::sync::Arc::new(
        aethercore_core::services::SchedulerService::new(db.clone(), bus.clone()),
    );
    let backup_service = aethercore_core::services::BackupService::new(db.clone());

    user_service.ensure_default_admin().await.unwrap();

    AppState {
        config: AppConfig::default(),
        db,
        bus,
        jwt_manager,
        user_service,
        session_service,
        audit_service,
        logger_service,
        notify_service,
        plugin_manager,
        scheduler_service,
        backup_service,
        start_time: Instant::now(),
    }
}

#[tokio::test]
async fn test_ws_codecs_json_and_msgpack() {
    let json_codec = WsCodecFormat::Json;
    let msgpack_codec = WsCodecFormat::MessagePack;

    let cmd = WsClientCommand::Publish {
        msg_id: Some("msg-1".into()),
        tab_id: Some("tab-1".into()),
        topic: "plugin.test.event".into(),
        payload: serde_json::json!({ "value": 42 }),
        priority: EventPriority::High,
        retain: true,
    };

    // 1. JSON Roundtrip
    let json_frame = json_codec.encode(&cmd).expect("JSON encode failed");
    match json_frame {
        Message::Text(ref text) => {
            assert!(text.contains("plugin.test.event"));
        }
        _ => panic!("Expected text frame for JSON codec"),
    }
    let decoded_json: WsClientCommand = json_codec.decode(&json_frame).expect("JSON decode failed");
    if let WsClientCommand::Publish { topic, priority, .. } = decoded_json {
        assert_eq!(topic, "plugin.test.event");
        assert_eq!(priority, EventPriority::High);
    } else {
        panic!("Decoded command mismatch");
    }

    // 2. MessagePack Roundtrip
    let msgpack_frame = msgpack_codec.encode(&cmd).expect("MessagePack encode failed");
    match msgpack_frame {
        Message::Binary(ref bin) => {
            assert!(!bin.is_empty());
        }
        _ => panic!("Expected binary frame for MessagePack codec"),
    }
    let decoded_msgpack: WsClientCommand = msgpack_codec.decode(&msgpack_frame).expect("MessagePack decode failed");
    if let WsClientCommand::Publish { topic, priority, .. } = decoded_msgpack {
        assert_eq!(topic, "plugin.test.event");
        assert_eq!(priority, EventPriority::High);
    } else {
        panic!("Decoded command mismatch");
    }
}

#[tokio::test]
async fn test_ws_session_seq_and_rbac() {
    let claims = JwtClaims {
        sub: Uuid::new_v4(),
        username: "operator".to_string(),
        is_superuser: false,
        roles: vec!["operator".to_string()],
        permissions: vec!["events.view".to_string(), "modules.view".to_string()],
        exp: 9999999999,
        iat: 1000000000,
        session_id: None,
    };

    let session = WsSession::new(Some(claims), WsCodecFormat::Json, "127.0.0.1".into());

    // 1. Монотонный Sequence ID
    assert_eq!(session.next_seq(), 1);
    assert_eq!(session.next_seq(), 2);
    assert_eq!(session.next_seq(), 3);

    // 2. Topic RBAC Guard
    assert!(session.can_read_topic("plugin.topology.nodes").await);
    assert!(session.can_read_topic("devices.sensor1").await);
    // Доступ к системным топикам безопасности запрещен обычному оператору
    assert!(!session.can_read_topic("system.auth.login").await);
    assert!(!session.can_write_topic("system.config").await);

    // 3. Superuser RBAC
    let admin_claims = JwtClaims {
        sub: Uuid::new_v4(),
        username: "admin".to_string(),
        is_superuser: true,
        roles: vec!["superuser".to_string()],
        permissions: vec!["system.manage".to_string()],
        exp: 9999999999,
        iat: 1000000000,
        session_id: None,
    };
    let admin_session = WsSession::new(Some(admin_claims), WsCodecFormat::MessagePack, "127.0.0.1".into());
    assert!(admin_session.can_read_topic("system.auth.login").await);
    assert!(admin_session.can_write_topic("system.config").await);
}

#[tokio::test]
async fn test_ws_server_messages_serialization() {
    let codec = WsCodecFormat::Json;

    let ev_msg = EventMessage::telemetry("plugin.chat.stream", "agent", serde_json::json!({ "delta": "Hello" }));
    let server_msg = WsServerMessage::Event {
        seq: 42,
        event: ev_msg,
    };

    let encoded = codec.encode(&server_msg).expect("Encode event failed");
    if let Message::Text(text) = encoded {
        assert!(text.contains("\"seq\":42"));
        assert!(text.contains("plugin.chat.stream"));
    } else {
        panic!("Expected text frame");
    }

    let ack_msg = WsServerMessage::Ack {
        msg_id: "req-123".into(),
        status: "ok".into(),
    };
    let encoded_ack = codec.encode(&ack_msg).expect("Encode ack failed");
    if let Message::Text(text) = encoded_ack {
        assert!(text.contains("req-123"));
        assert!(text.contains("\"status\":\"ok\""));
    } else {
        panic!("Expected text frame");
    }
}

#[tokio::test]
async fn test_rest_over_ws_in_process_dispatch() {
    use tower::ServiceExt;
    let state = setup_test_state().await;
    let router = aethercore_server::create_app_router(state);

    // Имитируем вызов REST эндпоинта /health через виртуальный HTTP-запрос
    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/health")
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.oneshot(req).await.expect("Router dispatch failed");
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert_eq!(&body_bytes[..], b"OK");
}
