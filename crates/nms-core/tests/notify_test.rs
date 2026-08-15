// Unit и интеграционные тесты для модуля уведомлений notify.rs

use nms_core::{
    get_notification_categories, get_notification_modules, init_db_pool, is_quiet_hours, EventBus,
    NotificationEngine, NotificationFilter, NotificationSeverity, NotifyParams,
    SetPreferencesInput,
};
use std::path::PathBuf;

#[tokio::test]
async fn test_notification_engine_send_no_db() {
    let bus = EventBus::new(100);
    let mut rx = bus.subscribe_receiver();

    let engine = NotificationEngine::new(bus);

    let notif = engine
        .send_notification(
            Some("usr-01"),
            "Device Down",
            "Router 192.168.1.1 is not responding",
            NotificationSeverity::Error,
            "security",
            Some("ping-collector"),
        )
        .await
        .unwrap();

    assert_eq!(notif.title, "Device Down");
    assert_eq!(notif.severity, NotificationSeverity::Error);

    // Проверка публикации события в шину EventBus
    let event = rx.recv().await.unwrap();
    assert_eq!(event.topic, "core.notifications.created");
    assert_eq!(event.sender, "ping-collector");
}

#[tokio::test]
async fn test_notification_engine_full_db_flow() {
    let bus = EventBus::new(100);
    let pool = init_db_pool(&PathBuf::from(":memory:")).await.unwrap();

    let engine = NotificationEngine::new_with_db(bus, pool);

    // 1. Создание уведомления с поддержкой шаблона заголовка и группировки
    let params = NotifyParams {
        user_id: "user-100".to_string(),
        title: "Interface eth0 flap #{count}".to_string(),
        body: "Packet loss detected".to_string(),
        severity: NotificationSeverity::Warning,
        category: "system".to_string(),
        module_id: "network-mon".to_string(),
        title_template: Some("Interface eth0 flap #{count}".to_string()),
        ..Default::default()
    };

    let msg1 = engine.notify(params.clone()).await.unwrap().unwrap();
    assert_eq!(msg1.group_count, 1);
    assert_eq!(msg1.title, "Interface eth0 flap #1");

    // Повторное отправление той же аварии вызывает дедупликацию (группировку)
    let msg2 = engine.notify(params).await.unwrap().unwrap();
    assert_eq!(msg2.id, msg1.id);
    assert_eq!(msg2.group_count, 2);
    assert_eq!(msg2.title, "Interface eth0 flap #2");

    // 2. Проверка подсчета непрочитанных
    let unread = engine.count_unread_notifications("user-100").await.unwrap();
    assert_eq!(unread, 1);

    // 3. Добавление второго уведомления другого уровня
    engine
        .send_notification(
            Some("user-100"),
            "System Backup Complete",
            "Daily backup succeeded",
            NotificationSeverity::Success,
            "system",
            Some("core"),
        )
        .await
        .unwrap();

    let list_all = engine
        .get_user_notifications("user-100", &NotificationFilter::default())
        .await
        .unwrap();
    assert_eq!(list_all.total, 2);
    assert_eq!(list_all.unread_count, 2);

    // 4. Тест фильтрации по severity
    let filter_success = NotificationFilter {
        severity: Some("success".to_string()),
        ..Default::default()
    };
    let list_success = engine
        .get_user_notifications("user-100", &filter_success)
        .await
        .unwrap();
    assert_eq!(list_success.filtered_total, 1);
    assert_eq!(list_success.items[0].title, "System Backup Complete");

    // 5. Прочтение и квитирование
    let updated = engine.mark_as_read(msg1.id, "user-100").await.unwrap();
    assert!(updated);

    let unread_after = engine.count_unread_notifications("user-100").await.unwrap();
    assert_eq!(unread_after, 1);

    let acked = engine
        .acknowledge_notification(msg1.id, "user-100")
        .await
        .unwrap();
    assert!(acked);

    // 6. Настройки пользователя (Preferences & Min Severity)
    let prefs = engine
        .set_notification_preferences(
            "user-100",
            SetPreferencesInput {
                module_rules: Some(serde_json::json!({
                    "chat-app": {
                        "min_severity": "warning"
                    }
                })),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    assert!(prefs.module_rules.get("chat-app").is_some());

    // Попытка отправить info от chat-app — должна проигнорироваться по min_severity
    let omitted = engine
        .notify(NotifyParams {
            user_id: "user-100".to_string(),
            title: "New chat message".to_string(),
            severity: NotificationSeverity::Info,
            module_id: "chat-app".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    assert!(omitted.is_none());

    // 7. Экспорт логов в CSV и JSON
    let (csv_str, mime_csv) = engine
        .export_user_notifications("user-100", "csv", &NotificationFilter::default())
        .await
        .unwrap();
    assert_eq!(mime_csv, "text/csv");
    assert!(csv_str.contains("Interface eth0 flap #2"));

    let (json_str, mime_json) = engine
        .export_user_notifications("user-100", "json", &NotificationFilter::default())
        .await
        .unwrap();
    assert_eq!(mime_json, "application/json");
    assert!(json_str.contains("System Backup Complete"));

    // 8. Удаление прочитанных
    let cleared = engine.clear_read_notifications("user-100").await.unwrap();
    assert_eq!(cleared, 1);
}

#[test]
fn test_is_quiet_hours_calculation() {
    let qh_enabled = serde_json::json!({
        "enabled": true,
        "start": "00:00",
        "end": "23:59",
        "days": "everyday"
    });

    let now_ts = 1700000000.0;
    assert!(is_quiet_hours(&qh_enabled, now_ts));

    let qh_disabled = serde_json::json!({
        "enabled": false,
        "start": "00:00",
        "end": "23:59"
    });
    assert!(!is_quiet_hours(&qh_disabled, now_ts));
}

#[test]
fn test_get_notification_categories_and_modules() {
    let categories = get_notification_categories();
    assert!(categories.contains(&"system".to_string()));
    assert!(categories.contains(&"security".to_string()));
    assert!(categories.contains(&"module".to_string()));
    assert!(categories.contains(&"user".to_string()));

    let modules = get_notification_modules();
    assert!(!modules.is_empty());
    assert_eq!(modules[0].id, "core");
}
