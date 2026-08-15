// Сервис журналирования аудита безопасности и действий пользователей NMS

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use tracing::{error, info};

/// Модель записи в журнале аудита
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: String,
    pub user_id: Option<String>,
    pub username: String,
    pub action: String,
    pub resource: String,
    pub details: Option<String>,
    pub ip_address: Option<String>,
}

/// Записать событие аудита в БД
pub async fn log_audit_event(
    pool: &Pool<Sqlite>,
    user_id: Option<&str>,
    username: &str,
    action: &str,
    resource: &str,
    details: Option<&str>,
    ip_address: Option<&str>,
) -> Result<()> {
    match sqlx::query(
        "INSERT INTO audit_logs (user_id, username, action, resource, details, ip_address)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(username)
    .bind(action)
    .bind(resource)
    .bind(details)
    .bind(ip_address)
    .execute(pool)
    .await
    {
        Ok(_) => Ok(()),
        Err(err) => {
            error!("Failed to write audit log entry: {}", err);
            Err(err.into())
        }
    }
}

/// Очистка (ротация) устаревших записей аудита
/// Удаляет записи старше `max_days` дней или при превышении лимита `max_records`
pub async fn rotate_audit_logs(
    pool: &Pool<Sqlite>,
    max_days: u32,
    max_records: u64,
) -> Result<u64> {
    let mut total_deleted = 0u64;

    // 1. Удаление записей старше max_days дней
    let days_stmt = format!("-{} days", max_days);
    let res_days = sqlx::query("DELETE FROM audit_logs WHERE timestamp < datetime('now', ?)")
        .bind(days_stmt)
        .execute(pool)
        .await?;

    total_deleted += res_days.rows_affected();

    // 2. Ограничение количества записей до max_records
    let res_limit = sqlx::query(
        "DELETE FROM audit_logs WHERE id NOT IN (
            SELECT id FROM audit_logs ORDER BY id DESC LIMIT ?
        )",
    )
    .bind(max_records as i64)
    .execute(pool)
    .await?;

    total_deleted += res_limit.rows_affected();

    if total_deleted > 0 {
        info!("Rotated audit logs: removed {} old entries", total_deleted);
    }

    Ok(total_deleted)
}
