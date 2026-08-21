//! # Тесты сервиса пользователей UserService, квот и политик безопасности RBAC

use nms_common::models::user::{CreateUserDto, UpdateUserDto};
use nms_core::db::Db;
use nms_core::users::UserService;

#[tokio::test]
async fn test_user_crud_and_auth() {
    let db = Db::init_in_memory().await.unwrap();
    let service = UserService::new(db);

    // Инициализация дефолтного админа
    service.ensure_default_admin().await.unwrap();

    // Аутентификация админа
    let admin = service.authenticate("admin", "admin").await.unwrap();
    assert_eq!(admin.username, "admin");
    assert!(admin.is_superuser);

    // Создание нового пользователя
    let operator = service
        .create_user(CreateUserDto {
            username: "operator1".into(),
            password: "password123".into(),
            full_name: Some("Operator One".into()),
            email: Some("op1@test.local".into()),
            is_active: Some(true),
            is_superuser: Some(false),
            must_change_password: Some(true),
            roles: Some(vec!["viewer".into()]),
        })
        .await
        .unwrap();

    assert_eq!(operator.username, "operator1");
    assert_eq!(operator.roles, vec!["viewer"]);
    assert!(operator.must_change_password);
    assert!(operator.permissions.contains(&"system.view".to_string()));

    // Проверка неверного пароля
    let auth_failed = service.authenticate("operator1", "wrongpassword").await;
    assert!(auth_failed.is_err());
}

#[tokio::test]
async fn test_superusers_quota_limit_4() {
    let db = Db::init_in_memory().await.unwrap();
    let service = UserService::new(db);

    // 1-й суперпользователь: admin
    service.ensure_default_admin().await.unwrap();

    // 2-й суперпользователь
    service
        .create_user(CreateUserDto {
            username: "super2".into(),
            password: "password123".into(),
            full_name: None,
            email: None,
            is_active: Some(true),
            is_superuser: Some(true),
            must_change_password: None,
            roles: Some(vec!["superuser".into()]),
        })
        .await
        .unwrap();

    // 3-й суперпользователь
    service
        .create_user(CreateUserDto {
            username: "super3".into(),
            password: "password123".into(),
            full_name: None,
            email: None,
            is_active: Some(true),
            is_superuser: Some(true),
            must_change_password: None,
            roles: Some(vec!["superuser".into()]),
        })
        .await
        .unwrap();

    // 4-й суперпользователь
    let super4 = service
        .create_user(CreateUserDto {
            username: "super4".into(),
            password: "password123".into(),
            full_name: None,
            email: None,
            is_active: Some(true),
            is_superuser: Some(true),
            must_change_password: None,
            roles: Some(vec!["superuser".into()]),
        })
        .await
        .unwrap();

    assert_eq!(service.count_superusers().await.unwrap(), 4);

    // 5-й суперпользователь -> должен получить ошибку квоты
    let super5_res = service
        .create_user(CreateUserDto {
            username: "super5".into(),
            password: "password123".into(),
            full_name: None,
            email: None,
            is_active: Some(true),
            is_superuser: Some(true),
            must_change_password: None,
            roles: Some(vec!["superuser".into()]),
        })
        .await;

    assert!(super5_res.is_err());

    // Создаем обычного пользователя и пробуем повысить до superuser -> тоже ошибка квоты
    let regular = service
        .create_user(CreateUserDto {
            username: "regular".into(),
            password: "password123".into(),
            full_name: None,
            email: None,
            is_active: Some(true),
            is_superuser: Some(false),
            must_change_password: None,
            roles: Some(vec!["viewer".into()]),
        })
        .await
        .unwrap();

    let promote_res = service
        .update_user(
            regular.id,
            UpdateUserDto {
                is_superuser: Some(true),
                ..Default::default()
            },
        )
        .await;

    assert!(promote_res.is_err());

    // Удаляем super4 -> теперь суперпользователей 3
    service.delete_user(super4.id).await.unwrap();
    assert_eq!(service.count_superusers().await.unwrap(), 3);

    // Теперь повышение regular должно пройти успешно
    let promote_ok = service
        .update_user(
            regular.id,
            UpdateUserDto {
                is_superuser: Some(true),
                ..Default::default()
            },
        )
        .await;
    assert!(promote_ok.is_ok());
    assert_eq!(service.count_superusers().await.unwrap(), 4);
}

#[tokio::test]
async fn test_cannot_delete_or_demote_last_superuser() {
    let db = Db::init_in_memory().await.unwrap();
    let service = UserService::new(db);

    service.ensure_default_admin().await.unwrap();
    let admin = service.get_user_by_username("admin").await.unwrap();

    // Попытка удалить единственного суперпользователя
    let delete_res = service.delete_user(admin.id).await;
    assert!(delete_res.is_err());

    // Попытка понизить роль единственного суперпользователя
    let demote_res = service
        .update_user(
            admin.id,
            UpdateUserDto {
                is_superuser: Some(false),
                roles: Some(vec!["operator".into()]),
                ..Default::default()
            },
        )
        .await;
    assert!(demote_res.is_err());

    // Попытка заблокировать суперпользователя
    let deactivate_res = service
        .update_user(
            admin.id,
            UpdateUserDto {
                is_active: Some(false),
                ..Default::default()
            },
        )
        .await;
    assert!(deactivate_res.is_err());
}
