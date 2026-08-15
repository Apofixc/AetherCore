//! # Сервис журнала аудита действий (AuditService)
//!
//! Фиксирует все важные действия пользователей и системных процессов (вход, создание/удаление устройств,
//! смена конфигураций) для соответствия требованиям безопасности.

use crate::db::Db;
use chrono::{DateTime, Utc};
use nms_common::error::{AppError, Result};
use serde::{Deserialize, Serialize};

/// Запись журнала аудита безопасности и системных действий
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogRecord {
    /// Уникальный автоинкрементный идентификатор записи
    pub id: i64,
    /// ID пользователя, совершившего действие (если применимо)
    pub user_id: Option<String>,
    /// Имя пользователя
    pub username: Option<String>,
    /// Выполненное действие (например, `"user.login"`, `"device.create"`)
    pub action: String,
    /// Целевой ресурс (например, `"auth"`, `"device:192.168.1.1"`)
    pub resource: String,
    /// Статус операции (`"success"`, `"failed"`)
    pub status: String,
    /// Дополнительные детали в свободном формате или JSON
    pub details: Option<String>,
    /// IP-адрес источника запроса
    pub ip_address: Option<String>,
    /// Временная метка фиксации события (UTC)
    pub created_at: DateTime<Utc>,
}

/// Сервис для работы с журналом аудита
#[derive(Debug, Clone)]
pub struct AuditService {
    db: Db,
}

impl AuditService {
    /// Создать новый экземпляр AuditService
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных платформы ([`Db`]).
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Записать действие пользователя или системы в журнал аудита
    ///
    /// # Аргументы
    /// * `user_id` — Опциональный идентификатор пользователя.
    /// * `username` — Опциональное имя пользователя.
    /// * `action` — Идентификатор действия (например, `"auth.login"`).
    /// * `resource` — Затронутый ресурс (например, `"system"`).
    /// * `status` — Результат выполнения (`"success"`, `"forbidden"` и т.д.).
    /// * `details` — Опциональное подробное описание или контекст.
    /// * `ip_address` — IP-адрес клиента.
    ///
    /// # Возвращаемое значение
    /// ID добавленной записи аудита.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое записи в SQLite.
    pub async fn log(
        &self,
        user_id: Option<&str>,
        username: Option<&str>,
        action: &str,
        resource: &str,
        status: &str,
        details: Option<&str>,
        ip_address: Option<&str>,
    ) -> Result<i64> {
        let now = Utc::now().to_rfc3339();

        let res = sqlx::query(
            r#"
            INSERT INTO audit_logs (user_id, username, action, resource, status, details, ip_address, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(username)
        .bind(action)
        .bind(resource)
        .bind(status)
        .bind(details)
        .bind(ip_address)
        .bind(now)
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        Ok(res.last_insert_rowid())
    }

    /// Получить список записей журнала аудита
    pub async fn list_logs(&self, limit: u32, after_id: Option<i64>) -> Result<Vec<AuditLogRecord>> {
        let limit = limit.min(500).max(1) as i64;
        let after_id = after_id.unwrap_or(0);

        let rows: Vec<(
            i64,
            Option<String>,
            Option<String>,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            String,
        )> = sqlx::query_as(
            r#"
            SELECT id, user_id, username, action, resource, status, details, ip_address, created_at
            FROM audit_logs
            WHERE id > ?
            ORDER BY id DESC
            LIMIT ?
            "#,
        )
        .bind(after_id)
        .bind(limit)
        .fetch_all(self.db.reader())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        let mut records = Vec::with_capacity(rows.len());
        for (id, u_id, u_name, action, res, status, details, ip, created_str) in rows {
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            records.push(AuditLogRecord {
                id,
                user_id: u_id,
                username: u_name,
                action,
                resource: res,
                status,
                details,
                ip_address: ip,
                created_at,
            });
        }

        Ok(records)
    }
}
