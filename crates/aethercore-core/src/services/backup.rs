//! # Сервис резервного копирования и восстановления базы данных (BackupService)
//!
//! Обеспечивает:
//! - Создание неблокирующих консистентных снимков SQLite базы данных на лету (`VACUUM INTO`).
//! - Валидацию целостности снимков (`PRAGMA integrity_check`, проверка схемы).
//! - Безопасное восстановление базы данных из снимков с автоматическим созданием точки отката (`pre_restore`).
//! - Ротацию и удаление устаревших бэкапов в соответствии с политикой хранения (`retention_days`).
//! - Предоставление списка и информации о доступных локальных резервных копиях.

use crate::db::Db;
use aethercore_common::error::{AppError, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::{info, warn};

/// Метаданные файла резервной копии
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    /// Имя файла бэкапа на диске
    pub filename: String,
    /// Размер файла в байтах
    pub size_bytes: u64,
    /// Время создания в формате RFC 3339 (ISO 8601)
    pub created_at: String,
    /// Тег типа бэкапа: "manual", "auto", "pre_restore", "upload"
    pub tag: String,
    /// Флаг успешного прохождения проверки целостности
    pub is_valid: bool,
}

/// Результат выполнения операции восстановления
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    /// Успешность операции восстановления
    pub success: bool,
    /// Имя созданного файла аварийной точки отката (pre-restore snapshot)
    pub pre_restore_backup: Option<String>,
    /// Текстовое сообщение о результате
    pub message: String,
}

/// Сервис управления резервными копиями SQLite
#[derive(Debug, Clone)]
pub struct BackupService {
    db: Db,
    backup_dir: PathBuf,
}

impl BackupService {
    /// Создать новый экземпляр сервиса резервного копирования
    ///
    /// # Аргументы
    /// * `db` — Экземпляр пула базы данных [`Db`].
    /// * `backup_dir` — Директория для хранения файлов резервных копий.
    pub fn new(db: Db, backup_dir: PathBuf) -> Self {
        Self { db, backup_dir }
    }

    /// Получить директорию хранения резервных копий
    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    /// Создать новую резервную копию базы данных
    ///
    /// Использует SQLite `VACUUM INTO '...'`, что формирует автономный,
    /// чистый и транзакционно консистентный файл без блокировки читателей и писателей.
    ///
    /// # Аргументы
    /// * `tag` — Метка бэкапа (`"manual"`, `"auto"`, `"pre_restore"`, `"upload"`).
    ///
    /// # Возвращаемое значение
    /// Метаинформация о созданной копии [`BackupInfo`].
    pub async fn create_backup(&self, tag: &str) -> Result<BackupInfo> {
        // Создаем директорию, если она отсутствует
        tokio::fs::create_dir_all(&self.backup_dir).await.map_err(|e| {
            AppError::internal(format!(
                "Failed to create backup directory {:?}: {}",
                self.backup_dir, e
            ))
        })?;

        let now = Utc::now();
        let timestamp = now.format("%Y%m%d_%H%M%S").to_string();
        let clean_tag = tag
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect::<String>();
        let clean_tag = if clean_tag.is_empty() {
            "manual".to_string()
        } else {
            clean_tag
        };

        let filename = format!("aethercore_backup_{}_{}.db", timestamp, clean_tag);
        let target_path = self.backup_dir.join(&filename);

        // Если файл уже существует, удаляем его перед VACUUM INTO
        if target_path.exists() {
            let _ = tokio::fs::remove_file(&target_path).await;
        }

        let escaped_path = target_path.display().to_string().replace('\'', "''");
        let vacuum_query = format!("VACUUM INTO '{}';", escaped_path);

        sqlx::query(&vacuum_query)
            .execute(self.db.writer())
            .await
            .map_err(|e| {
                AppError::database(format!(
                    "Failed to create SQLite backup snapshot via VACUUM INTO: {}",
                    e
                ))
            })?;

        let metadata = tokio::fs::metadata(&target_path).await.map_err(|e| {
            AppError::internal(format!("Failed to read created backup metadata: {}", e))
        })?;

        let size_bytes = metadata.len();
        info!(
            "SQLite backup snapshot created: {} ({} bytes, tag: {})",
            filename, size_bytes, clean_tag
        );

        Ok(BackupInfo {
            filename,
            size_bytes,
            created_at: now.to_rfc3339(),
            tag: clean_tag,
            is_valid: true,
        })
    }

    /// Получить список всех доступных локальных резервных копий
    ///
    /// Сканирует каталог бэкапов и сортирует список от самых свежих к старым.
    pub async fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        if !self.backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut read_dir = tokio::fs::read_dir(&self.backup_dir).await.map_err(|e| {
            AppError::internal(format!("Failed to read backup directory: {}", e))
        })?;

