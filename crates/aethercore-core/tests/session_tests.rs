//! # Тесты SessionService (глобальные сессии операторов)

use aethercore_common::models::user::CreateUserDto;
use aethercore_core::db::Db;
use aethercore_core::services::SessionService;
use aethercore_core::users::UserService;

#[tokio::test]
async fn test_session_lifecycle() {
    let db = Db::init_in_memory().await.unwrap();
    let user_service = UserService::new(db.clone());
    let service = SessionService::new(db);

    user_service.ensure_default_admin().await.unwrap();
    let admin = user_service.get_user_by_username("root").await.unwrap();

    let user_id = admin.id;
    let username = "root";
    let roles = vec!["admin".to_string()];
    let ip = "192.168.1.50";
    let ua = "Mozilla/5.0 Chrome/120.0 Linux";

    // 1. Создание сессии
    let session = service
        .create_session(user_id, username, &roles, ip, ua, 3600)
        .await
        .expect("Failed to create session");

    assert_eq!(session.username, "root");
    assert_eq!(session.ip_address, ip);
    assert_eq!(session.user_agent, ua);
    assert_eq!(session.roles, vec!["admin"]);

    // 2. Проверка валидности
    let is_valid = service.is_session_valid(session.id).await.unwrap();
    assert!(is_valid);

    // 3. Получение сессии
    let fetched = service.get_session(session.id).await.unwrap().expect("Session not found");
    assert_eq!(fetched.id, session.id);
    assert_eq!(fetched.user_id, user_id);

    // 4. Список активных сессий
    let list = service.list_active_sessions().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, session.id);

    // 5. Преобразование в DTO
    let dto = list[0].clone().into_dto(Some(session.id));
    assert!(dto.is_current);
    assert_eq!(dto.role, "Admin");

    // 6. Отзыв сессии
    let revoked = service.revoke_session(session.id).await.unwrap();
    assert!(revoked);

    let is_valid_after = service.is_session_valid(session.id).await.unwrap();
    assert!(!is_valid_after);

    let list_after = service.list_active_sessions().await.unwrap();
    assert_eq!(list_after.len(), 0);
}

#[tokio::test]
async fn test_revoke_other_and_all_sessions() {
    let db = Db::init_in_memory().await.unwrap();
    let user_service = UserService::new(db.clone());
    let service = SessionService::new(db);

    let u1 = user_service
        .create_user(CreateUserDto {
            username: "u1".into(),
            password: "Password1!".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let u2 = user_service
        .create_user(CreateUserDto {
            username: "u2".into(),
            password: "Password1!".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let u3 = user_service
        .create_user(CreateUserDto {
            username: "u3".into(),
            password: "Password1!".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let s1 = service.create_session(u1.id, "u1", &["operator".into()], "10.0.0.1", "UA1", 3600).await.unwrap();
    let s2 = service.create_session(u2.id, "u2", &["operator".into()], "10.0.0.2", "UA2", 3600).await.unwrap();
    let s3 = service.create_session(u3.id, "u3", &["admin".into()], "10.0.0.3", "UA3", 3600).await.unwrap();

    let list = service.list_active_sessions().await.unwrap();
    assert_eq!(list.len(), 3);

    // Сбросить чужие сессии относительно s3
    let count = service.revoke_all_except(s3.id).await.unwrap();
    assert_eq!(count, 2);

    assert!(!service.is_session_valid(s1.id).await.unwrap());
    assert!(!service.is_session_valid(s2.id).await.unwrap());
    assert!(service.is_session_valid(s3.id).await.unwrap());

    // Сбросить все сессии
    let all_count = service.revoke_all_sessions().await.unwrap();
    assert_eq!(all_count, 1);
    assert!(!service.is_session_valid(s3.id).await.unwrap());
}

