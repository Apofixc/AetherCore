//! # Сервис журнала аудита действий (AuditService)
//!
//! Фиксирует все важные действия пользователей и системных процессов (вход, создание/удаление устройств,
//! смена конфигураций, изменение прав) для соответствия требованиям безопасности и расследования инцидентов.

use crate::db::Db;
use chrono::{DateTime, Utc};
use aethercore_common::error::{AppError, Result};
use serde::{Deserialize, Serialize};

/// Запись журнала аудита безопасности и системных действий
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogRecord {
    /// Уникальный автоинкрементный идентификатор записи
    pub id: i64,
    /// ID пользователя, совершившего действие (если применимо)
    pub user_id: Option<String>,
    /// Имя пользователя (логин)
    pub username: Option<String>,
    /// Выполненное действие (например, `"auth.login"`, `"users.create"`, `"modules.enable"`)
    pub action: String,
    /// Целевой ресурс (например, `"auth"`, `"users/42"`, `"modules/snmp"`)
    pub resource: String,
    /// Статус операции (`"success"`, `"failed"`, `"forbidden"`)
    pub status: String,
    /// Дополнительные детали в свободном формате или JSON
    pub details: Option<String>,
    /// IP-адрес источника запроса
    pub ip_address: Option<String>,
    /// Временная метка фиксации события (UTC)
    pub created_at: DateTime<Utc>,
}

/// Сервис для персистентной фиксации и чтения журнала аудита безопасности
#[derive(Debug, Clone)]
pub struct AuditService {
    db: Db,
}

impl AuditService {
    /// Создать новый экземпляр [`AuditService`]
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
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое записи в SQLite.
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

    /// Получить общее количество записей журнала аудита с учетом опционального поиска
    ///
    /// # Аргументы
    /// * `search` — Опциональная поисковая подстрока.
    ///
    /// # Возвращаемое значение
    /// Общее количество соответствующих записей.
    pub async fn count_logs(&self, search: Option<&str>) -> Result<i64> {
        if let Some(q) = search.filter(|s| !s.trim().is_empty()) {
            let pattern = format!("%{}%", q.trim());
            let count: (i64,) = sqlx::query_as(
                r#"
                SELECT COUNT(*)
                FROM audit_logs
                WHERE username LIKE ? OR action LIKE ? OR resource LIKE ? OR details LIKE ? OR ip_address LIKE ?
                "#,
            )
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .fetch_one(self.db.reader())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
            Ok(count.0)
        } else {
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs")
                .fetch_one(self.db.reader())
                .await
                .map_err(|e| AppError::database(e.to_string()))?;
            Ok(count.0)
        }
    }

    /// Получить список записей журнала аудита с пагинацией и поиском
    ///
    /// # Аргументы
    /// * `limit` — Максимальное количество возвращаемых записей (ограничивается диапазоном `1..=500`).
    /// * `offset` — Смещение для постраничной пагинации.
    /// * `search` — Опциональная строка поиска по пользователю, действию, ресурсу, деталям или IP.
    ///
    /// # Возвращаемое значение
    /// Вектор записей журнала аудита [`AuditLogRecord`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](aethercore_common::error::AppError) при сбое запроса к SQLite.
    pub async fn list_logs(
        &self,
        limit: u32,
        offset: Option<u64>,
        search: Option<&str>,
    ) -> Result<Vec<AuditLogRecord>> {
        let limit = limit.min(500).max(1) as i64;
        let offset = offset.unwrap_or(0) as i64;

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
        )> = if let Some(q) = search.filter(|s| !s.trim().is_empty()) {
            let pattern = format!("%{}%", q.trim());
            sqlx::query_as(
                r#"
                SELECT id, user_id, username, action, resource, status, details, ip_address, created_at
                FROM audit_logs
                WHERE username LIKE ? OR action LIKE ? OR resource LIKE ? OR details LIKE ? OR ip_address LIKE ?
                ORDER BY id DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.reader())
            .await
            .map_err(|e| AppError::database(e.to_string()))?
        } else {
            sqlx::query_as(
                r#"
                SELECT id, user_id, username, action, resource, status, details, ip_address, created_at
                FROM audit_logs
                ORDER BY id DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.reader())
            .await
            .map_err(|e| AppError::database(e.to_string()))?
        };

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
