//! # Сервис глобальных сессий операторов (SessionService)
//!
//! Обеспечивает персистентное хранение, онлайн-валидацию, трекинг активности
//! и принудительный отзыв сессий операторов в SQLite.

use crate::db::Db;
use aethercore_common::error::{AppError, Result};
use aethercore_common::models::user::SessionRecord;
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

/// Сервис управления глобальными сессиями операторов
#[derive(Debug, Clone)]
pub struct SessionService {
    db: Db,
}

impl SessionService {
    /// Создать новый экземпляр [`SessionService`]
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных платформы ([`Db`]).
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Зарегистрировать новую активную сессию оператора
    ///
    /// # Аргументы
    /// * `user_id` — ID пользователя ([`Uuid`]).
    /// * `username` — Логин оператора.
    /// * `roles` — Список назначенных ролей.
    /// * `ip_address` — IP-адрес клиента.
    /// * `user_agent` — Заголовок User-Agent или идентификатор клиента.
    /// * `ttl_seconds` — Время жизни сессии в секундах.
    pub async fn create_session(
        &self,
        user_id: Uuid,
        username: &str,
        roles: &[String],
        ip_address: &str,
        user_agent: &str,
        ttl_seconds: i64,
    ) -> Result<SessionRecord> {
        let session_id = Uuid::new_v4();
        let now = Utc::now();
        let expires_at = now + Duration::seconds(ttl_seconds.max(60));
        let roles_json = serde_json::to_string(roles).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO active_sessions (
                id, user_id, username, roles, ip_address, user_agent, created_at, last_active_at, expires_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(session_id.to_string())
        .bind(user_id.to_string())
        .bind(username)
        .bind(&roles_json)
        .bind(ip_address)
        .bind(user_agent)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(expires_at.to_rfc3339())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(format!("Failed to insert active session: {}", e)))?;

        Ok(SessionRecord {
            id: session_id,
            user_id,
            username: username.to_string(),
            roles: roles.to_vec(),
            ip_address: ip_address.to_string(),
            user_agent: user_agent.to_string(),
            created_at: now,
            last_active_at: now,
            expires_at,
        })
    }

    /// Получить данные сессии по идентификатору
    pub async fn get_session(&self, session_id: Uuid) -> Result<Option<SessionRecord>> {
        type SessionRow = (String, String, String, String, String, String, String, String, String);

        let row: Option<SessionRow> = sqlx::query_as(
            r#"
            SELECT id, user_id, username, roles, ip_address, user_agent, created_at, last_active_at, expires_at
            FROM active_sessions
            WHERE id = ?
            "#,
        )
        .bind(session_id.to_string())
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| AppError::database(format!("Failed to fetch session: {}", e)))?;

        match row {
            Some((id_str, user_id_str, username, roles_raw, ip_address, user_agent, created_str, last_active_str, expires_str)) => {
                let id = Uuid::parse_str(&id_str).map_err(|e| AppError::internal(e.to_string()))?;
                let user_id = Uuid::parse_str(&user_id_str).map_err(|e| AppError::internal(e.to_string()))?;
                let roles: Vec<String> = serde_json::from_str(&roles_raw).unwrap_or_default();
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let last_active_at = DateTime::parse_from_rfc3339(&last_active_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let expires_at = DateTime::parse_from_rfc3339(&expires_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(SessionRecord {
                    id,
                    user_id,
                    username,
                    roles,
                    ip_address,
                    user_agent,
                    created_at,
                    last_active_at,
                    expires_at,
                }))
            }
            None => Ok(None),
        }
    }

