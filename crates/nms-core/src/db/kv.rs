//! # Изолированное Key-Value хранилище (KV Store)
//!
//! Предоставляет ядру и плагинам безопасное хранилище настроек и состояния
//! с автоматической изоляцией неймспейсов `module:{id}:{key}` или `system:{key}`.

use super::Db;
use chrono::Utc;
use nms_common::error::{AppError, Result};
use serde::{de::DeserializeOwned, Serialize};

/// Сервис изолированного Key-Value хранилища
#[derive(Debug, Clone)]
pub struct KvStore {
    db: Db,
    namespace: String,
}

impl KvStore {
    /// Создать KV-хранилище для указанного пространства имен
    pub fn new(db: Db, namespace: impl Into<String>) -> Self {
        Self {
            db,
            namespace: namespace.into(),
        }
    }

    /// Создать KV-хранилище для системных настроек ядра
    pub fn system(db: Db) -> Self {
        Self::new(db, "system")
    }

    /// Создать KV-хранилище для конкретного плагина
    pub fn for_plugin(db: Db, plugin_id: &str) -> Self {
        Self::new(db, format!("module:{}", plugin_id))
    }

    /// Получить типизированное значение по ключу
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value_json FROM kv_store WHERE namespace = ? AND key = ?",
        )
        .bind(&self.namespace)
        .bind(key)
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| AppError::Database {
            details: format!("Failed to read KV key '{}:{}': {}", self.namespace, key, e),
        })?;

        match row {
            Some((json_str,)) => {
                let val: T = serde_json::from_str(&json_str).map_err(|e| AppError::Validation {
                    field: key.to_string(),
                    details: format!("JSON deserialization failed: {}", e),
                })?;
                Ok(Some(val))
            }
            None => Ok(None),
        }
    }

    /// Сохранить значение по ключу (UPSERT)
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json_str = serde_json::to_string(value).map_err(|e| AppError::Validation {
            field: key.to_string(),
            details: format!("JSON serialization failed: {}", e),
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
        .map_err(|e| AppError::Database {
            details: format!("Failed to write KV key '{}:{}': {}", self.namespace, key, e),
        })?;

        Ok(())
    }

    /// Удалить значение по ключу
    pub async fn delete(&self, key: &str) -> Result<bool> {
        let res = sqlx::query("DELETE FROM kv_store WHERE namespace = ? AND key = ?")
            .bind(&self.namespace)
            .bind(key)
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::Database {
                details: format!("Failed to delete KV key '{}:{}': {}", self.namespace, key, e),
            })?;

        Ok(res.rows_affected() > 0)
    }

    /// Получить все ключи в текущем пространстве имен
    pub async fn list_keys(&self) -> Result<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT key FROM kv_store WHERE namespace = ? ORDER BY key ASC")
                .bind(&self.namespace)
                .fetch_all(self.db.reader())
                .await
                .map_err(|e| AppError::Database {
                    details: format!("Failed to list KV keys for '{}': {}", self.namespace, e),
                })?;

        Ok(rows.into_iter().map(|r| r.0).collect())
    }
}
