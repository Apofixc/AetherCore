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
            password: "Password123!".into(),
            full_name: Some("Operator One".into()),
            email: Some("op1@test.local".into()),
            is_active: Some(true),
            is_superuser: Some(false),
            must_change_password: Some(true),
            roles: Some(vec!["viewer".into()]),
            ..Default::default()
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
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(true),
            roles: Some(vec!["superuser".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    // 3-й суперпользователь
    service
        .create_user(CreateUserDto {
            username: "super3".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(true),
            roles: Some(vec!["superuser".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    // 4-й суперпользователь
    let super4 = service
        .create_user(CreateUserDto {
            username: "super4".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(true),
            roles: Some(vec!["superuser".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(service.count_superusers().await.unwrap(), 4);

    // 5-й суперпользователь -> должен получить ошибку квоты
    let super5_res = service
        .create_user(CreateUserDto {
            username: "super5".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(true),
            roles: Some(vec!["superuser".into()]),
            ..Default::default()
        })
        .await;

    assert!(super5_res.is_err());

    // Создаем обычного пользователя и пробуем повысить до superuser -> тоже ошибка квоты
    let regular = service
        .create_user(CreateUserDto {
            username: "regular".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(false),
            roles: Some(vec!["viewer".into()]),
            ..Default::default()
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

#[tokio::test]
async fn test_username_change_on_first_login_only() {
    let db = Db::init_in_memory().await.unwrap();
    let service = UserService::new(db);

    service.ensure_default_admin().await.unwrap();

    // Создаем пользователя с временным логином, департаментом и must_change_password = false
    let user_no_pwd_req = service
        .create_user(CreateUserDto {
            username: "op_initial".into(),
            password: "InitPassword123!".into(),
            full_name: Some("Initial Operator".into()),
            email: Some("initial@test.local".into()),
            department: Some("Core Network".into()),
            is_active: Some(true),
            is_superuser: Some(false),
            must_change_password: Some(false),
            roles: Some(vec!["operator".into()]),
        })
        .await
        .unwrap();

    assert_eq!(user_no_pwd_req.department, Some("Core Network".to_string()));
    assert_eq!(user_no_pwd_req.login_count, 0);

    // Выполняем 1-ю аутентификацию при первом входе (login_count становится 1)
    let authenticated = service.authenticate("op_initial", "InitPassword123!").await.unwrap();
    assert_eq!(authenticated.username, "op_initial");

    // В первой сессии смена логина разрешена (первичная настройка аккаунта)
    let updated_user = service
        .update_user(
            user_no_pwd_req.id,
            UpdateUserDto {
                username: Some("op_final".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(updated_user.username, "op_final");
    assert_eq!(updated_user.department, Some("Core Network".to_string()));

    // Выполняем 2-ю аутентификацию под новым логином (login_count становится 2)
    let second_auth = service.authenticate("op_final", "InitPassword123!").await.unwrap();
    assert_eq!(second_auth.username, "op_final");

    // После завершения первичной настройки и повторного входа смена логина навсегда заблокирована
    let after_login_change = service
        .update_user(
            user_no_pwd_req.id,
            UpdateUserDto {
                username: Some("op_forbidden".into()),
                ..Default::default()
            },
        )
        .await;
    assert!(after_login_change.is_err());
}

#[tokio::test]
async fn test_rate_limiting_lockout_and_password_complexity() {
    let db = Db::init_in_memory().await.unwrap();
    let service = UserService::new(db.clone());

    service.ensure_default_admin().await.unwrap();

    // 1. Проверка валидации сложности пароля при создании
    let weak_short = service.create_user(CreateUserDto {
        username: "test_user".into(),
        password: "Ab1".into(),
        ..Default::default()
    }).await;
    assert!(weak_short.is_err(), "Пароль < 8 символов должен быть отклонен");

    let weak_no_upper = service.create_user(CreateUserDto {
        username: "test_user".into(),
        password: "password123!".into(),
        ..Default::default()
    }).await;
    assert!(weak_no_upper.is_err(), "Пароль без заглавных должен быть отклонен");

    let weak_no_digits = service.create_user(CreateUserDto {
        username: "test_user".into(),
        password: "Password!!!!".into(),
        ..Default::default()
    }).await;
    assert!(weak_no_digits.is_err(), "Пароль без цифр должен быть отклонен");

    let weak_no_special = service.create_user(CreateUserDto {
        username: "test_user".into(),
        password: "Password1234".into(),
        ..Default::default()
    }).await;
    assert!(weak_no_special.is_err(), "Пароль без спецсимволов должен быть отклонен");

    // Корректный пароль
    let user = service.create_user(CreateUserDto {
        username: "test_lockout".into(),
        password: "ValidPassword123!".into(),
        is_active: Some(true),
        ..Default::default()
    }).await.unwrap();

    // 2. Тестирование блокировки после 5 неудачных попыток входа
    for i in 1..=4 {
        let auth_res = service.authenticate("test_lockout", "WrongPassword!").await;
        assert!(auth_res.is_err());
        let u = service.get_user_by_id(user.id).await.unwrap();
        assert_eq!(u.failed_login_attempts, i);
        assert!(u.locked_until.is_none());
    }

    // 5-я попытка -> должна наступить блокировка
    let lock_res = service.authenticate("test_lockout", "WrongPassword!").await;
    assert!(lock_res.is_err());
    let u_locked = service.get_user_by_id(user.id).await.unwrap();
    assert_eq!(u_locked.failed_login_attempts, 5);
    assert!(u_locked.locked_until.is_some());

    // 6-я попытка даже с правильным паролем -> отказ из-за блокировки
    let try_correct_while_locked = service.authenticate("test_lockout", "ValidPassword123!").await;
    assert!(try_correct_while_locked.is_err());
    assert!(try_correct_while_locked.unwrap_err().message.contains("locked"));

    // Смена пароля администратором должна сбросить блокировку и попытки
    service.update_user(user.id, UpdateUserDto {
        password: Some("NewValidPassword123!".into()),
        ..Default::default()
    }).await.unwrap();

    let u_unlocked = service.get_user_by_id(user.id).await.unwrap();
    assert_eq!(u_unlocked.failed_login_attempts, 0);
    assert!(u_unlocked.locked_until.is_none());

    // Теперь вход с новым паролем должен пройти успешно
    let auth_ok = service.authenticate("test_lockout", "NewValidPassword123!").await;
    assert!(auth_ok.is_ok());
}

#[tokio::test]
async fn test_user_password_change_verification() {
    let db = Db::init_in_memory().await.unwrap();
    let service = UserService::new(db);

    let user = service
        .create_user(CreateUserDto {
            username: "pwd_test".into(),
            password: "OldPassword123!".into(),
            is_active: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();

    // 1. Попытка смены с неверным current_password -> ошибка
    let err_res = service
        .update_user(
            user.id,
            UpdateUserDto {
                password: Some("NewPassword123!".into()),
                current_password: Some("WrongOldPassword123!".into()),
                ..Default::default()
            },
        )
        .await;
    assert!(err_res.is_err());
    let err = err_res.unwrap_err();
    assert_eq!(err.details["field"], "current_password");

    // 2. Успешная смена с правильным current_password
    let ok_res = service
        .update_user(
            user.id,
            UpdateUserDto {
                password: Some("NewPassword123!".into()),
                current_password: Some("OldPassword123!".into()),
                ..Default::default()
            },
        )
        .await;
    assert!(ok_res.is_ok());

    // 3. Проверка аутентификации с новым паролем
    let auth = service.authenticate("pwd_test", "NewPassword123!").await;
    assert!(auth.is_ok());
}
