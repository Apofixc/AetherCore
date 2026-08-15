//! # Сервис управления пользователями и ролями (UserService)

use crate::auth::{hash_password, verify_password};
use crate::db::Db;
use chrono::{DateTime, Utc};
use nms_common::error::{AppError, Result};
use nms_common::models::user::{CreateUserDto, UpdateUserDto, User};
use std::collections::HashSet;
use tracing::info;
use uuid::Uuid;

/// Сервис для работы с пользователями
#[derive(Debug, Clone)]
pub struct UserService {
    db: Db,
}

impl UserService {
    /// Создать новый экземпляр сервиса пользователей
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Инициализировать дефолтного администратора, если в базе нет ни одного пользователя
    pub async fn ensure_default_admin(&self) -> Result<()> {
        let count_row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(self.db.reader())
            .await
            .map_err(|e| AppError::Database {
                details: e.to_string(),
            })?;

        if count_row.0 == 0 {
            info!("No users found in database. Initializing default admin user: 'admin'");
            self.create_user(CreateUserDto {
                username: "admin".to_string(),
                password: "admin".to_string(),
                full_name: Some("System Administrator".to_string()),
                email: Some("admin@nms.local".to_string()),
                is_active: Some(true),
                is_superuser: Some(true),
                roles: Some(vec!["admin".to_string()]),
            })
            .await?;
        }

        Ok(())
    }

    /// Создать нового пользователя
    pub async fn create_user(&self, dto: CreateUserDto) -> Result<User> {
        let username = dto.username.trim().to_lowercase();
        if username.is_empty() {
            return Err(AppError::Validation {
                field: "username".into(),
                details: "Username cannot be empty".into(),
            });
        }

        if dto.password.len() < 4 {
            return Err(AppError::Validation {
                field: "password".into(),
                details: "Password must be at least 4 characters long".into(),
            });
        }

        // Проверка уникальности имени пользователя
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM users WHERE username = ?")
                .bind(&username)
                .fetch_optional(self.db.reader())
                .await
                .map_err(|e| AppError::Database {
                    details: e.to_string(),
                })?;

        if existing.is_some() {
            return Err(AppError::Conflict {
                details: format!("User '{}' already exists", username),
            });
        }

        let id = Uuid::new_v4();
        let password_hash = hash_password(&dto.password)?;
        let now = Utc::now();
        let is_active = dto.is_active.unwrap_or(true);
        let is_superuser = dto.is_superuser.unwrap_or(false);

        // Вставляем пользователя
        sqlx::query(
            r#"
            INSERT INTO users (id, username, full_name, email, password_hash, is_active, is_superuser, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&username)
        .bind(&dto.full_name)
        .bind(&dto.email)
        .bind(&password_hash)
        .bind(if is_active { 1 } else { 0 })
        .bind(if is_superuser { 1 } else { 0 })
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::Database {
            details: format!("Failed to insert user: {}", e),
        })?;

        // Назначаем роли
        let roles = dto.roles.unwrap_or_else(|| vec!["viewer".to_string()]);
        for role in &roles {
            sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role_name) VALUES (?, ?)")
                .bind(id.to_string())
                .bind(role)
                .execute(self.db.writer())
                .await
                .map_err(|e| AppError::Database {
                    details: e.to_string(),
                })?;
        }

