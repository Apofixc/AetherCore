//! # Тесты подсистемы базы данных SQLite и KV хранилища

use aethercore_core::db::kv::KvStore;
use aethercore_core::db::Db;

#[tokio::test]
async fn test_database_init_in_memory() {
    let db = Db::init_in_memory().await.expect("DB in memory init failed");

    // Проверяем, что стандартные роли созданы (superuser, admin, operator, viewer)
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM roles")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(row.0, 4);

    // Проверяем, что права созданы
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM permissions")
        .fetch_one(db.reader())
        .await
        .unwrap();
    assert_eq!(row.0, 15);
}

#[tokio::test]
async fn test_kv_store_crud_and_isolation() {
    let db = Db::init_in_memory().await.unwrap();

    let store_a = KvStore::for_plugin(db.clone(), "plugin_a");
    let store_b = KvStore::for_plugin(db.clone(), "plugin_b");

    // Запись в store_a
    store_a.set("interval", &10).await.unwrap();
    let val_a: Option<i32> = store_a.get("interval").await.unwrap();
    assert_eq!(val_a, Some(10));

    // Проверяем изоляцию: в store_b этого ключа нет
    let val_b: Option<i32> = store_b.get("interval").await.unwrap();
    assert_eq!(val_b, None);

    // Список ключей
    store_a.set("token", &"secret".to_string()).await.unwrap();
    let keys = store_a.list_keys().await.unwrap();
    assert_eq!(keys, vec!["interval", "token"]);

    // Удаление
    assert!(store_a.delete("interval").await.unwrap());
    let deleted: Option<i32> = store_a.get("interval").await.unwrap();
    assert_eq!(deleted, None);
}
