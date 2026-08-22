//! # Стресс-тестирование и проверка атак на систему безопасности (Penetration / Break Tests)
//!
//! Проверяет устойчивость к попыткам эскалации привилегий, взлома квот,
//! самоблокировки, удаления администраторов и обхода обязательной смены пароля.

use nms_common::models::user::{CreateUserDto, UpdateUserDto};
use nms_core::db::Db;
use nms_core::users::UserService;

#[tokio::test]
async fn test_attack_scenarios_and_security_edge_cases() {
    let db = Db::init_in_memory().await.unwrap();
    let service = UserService::new(db);

    // 1. Инициализируем систему (admin:admin)
    service.ensure_default_admin().await.unwrap();
    let root_admin = service.get_user_by_username("admin").await.unwrap();
    assert!(root_admin.is_superuser);

    // -------------------------------------------------------------------------
    // Атака 1: Попытка создать дубликат пользователя в разном регистре ('ADMIN', 'Admin')
    // -------------------------------------------------------------------------
    let dup_res = service
        .create_user(CreateUserDto {
            username: "ADMIN".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(false),
            roles: Some(vec!["viewer".into()]),
            ..Default::default()
        })
        .await;
    assert!(dup_res.is_err(), "Должно блокировать дубликаты без учета регистра");

    // -------------------------------------------------------------------------
    // Атака 2: Создание пользователя с коротким паролем (< 4 символов) или пустым логином
    // -------------------------------------------------------------------------
    let short_pwd = service
        .create_user(CreateUserDto {
            username: "hacker".into(),
            password: "123".into(),
            is_active: Some(true),
            is_superuser: Some(false),
            roles: Some(vec!["viewer".into()]),
            ..Default::default()
        })
        .await;
    assert!(short_pwd.is_err(), "Пароли короче 4 символов должны отклоняться");

    let empty_user = service
        .create_user(CreateUserDto {
            username: "   ".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(false),
            roles: Some(vec!["viewer".into()]),
            ..Default::default()
        })
        .await;
    assert!(empty_user.is_err(), "Пустой логин должен отклоняться");

    // -------------------------------------------------------------------------
    // Атака 3: Попытка заблокировать суперпользователя (is_active = false)
    // -------------------------------------------------------------------------
    let lock_root = service
        .update_user(
            root_admin.id,
            UpdateUserDto {
                is_active: Some(false),
                ..Default::default()
            },
        )
        .await;
    assert!(lock_root.is_err(), "Суперпользователя запрещено блокировать");

    // -------------------------------------------------------------------------
    // Атака 4: Попытка удалить единственного суперпользователя
    // -------------------------------------------------------------------------
    let del_root = service.delete_user(root_admin.id).await;
    assert!(del_root.is_err(), "Нельзя удалить единственного суперпользователя");

    // -------------------------------------------------------------------------
    // Атака 5: Попытка понизить роль единственного суперпользователя до operator
    // -------------------------------------------------------------------------
    let demote_root = service
        .update_user(
            root_admin.id,
            UpdateUserDto {
                is_superuser: Some(false),
                roles: Some(vec!["operator".into()]),
                ..Default::default()
            },
        )
        .await;
    assert!(demote_root.is_err(), "Нельзя понизить единственного суперпользователя");

    // -------------------------------------------------------------------------
    // Атака 6: Взлом квоты суперпользователей (попытка создать больше 4)
    // -------------------------------------------------------------------------
    let s2 = service
        .create_user(CreateUserDto {
            username: "super_2".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(true),
            roles: Some(vec!["superuser".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    let s3 = service
        .create_user(CreateUserDto {
            username: "super_3".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(true),
            roles: Some(vec!["superuser".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    let s4 = service
        .create_user(CreateUserDto {
            username: "super_4".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(true),
            roles: Some(vec!["superuser".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(service.count_superusers().await.unwrap(), 4);

    // 5-й суперпользователь -> ошибка
    let s5_attack = service
        .create_user(CreateUserDto {
            username: "super_5".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(true),
            roles: Some(vec!["superuser".into()]),
            ..Default::default()
        })
        .await;
    assert!(s5_attack.is_err(), "5-й суперпользователь не должен создаваться");

    // -------------------------------------------------------------------------
    // Атака 7: Попытка обхода квоты через повышение роли обычного пользователя
    // -------------------------------------------------------------------------
    let regular_user = service
        .create_user(CreateUserDto {
            username: "regular_user".into(),
            password: "Password123!".into(),
            is_active: Some(true),
            is_superuser: Some(false),
            must_change_password: Some(true),
            roles: Some(vec!["viewer".into()]),
            ..Default::default()
        })
        .await
        .unwrap();

    let promote_attack = service
        .update_user(
            regular_user.id,
            UpdateUserDto {
                roles: Some(vec!["superuser".into()]),
                ..Default::default()
            },
        )
        .await;
    assert!(promote_attack.is_err(), "Повышение при квоте 4 должно блокироваться");

    // -------------------------------------------------------------------------
    // Атака 8: Проверка сброса флага must_change_password
    // -------------------------------------------------------------------------
    assert!(regular_user.must_change_password);

    // Пользователь меняет свой пароль
    let user_pwd_changed = service
        .update_user(
            regular_user.id,
            UpdateUserDto {
                password: Some("NewSecretPassword2026!".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // Флаг must_change_password должен автоматически сброситься в false
    assert!(!user_pwd_changed.must_change_password, "Флаг смены пароля должен стать false");

    // -------------------------------------------------------------------------
    // Атака 9: Удаление суперпользователя при наличии других суперпользователей (> 1)
    // -------------------------------------------------------------------------
    let del_s4_res = service.delete_user(s4.id).await;
    assert!(del_s4_res.is_ok(), "Удаление суперпользователя разрешено, если осталось > 1");
    assert_eq!(service.count_superusers().await.unwrap(), 3);

    // Очистка остальных
    service.delete_user(s3.id).await.unwrap();
    service.delete_user(s2.id).await.unwrap();
    assert_eq!(service.count_superusers().await.unwrap(), 1);

    // Снова проверяем, что последний суперпользователь неудаляем
    assert!(service.delete_user(root_admin.id).await.is_err());
}