    /// Проверить, активна ли сессия и не истек ли срок ее действия
    pub async fn is_session_valid(&self, session_id: Uuid) -> Result<bool> {
        let now_rfc = Utc::now().to_rfc3339();

        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT id FROM active_sessions
            WHERE id = ? AND expires_at > ?
            "#,
        )
        .bind(session_id.to_string())
        .bind(&now_rfc)
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| AppError::database(format!("Failed to validate session: {}", e)))?;

        Ok(row.is_some())
    }

    /// Обновить время последней активности сессии
    pub async fn touch_session(&self, session_id: Uuid) -> Result<()> {
        let now_rfc = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE active_sessions
            SET last_active_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&now_rfc)
        .bind(session_id.to_string())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(format!("Failed to touch session: {}", e)))?;

        Ok(())
    }

    /// Получить список всех актуальных (неистекших) сессий операторов
    pub async fn list_active_sessions(&self) -> Result<Vec<SessionRecord>> {
        type SessionRow = (String, String, String, String, String, String, String, String, String);
        let now_rfc = Utc::now().to_rfc3339();

        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT id, user_id, username, roles, ip_address, user_agent, created_at, last_active_at, expires_at
            FROM active_sessions
            WHERE expires_at > ?
            ORDER BY last_active_at DESC
            "#,
        )
        .bind(&now_rfc)
        .fetch_all(self.db.reader())
        .await
        .map_err(|e| AppError::database(format!("Failed to list active sessions: {}", e)))?;

        let mut list = Vec::with_capacity(rows.len());
        for (id_str, user_id_str, username, roles_raw, ip_address, user_agent, created_str, last_active_str, expires_str) in rows {
            let id = Uuid::parse_str(&id_str).unwrap_or_default();
            let user_id = Uuid::parse_str(&user_id_str).unwrap_or_default();
            let roles: Vec<String> = serde_json::from_str(&roles_raw).unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let last_active_at = DateTime::parse_from_rfc3339(&last_active_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let expires_at = DateTime::parse_from_rfc3339(&expires_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            list.push(SessionRecord {
                id,
                user_id,
                username,
                roles,
                ip_address,
                user_agent,
                created_at,
                last_active_at,
                expires_at,
            });
        }

        Ok(list)
    }

    /// Отозвать (принудительно завершить) конкретную сессию по ID
    pub async fn revoke_session(&self, session_id: Uuid) -> Result<bool> {
        let res = sqlx::query("DELETE FROM active_sessions WHERE id = ?")
            .bind(session_id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(format!("Failed to revoke session: {}", e)))?;

        Ok(res.rows_affected() > 0)
    }

    /// Отозвать все сессии, кроме указанной (сбросить чужие сессии)
    pub async fn revoke_all_except(&self, except_session_id: Uuid) -> Result<usize> {
        let res = sqlx::query("DELETE FROM active_sessions WHERE id != ?")
            .bind(except_session_id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(format!("Failed to revoke other sessions: {}", e)))?;

        Ok(res.rows_affected() as usize)
    }

    /// Принудительно завершить абсолютно все сессии
    pub async fn revoke_all_sessions(&self) -> Result<usize> {
        let res = sqlx::query("DELETE FROM active_sessions")
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(format!("Failed to revoke all sessions: {}", e)))?;

        Ok(res.rows_affected() as usize)
    }

    /// Получить список активных сессий конкретного пользователя
    pub async fn list_user_sessions(&self, user_id: Uuid) -> Result<Vec<SessionRecord>> {
        type SessionRow = (String, String, String, String, String, String, String, String, String);
        let now_rfc = Utc::now().to_rfc3339();

        let rows: Vec<SessionRow> = sqlx::query_as(
            r#"
            SELECT id, user_id, username, roles, ip_address, user_agent, created_at, last_active_at, expires_at
            FROM active_sessions
            WHERE user_id = ? AND expires_at > ?
            ORDER BY last_active_at DESC
            "#,
        )
        .bind(user_id.to_string())
        .bind(&now_rfc)
        .fetch_all(self.db.reader())
        .await
        .map_err(|e| AppError::database(format!("Failed to list user active sessions: {}", e)))?;

        let mut list = Vec::with_capacity(rows.len());
        for (id_str, user_id_str, username, roles_raw, ip_address, user_agent, created_str, last_active_str, expires_str) in rows {
            let id = Uuid::parse_str(&id_str).unwrap_or_default();
            let user_id = Uuid::parse_str(&user_id_str).unwrap_or_default();
            let roles: Vec<String> = serde_json::from_str(&roles_raw).unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let last_active_at = DateTime::parse_from_rfc3339(&last_active_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let expires_at = DateTime::parse_from_rfc3339(&expires_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            list.push(SessionRecord {
                id,
                user_id,
                username,
                roles,
                ip_address,
                user_agent,
                created_at,
                last_active_at,
                expires_at,
            });
        }

        Ok(list)
    }

    /// Отозвать все сессии конкретного пользователя, кроме указанной
    pub async fn revoke_user_sessions_except(&self, user_id: Uuid, except_session_id: Uuid) -> Result<usize> {
        let res = sqlx::query("DELETE FROM active_sessions WHERE user_id = ? AND id != ?")
            .bind(user_id.to_string())
            .bind(except_session_id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(format!("Failed to revoke user other sessions: {}", e)))?;

        Ok(res.rows_affected() as usize)
    }

    /// Отозвать все сессии конкретного пользователя
    pub async fn revoke_user_sessions(&self, user_id: Uuid) -> Result<usize> {
        let res = sqlx::query("DELETE FROM active_sessions WHERE user_id = ?")
            .bind(user_id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(format!("Failed to revoke user sessions: {}", e)))?;

        Ok(res.rows_affected() as usize)
    }

    /// Очистить истекшие сессии
    pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
        let now_rfc = Utc::now().to_rfc3339();

        let res = sqlx::query("DELETE FROM active_sessions WHERE expires_at <= ?")
            .bind(&now_rfc)
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(format!("Failed to cleanup expired sessions: {}", e)))?;

        Ok(res.rows_affected() as usize)
    }
}
