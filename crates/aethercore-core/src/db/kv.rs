//! # Изолированное Key-Value хранилище (KV Store)
//!
//! Предоставляет ядру и плагинам безопасное хранилище настроек и состояния
//! с автоматической изоляцией неймспейсов `module:{id}:{key}` или `system:{key}`.

use super::Db;
use chrono::Utc;
use nms_common::error::{AppError, Result};
use serde::{de::DeserializeOwned, Serialize};

/// Сервис изолированного Key-Value хранилища
///
/// Все операции автоматически привязаны к заданному пространству имен (`namespace`),
/// что предотвращает случайную перезапись данных между плагинами и системными настройками.
#[derive(Debug, Clone)]
pub struct KvStore {
    db: Db,
    namespace: String,
}

impl KvStore {
    /// Создать экземпляр KV-хранилища для указанного пространства имен
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных SQLite ([`Db`]).
    /// * `namespace` — Идентификатор пространства имен (например, `"system"` или `"module:ping"`).
    pub fn new(db: Db, namespace: impl Into<String>) -> Self {
        Self {
            db,
            namespace: namespace.into(),
        }
    }

    /// Создать KV-хранилище для глобальных системных настроек ядра (`"system"`)
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных SQLite ([`Db`]).
    pub fn system(db: Db) -> Self {
        Self::new(db, "system")
    }

    /// Создать изолированное KV-хранилище для плагина (`"module:{plugin_id}"`)
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных SQLite ([`Db`]).
    /// * `plugin_id` — Уникальный строковый идентификатор плагина (например, `"snmp-collector"`).
    pub fn for_plugin(db: Db, plugin_id: &str) -> Self {
        Self::new(db, format!("module:{}", plugin_id))
    }

    /// Получить десериализованное значение по ключу
    ///
    /// # Аргументы
    /// * `key` — Ключ в рамках текущего пространства имен.
    ///
    /// # Возвращаемое значение
    /// - `Ok(Some(T))` — если ключ найден и успешно десериализован из JSON.
    /// - `Ok(None)` — если ключ отсутствует в базе данных.
    ///
    /// # Ошибки
    /// - [`AppError::Database`](nms_common::error::AppError) — при ошибке выполнения SQL-запроса.
    /// - [`AppError::Validation`](nms_common::error::AppError) — если JSON-строку невозможно десериализовать в тип `T`.
    ///
    /// # Примеры
    /// ```rust,no_run
    /// use nms_core::db::{Db, kv::KvStore};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Config { timeout_ms: u64 }
    ///
    /// # async fn test(db: Db) -> Result<(), Box<dyn std::error::Error>> {
    /// let store = KvStore::system(db);
    /// let cfg: Option<Config> = store.get("network_config").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value_json FROM kv_store WHERE namespace = ? AND key = ?",
        )
        .bind(&self.namespace)
        .bind(key)
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| {
            AppError::database(format!("Failed to read KV key '{}:{}': {}", self.namespace, key, e))
        })?;

        match row {
            Some((json_str,)) => {
                let val: T = serde_json::from_str(&json_str).map_err(|e| {
                    AppError::validation(key, format!("JSON deserialization failed: {}", e))
                })?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    /// Сохранить сериализуемое значение по ключу (UPSERT)
    ///
    /// Если ключ уже существует в текущем `namespace`, его значение и поле `updated_at` обновляются.
    ///
    /// # Аргументы
    /// * `key` — Ключ в рамках текущего пространства имен.
    /// * `value` — Сериализуемый в JSON объект или примитив.
    ///
    /// # Ошибки
    /// - [`AppError::Validation`](nms_common::error::AppError) — при ошибке сериализации объекта в JSON.
    /// - [`AppError::Database`](nms_common::error::AppError) — при ошибке записи в базу данных.
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json_str = serde_json::to_string(value).map_err(|e| {
            AppError::validation(key, format!("JSON serialization failed: {}", e))
        })?;

        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO kv_store (namespace, key, value_json, updated_at)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (namespace, key) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&self.namespace)
        .bind(key)
        .bind(json_str)
        .bind(now)
        .execute(self.db.writer())
        .await
        .map_err(|e| {
            AppError::database(format!("Failed to write KV key '{}:{}': {}", self.namespace, key, e))
        })?;

        Ok(())
    }

    /// Удалить запись по ключу
    ///
    /// # Аргументы
    /// * `key` — Ключ для удаления.
    ///
    /// # Возвращаемое значение
    /// Возвращает `Ok(true)`, если запись существовала и была удалена, иначе `Ok(false)`.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое запроса к БД.
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let res = sqlx::query("DELETE FROM kv_store WHERE namespace = ? AND key = ?")
            .bind(&self.namespace)
            .bind(key)
            .execute(self.db.writer())
            .await
            .map_err(|e| {
                AppError::database(format!("Failed to delete KV key '{}:{}': {}", self.namespace, key, e))
            })?;

        Ok(res.rows_affected() > 0)
    }

    /// Получить отсортированный список всех ключей в текущем пространстве имен
    ///
    /// # Возвращаемое значение
    /// Вектор строковых ключей в алфавитном порядке.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое выполнения запроса.
    pub async fn list_keys(&self) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT key FROM kv_store WHERE namespace = ? ORDER BY key ASC")
                .bind(&self.namespace)
                .fetch_all(self.db.reader())
                .await
                .map_err(|e| {
                    AppError::database(format!("Failed to list KV keys for '{}': {}", self.namespace, e))
                })?;

        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
