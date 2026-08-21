//! # Сервис управления пользователями и ролями (UserService)
//!
//! Предоставляет CRUD операции над учетными записями пользователей,
//! управление назначением RBAC-ролей, хэширование паролей и проверку учетных данных при аутентификации.

use crate::auth::{hash_password, verify_password};
use crate::db::Db;
use chrono::{DateTime, Utc};
use nms_common::error::{AppError, Result};
use nms_common::models::user::{CreateUserDto, UpdateUserDto, User};
use std::collections::HashSet;
use tracing::info;
use uuid::Uuid;

/// Сервис управления учетными записями пользователей и правами доступа
#[derive(Debug, Clone)]
pub struct UserService {
    db: Db,
}

impl UserService {
    /// Создать новый экземпляр UserService
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных платформы ([`Db`]).
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Проверить наличие пользователей и инициализировать дефолтного администратора (`admin:admin`), если база пуста
    ///
    /// # Ошибки
    /// Возвращает [`AppError`] при сбое запроса к базе данных или создании пользователя.
    pub async fn ensure_default_admin(&self) -> Result<()> {
        let count_row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(self.db.reader())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        if count_row.0 == 0 {
            info!("No users found in database. Initializing default admin user: 'admin'");
            self.create_user(CreateUserDto {
                username: "admin".to_string(),
                password: "admin".to_string(),
                full_name: Some("System Administrator".to_string()),
                email: Some("admin@nms.local".to_string()),
                is_active: Some(true),
                is_superuser: Some(true),
                must_change_password: Some(false),
                roles: Some(vec!["admin".to_string()]),
            })
            .await?;
        }

        Ok(())
    }

    /// Получить количество активных суперпользователей в системе
    pub async fn count_superusers(&self) -> Result<i64> {
        let count_row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_superuser = 1")
            .fetch_one(self.db.reader())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        Ok(count_row.0)
    }

    /// Создать нового пользователя системы
    ///
    /// Выполняет валидацию логина и пароля, проверку квоты суперпользователей (макс 4),
    /// хэширует пароль алгоритмом Argon2id, сохраняет запись в БД и привязывает указанные роли.
    pub async fn create_user(&self, dto: CreateUserDto) -> Result<User> {
        let username = dto.username.trim().to_lowercase();
        if username.is_empty() {
            return Err(AppError::validation("username", "Username cannot be empty"));
        }

        if dto.password.len() < 4 {
            return Err(AppError::validation(
                "password",
                "Password must be at least 4 characters long",
            ));
        }

        // Проверка уникальности имени пользователя
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM users WHERE username = ?")
                .bind(&username)
                .fetch_optional(self.db.reader())
                .await
                .map_err(|e| AppError::database(e.to_string()))?;

        if existing.is_some() {
            return Err(AppError::conflict(format!("User '{}' already exists", username)));
        }

        let is_superuser = dto.is_superuser.unwrap_or(false)
            || dto.roles.as_ref().map_or(false, |r| r.contains(&"superuser".to_string()));

        // Проверка квоты суперпользователей (максимум 4)
        if is_superuser {
            let current_superusers = self.count_superusers().await?;
            if current_superusers >= 4 {
                return Err(AppError::validation(
                    "roles",
                    "Maximum 4 superusers allowed in the system",
                ));
            }
        }

        let id = Uuid::new_v4();
        let password_hash = hash_password(&dto.password)?;
        let now = Utc::now();
        let is_active = dto.is_active.unwrap_or(true);
        let must_change_password = dto.must_change_password.unwrap_or(false);

        // Вставляем пользователя
        sqlx::query(
            r#"
            INSERT INTO users (id, username, full_name, email, password_hash, is_active, is_superuser, must_change_password, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&username)
        .bind(&dto.full_name)
        .bind(&dto.email)
        .bind(&password_hash)
        .bind(if is_active { 1 } else { 0 })
        .bind(if is_superuser { 1 } else { 0 })
        .bind(if must_change_password { 1 } else { 0 })
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(format!("Failed to insert user: {}", e)))?;

        // Назначаем роли
        let roles = dto.roles.unwrap_or_else(|| {
            if is_superuser {
                vec!["superuser".to_string()]
            } else {
                vec!["viewer".to_string()]
            }
        });

        for role in &roles {
            sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role_name) VALUES (?, ?)")
                .bind(id.to_string())
                .bind(role)
                .execute(self.db.writer())
                .await
                .map_err(|e| AppError::database(e.to_string()))?;
        }

