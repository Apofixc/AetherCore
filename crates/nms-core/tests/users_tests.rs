//! # Тесты сервиса пользователей UserService и RBAC

use nms_common::models::user::CreateUserDto;
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
            roles: Some(vec!["viewer".into()]),
        })
        .await
        .unwrap();

    assert_eq!(operator.username, "operator1");
    assert_eq!(operator.roles, vec!["viewer"]);
    assert!(operator.permissions.contains(&"system.view".to_string()));

    // Проверка неверного пароля
    let auth_failed = service.authenticate("operator1", "wrongpassword").await;
    assert!(auth_failed.is_err());
}
