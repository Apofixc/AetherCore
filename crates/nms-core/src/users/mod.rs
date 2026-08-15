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
                roles: Some(vec!["admin".to_string()]),
            })
            .await?;
        }

        Ok(())
    }

    /// Создать нового пользователя системы
    ///
    /// Выполняет валидацию логина и пароля, хэширует пароль алгоритмом Argon2id,
    /// сохраняет запись в БД и привязывает указанные роли (по умолчанию `"viewer"`).
    ///
    /// # Аргументы
    /// * `dto` — Данные создаваемого пользователя ([`CreateUserDto`]).
    ///
    /// # Возвращаемое значение
    /// Созданный объект пользователя ([`User`]) с агрегированными правами доступа.
    ///
    /// # Ошибки
    /// - [`AppError::Validation`](nms_common::error::AppError) — если имя пользователя или пароль не удовлетворяют требованиям.
    /// - [`AppError::Conflict`](nms_common::error::AppError) — если пользователь с таким именем уже существует.
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
        .map_err(|e| AppError::database(format!("Failed to insert user: {}", e)))?;

        // Назначаем роли
        let roles = dto.roles.unwrap_or_else(|| vec!["viewer".to_string()]);
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
    ///
    /// Включает загрузку связанных ролей и вычисление агрегированных прав доступа.
    ///
    /// # Аргументы
    /// * `id` — Идентификатор пользователя ([`Uuid`]).
    ///
    /// # Возвращаемое значение
    /// Полный объект [`User`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::NotFound`](nms_common::error::AppError), если пользователь не найден.
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
        .map_err(|e| AppError::database(e.to_string()))?;

        match row {
            Some(r) => self.map_user_row(r).await,
            None => Err(AppError::not_found(format!("User with id '{}'", id))),
        }
    }

    /// Получить пользователя по логину (username)
    ///
    /// Поиск выполняется без учета регистра.
    ///
    /// # Аргументы
    /// * `username` — Имя пользователя для поиска.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::NotFound`](nms_common::error::AppError), если пользователь не найден.
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
        .map_err(|e| AppError::database(e.to_string()))?;

        match row {
            Some(r) => self.map_user_row(r).await,
            None => Err(AppError::not_found(format!("User '{}'", username))),
        }
    }

    /// Выполнить аутентификацию пользователя по логину и паролю
    ///
    /// Проверяет активность аккаунта (`is_active`), сверяет хэш пароля через Argon2id
    /// и обновляет поле `last_login_at`.
    ///
    /// # Аргументы
    /// * `username` — Имя пользователя.
    /// * `password` — Открытый пароль.
    ///
    /// # Возвращаемое значение
    /// Аутентифицированный объект [`User`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Unauthorized`](nms_common::error::AppError), если логин/пароль неверны или аккаунт отключен.
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
    ///
    /// # Возвращаемое значение
    /// Вектор объектов [`User`] в алфавитном порядке имен пользователей.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое запроса.
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
        .map_err(|e| AppError::database(e.to_string()))?;

        let mut users = Vec::with_capacity(rows.len());
        for row in rows {
            users.push(self.map_user_row(row).await?);
        }

        Ok(users)
    }

    /// Обновить профиль, пароль или роли пользователя
    ///
    /// # Аргументы
    /// * `id` — Идентификатор обновляемого пользователя.
    /// * `dto` — Объект с обновляемыми полями ([`UpdateUserDto`]).
    ///
    /// # Возвращаемое значение
    /// Актуальный объект [`User`] после применения изменений.
    ///
    /// # Ошибки
    /// - [`AppError::NotFound`](nms_common::error::AppError) — пользователь не найден.
    /// - [`AppError::Database`](nms_common::error::AppError) — сбой при сохранении в SQLite.
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

    /// Удалить пользователя по его идентификатору
    ///
    /// Все связанные роли каскадно удаляются через внешние ключи SQLite (`ON DELETE CASCADE`).
    ///
    /// # Аргументы
    /// * `id` — Идентификатор пользователя.
    ///
    /// # Возвращаемое значение
    /// `Ok(true)` если пользователь существовал и был удален, иначе `Ok(false)`.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`](nms_common::error::AppError) при сбое запроса.
    pub async fn delete_user(&self, id: Uuid) -> Result<bool> {
        let res = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

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
            roles,
            permissions,
            created_at,
            updated_at,
            last_login_at,
        })
    }
}
