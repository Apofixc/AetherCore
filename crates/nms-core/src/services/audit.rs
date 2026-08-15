//! # Сервис журнала аудита действий (AuditService)

use crate::db::Db;
use chrono::{DateTime, Utc};
use nms_common::error::{AppError, Result};
use serde::{Deserialize, Serialize};

/// Запись журнала аудита
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogRecord {
    pub id: i64,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub action: String,
    pub resource: String,
    pub status: String,
    pub details: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Сервис для работы с журналом аудита
#[derive(Debug, Clone)]
pub struct AuditService {
    db: Db,
}

impl AuditService {
    /// Создать новый экземпляр AuditService
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Записать действие пользователя или системы в журнал аудита
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