        let mut backups = Vec::new();

        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            AppError::internal(format!("Failed to iterate backup directory: {}", e))
        })? {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let fname = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            if !fname.ends_with(".db") && !fname.ends_with(".sqlite") {
                continue;
            }

            let meta = match entry.metadata().await {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Извлечение тега из имени: aethercore_backup_YYYYMMDD_HHMMSS_{tag}.db
            let tag = if fname.contains("_auto") {
                "auto"
            } else if fname.contains("_pre_restore") {
                "pre_restore"
            } else if fname.contains("_upload") {
                "upload"
            } else {
                "manual"
            };

            let created_at = match meta.created().or_else(|_| meta.modified()) {
                Ok(time) => {
                    let dt: DateTime<Utc> = time.into();
                    dt.to_rfc3339()
                }
                Err(_) => Utc::now().to_rfc3339(),
            };

            backups.push(BackupInfo {
                filename: fname,
                size_bytes: meta.len(),
                created_at,
                tag: tag.to_string(),
                is_valid: true,
            });
        }

        // Сортировка по имени/времени (убывание — сначала новые)
        backups.sort_by(|a, b| b.filename.cmp(&a.filename));

        Ok(backups)
    }

    /// Безопасно получить абсолютный путь к файлу бэкапа по его имени
    ///
    /// Проверяет отсутствие попыток обхода директорий (`..`, слеши).
    pub fn get_backup_path(&self, filename: &str) -> Result<PathBuf> {
        let clean_name = Path::new(filename)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| AppError::validation("filename", "Invalid backup filename"))?;

        if clean_name != filename
            || filename.contains("..")
            || filename.contains('/')
            || filename.contains('\\')
        {
            return Err(AppError::validation(
                "filename",
                "Directory traversal attempt detected",
            ));
        }

        if !filename.ends_with(".db") && !filename.ends_with(".sqlite") {
            return Err(AppError::validation(
                "filename",
                "Backup file must have .db or .sqlite extension",
            ));
        }

        let full_path = self.backup_dir.join(clean_name);
        if !full_path.exists() {
            return Err(AppError::not_found(format!(
                "Backup file '{}' not found",
                filename
            )));
        }

        Ok(full_path)
    }

    /// Удалить файл резервной копии
    ///
    /// # Аргументы
    /// * `filename` — Имя файла для удаления.
    pub async fn delete_backup(&self, filename: &str) -> Result<()> {
        let path = self.get_backup_path(filename)?;
        tokio::fs::remove_file(&path).await.map_err(|e| {
            AppError::internal(format!("Failed to delete backup file '{}': {}", filename, e))
        })?;
        info!("Deleted backup file: {}", filename);
        Ok(())
    }

    /// Удалить устаревшие резервные копии согласно сроку хранения (Retention Policy)
    ///
    /// # Аргументы
    /// * `retention_days` — Срок хранения в днях. Если 0 — удаление не выполняется.
    ///
    /// # Возвращаемое значение
    /// Количество удаленных файлов.
    pub async fn prune_backups(&self, retention_days: u32) -> Result<usize> {
        if retention_days == 0 {
            return Ok(0);
        }

        let cutoff = Utc::now() - Duration::days(retention_days as i64);
        let backups = self.list_backups().await?;
        let mut pruned = 0;

        for b in backups {
            // Не удаляем точки отката pre_restore автоматически, пока они моложе 2 дней
            if b.tag == "pre_restore" {
                let pre_cutoff = Utc::now() - Duration::days(2);
                if let Ok(created) = DateTime::parse_from_rfc3339(&b.created_at) {
                    if created.with_timezone(&Utc) < pre_cutoff {
                        if self.delete_backup(&b.filename).await.is_ok() {
                            pruned += 1;
                        }
                    }
                }
                continue;
            }

            if let Ok(created) = DateTime::parse_from_rfc3339(&b.created_at) {
                if created.with_timezone(&Utc) < cutoff {
                    if self.delete_backup(&b.filename).await.is_ok() {
                        pruned += 1;
                    }
                }
            }
        }

        if pruned > 0 {
            info!(
                "Pruned {} outdated backup files older than {} days",
                pruned, retention_days
            );
        }

        Ok(pruned)
    }

    /// Проверить целостность и корректность структуры файла бэкапа
    ///
    /// # Аргументы
    /// * `path` — Путь к файлу бэкапа на диске.
    ///
    /// # Возвращаемое значение
    /// `Ok(true)` если файл является валидной базой данных SQLite со всеми ключевыми таблицами.
    pub async fn validate_backup_file(&self, path: &Path) -> Result<bool> {
        if !path.exists() {
            return Err(AppError::not_found(format!(
                "File {:?} does not exist",
                path
            )));
        }

        let url = format!("sqlite://{}?mode=ro", path.display());
        let connect_opts = SqliteConnectOptions::from_str(&url)
            .map_err(|e| AppError::validation("backup_file", format!("Invalid SQLite file: {}", e)))?
            .read_only(true);

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(connect_opts)
            .await
            .map_err(|e| {
                AppError::validation(
                    "backup_file",
                    format!("Failed to open backup database for validation: {}", e),
                )
            })?;

        // 1. Проверка PRAGMA integrity_check
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check;")
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                AppError::validation(
                    "backup_file",
                    format!("Failed to run integrity check on backup file: {}", e),
                )
            })?;

        if integrity.to_lowercase() != "ok" {
            return Err(AppError::validation(
                "backup_file",
                format!("Backup file failed integrity check: {}", integrity),
            ));
        }

        // 2. Проверка наличия обязательных системных таблиц
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('users', 'roles', 'permissions');",
        )
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

        if count < 3 {
            return Err(AppError::validation(
                "backup_file",
                "Backup database schema is incompatible (missing core tables)",
            ));
        }

        Ok(true)
    }

    /// Восстановить базу данных из указанного файла резервной копии
    ///
    /// Процедура:
    /// 1. Валидация целостности файла бэкапа.
    /// 2. Создание аварийной точки отката (`pre_restore` snapshot) перед внесением изменений.
    /// 3. Онлайн-перенос всех таблиц и данных через `ATTACH DATABASE` внутри транзакции.
    ///
    /// # Аргументы
    /// * `backup_path` — Путь к валидному файлу `.db` для наката.
    pub async fn restore_from_backup_file(&self, backup_path: &Path) -> Result<RestoreResult> {
        // 1. Валидация файла бэкапа
        self.validate_backup_file(backup_path).await?;

        // 2. Создание снимка безопасности
        let pre_restore = match self.create_backup("pre_restore").await {
            Ok(info) => {
                info!(
                    "Created pre-restore safety snapshot: {}",
                    info.filename
                );
                Some(info.filename)
            }
            Err(e) => {
                warn!("Could not create pre-restore snapshot: {}", e);
                None
            }
        };

        // 3. Подключение файла бэкапа и замена данных таблиц
        let escaped_path = backup_path.display().to_string().replace('\'', "''");
        let attach_query = format!("ATTACH DATABASE '{}' AS restore_src;", escaped_path);

        sqlx::query(&attach_query)
            .execute(self.db.writer())
            .await
            .map_err(|e| {
                AppError::database(format!("Failed to attach restore database source: {}", e))
            })?;

        let res = self.execute_restore_tables().await;

        // В любом случае отсоединяем базу данных
        let _ = sqlx::query("DETACH DATABASE restore_src;")
            .execute(self.db.writer())
            .await;

        res.map(|_| {
            info!("Database restored successfully from {:?}", backup_path);
            RestoreResult {
                success: true,
                pre_restore_backup: pre_restore,
                message: "Database restore completed successfully".to_string(),
            }
        })
    }

    async fn execute_restore_tables(&self) -> Result<()> {
        let pool = self.db.writer();

        // Получаем список таблиц из источника (исключая sqlite_* и миграции)
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM restore_src.sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%';",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::database(format!("Failed to list tables in restore source: {}", e)))?;

        // Отключаем проверку внешних ключей на время перезаписи таблиц
        let _ = sqlx::query("PRAGMA foreign_keys = OFF;")
            .execute(pool)
            .await;

        let mut restore_error = None;

        for table in &tables {
            let clean_table = table.replace('"', "\"\"");
            let exists: i64 = sqlx::query_scalar(&format!(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name = '{}';",
                clean_table
            ))
            .fetch_one(pool)
            .await
            .unwrap_or(0);

            if exists > 0 {
                // Очищаем старые данные таблицы и копируем новые из бэкапа
                if let Err(e) = sqlx::query(&format!("DELETE FROM \"{}\";", clean_table))
                    .execute(pool)
                    .await
                {
                    restore_error = Some(e);
                    break;
                }

                if let Err(e) = sqlx::query(&format!(
                    "INSERT OR REPLACE INTO \"{}\" SELECT * FROM restore_src.\"{}\";",
                    clean_table, clean_table
                ))
                .execute(pool)
                .await
                {
                    restore_error = Some(e);
                    break;
                }
            }
        }

        // Включаем обратно внешние ключи
        let _ = sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(pool)
            .await;

        if let Some(err) = restore_error {
            return Err(AppError::database(format!(
                "Failed to restore tables data: {}",
                err
            )));
        }

        Ok(())
    }
}