        self.get_user_by_id(id).await
    }

    /// Получить пользователя по его уникальному идентификатору (UUID)
    pub async fn get_user_by_id(&self, id: Uuid) -> Result<User> {
        let row: Option<(
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, password_hash, is_active, is_superuser, must_change_password, created_at, updated_at, last_login_at
            FROM users WHERE id = ?
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        match row {
            Some(r) => self.map_user_row(r).await,
            None => Err(AppError::not_found(format!("User with id '{}'", id))),
        }
    }

    /// Получить пользователя по логину (username)
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
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, password_hash, is_active, is_superuser, must_change_password, created_at, updated_at, last_login_at
            FROM users WHERE username = ?
            "#,
        )
        .bind(&username_clean)
        .fetch_optional(self.db.reader())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        match row {
            Some(r) => self.map_user_row(r).await,
            None => Err(AppError::not_found(format!("User '{}'", username))),
        }
    }

    /// Выполнить аутентификацию пользователя по логину и паролю
    pub async fn authenticate(&self, username: &str, password: &str) -> Result<User> {
        let user = self.get_user_by_username(username).await.map_err(|_| {
            AppError::unauthorized("Invalid username or password")
        })?;

        if !user.is_active {
            return Err(AppError::unauthorized("Account is disabled"));
        }

        if !verify_password(password, &user.password_hash)? {
            return Err(AppError::unauthorized("Invalid username or password"));
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

    /// Получить список всех зарегистрированных пользователей системы
    pub async fn list_users(&self) -> Result<Vec<User>> {
        let rows: Vec<(
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, password_hash, is_active, is_superuser, must_change_password, created_at, updated_at, last_login_at
            FROM users ORDER BY username ASC
            "#,
        )
        .fetch_all(self.db.reader())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        let mut users = Vec::with_capacity(rows.len());
        for row in rows {
            users.push(self.map_user_row(row).await?);
        }

        Ok(users)
    }

    /// Обновить профиль, пароль или роли пользователя с проверкой правил безопасности
    pub async fn update_user(&self, id: Uuid, dto: UpdateUserDto) -> Result<User> {
        let existing = self.get_user_by_id(id).await?;
        let now = Utc::now().to_rfc3339();

        // 1. Проверка роли superuser
        let new_is_superuser = dto.is_superuser.unwrap_or_else(|| {
            dto.roles
                .as_ref()
                .map_or(existing.is_superuser, |r| r.contains(&"superuser".to_string()))
        });

        // Запрет блокировки суперпользователя
        if existing.is_superuser && dto.is_active == Some(false) {
            return Err(AppError::validation(
                "is_active",
                "Superusers cannot be deactivated or locked",
            ));
        }

        // Повышение до superuser -> проверка квоты <= 4
        if !existing.is_superuser && new_is_superuser {
            let current_superusers = self.count_superusers().await?;
            if current_superusers >= 4 {
                return Err(AppError::validation(
                    "roles",
                    "Maximum 4 superusers allowed in the system",
                ));
            }
        }

        // Понижение superuser -> проверка квоты >= 1 (нельзя понизить последнего)
        if existing.is_superuser && !new_is_superuser {
            let current_superusers = self.count_superusers().await?;
            if current_superusers <= 1 {
                return Err(AppError::validation(
                    "roles",
                    "Cannot demote the last remaining superuser in the system",
                ));
            }
        }

        let is_password_changed = dto.password.as_ref().map_or(false, |p| !p.trim().is_empty());
        let password_hash = match dto.password {
            Some(ref pwd) if !pwd.trim().is_empty() => hash_password(pwd)?,
            _ => existing.password_hash.clone(),
        };

        // Если пароль сменен, сбрасываем must_change_password в 0 (если не указано обратное)
        let must_change_password = if is_password_changed && dto.must_change_password.is_none() {
            false
        } else {
            dto.must_change_password.unwrap_or(existing.must_change_password)
        };

        let full_name = dto.full_name.or(existing.full_name);
        let email = dto.email.or(existing.email);
        let is_active = dto.is_active.unwrap_or(existing.is_active);

        sqlx::query(
            r#"
            UPDATE users SET full_name = ?, email = ?, password_hash = ?, is_active = ?, is_superuser = ?, must_change_password = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(full_name)
        .bind(email)
        .bind(password_hash)
        .bind(if is_active { 1 } else { 0 })
        .bind(if new_is_superuser { 1 } else { 0 })
        .bind(if must_change_password { 1 } else { 0 })
        .bind(now)
        .bind(id.to_string())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        // Обновляем роли, если переданы
        if let Some(roles) = dto.roles {
            sqlx::query("DELETE FROM user_roles WHERE user_id = ?")
                .bind(id.to_string())
                .execute(self.db.writer())
                .await
                .map_err(|e| AppError::database(e.to_string()))?;

            for role in roles {
                sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role_name) VALUES (?, ?)")
                    .bind(id.to_string())
                    .bind(role)
                    .execute(self.db.writer())
                    .await
                    .map_err(|e| AppError::database(e.to_string()))?;
            }
        }

        self.get_user_by_id(id).await
    }

    /// Удалить пользователя по его идентификатору с проверкой защиты последнего суперпользователя
    pub async fn delete_user(&self, id: Uuid) -> Result<bool> {
        let existing = self.get_user_by_id(id).await?;
        if existing.is_superuser {
            let current_superusers = self.count_superusers().await?;
            if current_superusers <= 1 {
                return Err(AppError::validation(
                    "id",
                    "Cannot delete the last remaining superuser in the system",
                ));
            }
        }

        let res = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    /// Преобразовать кортеж строки таблицы `users` в модель [`User`]
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
            must_change_pwd_num,
            created_at_str,
            updated_at_str,
            last_login_at_str,
        ) = r;

        let id = Uuid::parse_str(&id_str).unwrap_or_default();
        let is_active = is_active_num != 0;
        let is_superuser = is_superuser_num != 0;
        let must_change_password = must_change_pwd_num != 0;

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
                .map_err(|e| AppError::database(e.to_string()))?;

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
            .map_err(|e| AppError::database(e.to_string()))?;

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
            must_change_password,
            roles,
            permissions,
            created_at,
            updated_at,
            last_login_at,
        })
    }
}
