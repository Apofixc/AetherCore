// Unit-тесты для сервисов базы данных (db.rs) и журналов аудита (audit.rs)

use nms_core::{
    get_system_setting, init_db_pool, log_audit_event, rotate_audit_logs, set_system_setting,
    verify_password, AuditLogEntry,
};
use tokio::fs;

#[tokio::test]
async fn test_db_init_and_tables() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join("nms_test_storage.db");

    let pool = init_db_pool(&db_path).await.unwrap();

    // Проверка создания системной роли role-admin
    let role_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM roles WHERE id = 'role-admin'")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(role_count.0, 1);

    let _ = fs::remove_file(db_path).await;
}

#[tokio::test]
async fn test_db_seed_roles_permissions_and_root_user() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join("nms_test_seed.db");

    let pool = init_db_pool(&db_path).await.unwrap();

    // 1. Проверка наличия 5 ролей (1, 2, 3, 4, role-admin)
    let total_roles: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total_roles.0, 5);

    // 2. Проверка наличия 12 системных разрешений
    let total_perms: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM permissions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total_perms.0, 12);

    // 3. Проверка назначения прав роли 1 (Superuser) — все 12 прав
    let superuser_perms: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM role_permissions WHERE role_id = '1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(superuser_perms.0, 12);

    // 4. Проверка существования пользователя root и валидности его Argon2 пароля 'admin'
    let root_user: (String, String) =
        sqlx::query_as("SELECT username, hashed_password FROM users WHERE username = 'root'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(root_user.0, "root");
    assert!(verify_password("admin", &root_user.1));

    let _ = fs::remove_file(db_path).await;
}

#[tokio::test]
async fn test_system_settings_get_set() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join("nms_test_settings.db");

    let pool = init_db_pool(&db_path).await.unwrap();

    // Проверка отсутствующего ключа
    let non_existent = get_system_setting(&pool, "site_name").await.unwrap();
    assert_eq!(non_existent, None);

    // Сохранение и получение значения
    set_system_setting(&pool, "site_name", "NMS Enterprise Portal")
        .await
        .unwrap();
    let val = get_system_setting(&pool, "site_name").await.unwrap();
    assert_eq!(val, Some("NMS Enterprise Portal".to_string()));

    // Обновление значения (UPSERT)
    set_system_setting(&pool, "site_name", "NMS Updated Portal")
        .await
        .unwrap();
    let val_updated = get_system_setting(&pool, "site_name").await.unwrap();
    assert_eq!(val_updated, Some("NMS Updated Portal".to_string()));

    let _ = fs::remove_file(db_path).await;
}

#[tokio::test]
async fn test_audit_logging_and_rotation() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join("nms_test_audit.db");

    let pool = init_db_pool(&db_path).await.unwrap();

    // Запись нескольких событий аудита
    log_audit_event(
        &pool,
        Some("usr-01"),
        "admin",
        "user.create",
        "user",
        Some("Created new operator user"),
        Some("127.0.0.1"),
    )
    .await
    .unwrap();

    log_audit_event(
        &pool,
        Some("usr-01"),
        "admin",
        "system.config",
        "settings",
        Some("Changed port to 8080"),
        Some("127.0.0.1"),
    )
    .await
    .unwrap();

    // Проверка чтения записей из audit_logs через прямые SQL-запросы
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs;")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 2);

    let entries: Vec<AuditLogEntry> = sqlx::query_as(
        "SELECT id, timestamp, user_id, username, action, resource, details, ip_address FROM audit_logs ORDER BY id DESC LIMIT 10;",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].action, "system.config"); // Новое событие первым (ORDER BY id DESC)
    assert_eq!(entries[1].action, "user.create");

    // Тестирование ротации записей (ограничиваем ровно 1 запись)
    let deleted = rotate_audit_logs(&pool, 90, 1).await.unwrap();
    assert_eq!(deleted, 1);

    let remaining: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs;")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining.0, 1);

    let _ = fs::remove_file(db_path).await;
}

#[tokio::test]
async fn test_db_module_1to1_api_compatibility() {
    let temp_dir = std::env::temp_dir();
    let db_path = temp_dir.join("nms_test_1to1_api.db");

    // Проверка работы alias-функции init_db
    let _pool = nms_core::db::init_db(&db_path).await.unwrap();

    // Проверка работы re-exported hash_password и verify_password из модуля db
    let pass = "secret_pass_123";
    let hashed = nms_core::db::hash_password(pass).unwrap();
    assert!(nms_core::db::verify_password(pass, &hashed));
    assert!(!nms_core::db::verify_password("wrong_pass", &hashed));

    let _ = fs::remove_file(db_path).await;
}
