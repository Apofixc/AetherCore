//! # Тесты сервиса резервного копирования и восстановления (BackupService)

use aethercore_core::db::Db;
use aethercore_core::services::backup::BackupService;
use tempfile::TempDir;

#[tokio::test]
async fn test_backup_create_list_validate_restore_prune_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_main.db");
    let backups_dir = temp_dir.path().join("backups");

    // 1. Инициализируем БД на диске
    let db = Db::init(&db_path, 5, 5000).await.expect("DB init failed");

    // Вставляем тестовые данные в kv_store
    let kv = aethercore_core::db::kv::KvStore::system(db.clone());
    kv.set("test_key", &"initial_value").await.unwrap();

    let backup_svc = BackupService::new(db.clone(), backups_dir.clone());

    // 2. Создаем первый бэкап
    let backup1 = backup_svc.create_backup("test1").await.expect("Create backup failed");
    assert_eq!(backup1.tag, "test1");
    assert!(backup1.size_bytes > 0);
    assert!(backup1.is_valid);

    // 3. Проверяем список бэкапов
    let list = backup_svc.list_backups().await.expect("List backups failed");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].filename, backup1.filename);

    // 4. Проверяем валидацию файла
    let backup_path = backup_svc.get_backup_path(&backup1.filename).unwrap();
    let is_valid = backup_svc.validate_backup_file(&backup_path).await.unwrap();
    assert!(is_valid);

    // 5. Меняем данные в БД
    kv.set("test_key", &"modified_value").await.unwrap();
    let val: Option<String> = kv.get("test_key").await.unwrap();
    assert_eq!(val, Some("modified_value".to_string()));

    // 6. Восстанавливаем из первого бэкапа
    let restore_res = backup_svc
        .restore_from_backup_file(&backup_path)
        .await
        .expect("Restore failed");
    assert!(restore_res.success);
    assert!(restore_res.pre_restore_backup.is_some());

    // Проверяем, что значение вернулось к "initial_value"
    let val_restored: Option<String> = kv.get("test_key").await.unwrap();
    assert_eq!(val_restored, Some("initial_value".to_string()));

    // 7. Проверяем статистику хранилища
    let stats = db.get_storage_stats().await.expect("Get storage stats failed");
    assert!(stats.db_size_bytes > 0);
    assert!(stats.tables_count >= 5);

    // 8. Удаление бэкапа
    backup_svc.delete_backup(&backup1.filename).await.expect("Delete backup failed");
    let after_delete = backup_svc.list_backups().await.unwrap();
    // Остался только pre_restore
    assert!(!after_delete.iter().any(|b| b.filename == backup1.filename));
}