        self.get_user_by_id(id).await
    }

    /// Получить пользователя по ID
    pub async fn get_user_by_id(&self, id: Uuid) -> Result<User> {
        let row: Option<(
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, password_hash, is_active, is_superuser, created_at, updated_at, last_login_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| AppError::Database {
            details: e.to_string(),
        })?;

        match row {
            Some(r) => self.map_user_row(r).await,
            None => Err(AppError::NotFound {
                resource: format!("User with id '{}'", id),
            }),
        }
    }

    /// Получить пользователя по имени пользователя
    pub async fn get_user_by_username(&self, username: &str) -> Result<User> {
        let username_clean = username.trim().to_lowercase();
        let row: Option<(
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, password_hash, is_active, is_superuser, created_at, updated_at, last_login_at
            FROM users WHERE username = ?
            "#,
        )
        .bind(&username_clean)
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| AppError::Database {
            details: e.to_string(),
        })?;

        match row {
            Some(r) => self.map_user_row(r).await,
            None => Err(AppError::NotFound {
                resource: format!("User '{}'", username),
            }),
        }
    }

    /// Аутентификация пользователя по логину и паролю
    pub async fn authenticate(&self, username: &str, password: &str) -> Result<User> {
        let user = self.get_user_by_username(username).await.map_err(|_| {
            AppError::Unauthorized {
                details: "Invalid username or password".into(),
            }
        })?;

        if !user.is_active {
            return Err(AppError::Unauthorized {
                details: "Account is disabled".into(),
            });
        }

        if !verify_password(password, &user.password_hash)? {
            return Err(AppError::Unauthorized {
                details: "Invalid username or password".into(),
            });
        }

        // Обновляем время последнего входа
        let now = Utc::now().to_rfc3339();
        let _ = sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
            .bind(now)
            .bind(user.id.to_string())
            .execute(self.db.writer())
            .await;

        Ok(user)
    }

    /// Получить список всех пользователей
    pub async fn list_users(&self) -> Result<Vec<User>> {
        let rows: Vec<(
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, password_hash, is_active, is_superuser, created_at, updated_at, last_login_at
            FROM users ORDER BY username ASC
            "#,
        )
        .fetch_all(self.db.reader())
        .await
        .map_err(|e| AppError::Database {
            details: e.to_string(),
        })?;

        let mut users = Vec::with_capacity(rows.len());
        for row in rows {
            users.push(self.map_user_row(row).await?);
        }

        Ok(users)
    }

    /// Обновить данные пользователя
    pub async fn update_user(&self, id: Uuid, dto: UpdateUserDto) -> Result<User> {
        let existing = self.get_user_by_id(id).await?;
        let now = Utc::now().to_rfc3339();

        let password_hash = match dto.password {
            Some(ref pwd) if !pwd.is_empty() => hash_password(pwd)?,
            _ => existing.password_hash.clone(),
        };

        let full_name = dto.full_name.or(existing.full_name);
        let email = dto.email.or(existing.email);
        let is_active = dto.is_active.unwrap_or(existing.is_active);
        let is_superuser = dto.is_superuser.unwrap_or(existing.is_superuser);

        sqlx::query(
            r#"
            UPDATE users SET full_name = ?, email = ?, password_hash = ?, is_active = ?, is_superuser = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(full_name)
        .bind(email)
        .bind(password_hash)
        .bind(if is_active { 1 } else { 0 })
        .bind(if is_superuser { 1 } else { 0 })
        .bind(now)
        .bind(id.to_string())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::Database {
            details: e.to_string(),
        })?;

        // Обновляем роли, если переданы
        if let Some(roles) = dto.roles {
            sqlx::query("DELETE FROM user_roles WHERE user_id = ?")
                .bind(id.to_string())
                .execute(self.db.writer())
                .await
                .map_err(|e| AppError::Database {
                    details: e.to_string(),
                })?;

            for role in roles {
                sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role_name) VALUES (?, ?)")
                    .bind(id.to_string())
                    .bind(role)
                    .execute(self.db.writer())
                    .await
                    .map_err(|e| AppError::Database {
                        details: e.to_string(),
                    })?;
            }
        }

        self.get_user_by_id(id).await
    }

    /// Удалить пользователя
    pub async fn delete_user(&self, id: Uuid) -> Result<bool> {
        let res = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::Database {
                details: e.to_string(),
            })?;

        Ok(res.rows_affected() > 0)
    }

    /// Вспомогательный маппинг строки БД в структуру User с подгрузкой ролей и прав
    async fn map_user_row(
        &self,
        r: (
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            String,
            String,
            Option<String>,
        ),
    ) -> Result<User> {
        let (
            id_str,
            username,
            full_name,
            email,
            password_hash,
            is_active_num,
            is_superuser_num,
            created_at_str,
            updated_at_str,
            last_login_at_str,
        ) = r;

        let id = Uuid::parse_str(&id_str).unwrap_or_default();
        let is_active = is_active_num != 0;
        let is_superuser = is_superuser_num != 0;

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let last_login_at = last_login_at_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        // Загрузка ролей
        let role_rows: Vec<(String,)> =
            sqlx::query_as("SELECT role_name FROM user_roles WHERE user_id = ?")
                .bind(&id_str)
                .fetch_all(self.db.reader())
                .await
                .map_err(|e| AppError::Database {
                    details: e.to_string(),
                })?;

        let roles: Vec<String> = role_rows.into_iter().map(|r| r.0).collect();

        // Загрузка прав по ролям
        let mut permissions_set = HashSet::new();
        for role in &roles {
            let perm_rows: Vec<(String,)> = sqlx::query_as(
                "SELECT permission_id FROM role_permissions WHERE role_name = ?",
            )
            .bind(role)
            .fetch_all(self.db.reader())
            .await
            .map_err(|e| AppError::Database {
                details: e.to_string(),
            })?;

            for p in perm_rows {
                permissions_set.insert(p.0);
            }
        }

        let mut permissions: Vec<String> = permissions_set.into_iter().collect();
        permissions.sort();

        Ok(User {
            id,
            username,
            full_name,
            email,
            password_hash,
            is_active,
            is_superuser,
            roles,
            permissions,
            created_at,
            updated_at,
            last_login_at,
        })
    }
}
