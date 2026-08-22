//! # Сервис управления пользователями и ролями (UserService)
//!
//! Предоставляет CRUD операции над учетными записями пользователей,
//! управление назначением RBAC-ролей, хэширование паролей алгоритмом Argon2id,
//! контроль лимитов суперпользователей, правила защиты от случайной блокировки (Anti-Lockout)
//! и проверку учетных данных при аутентификации.

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
    /// Создать новый экземпляр [`UserService`]
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных платформы ([`Db`]).
    ///
    /// # Возвращаемое значение
    /// Экземпляр сервиса пользователей.
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Проверить наличие пользователей и инициализировать дефолтного администратора (`admin:admin`), если база пуста
    ///
    /// # Ошибки
    /// Возвращает [`AppError`] при сбое запроса к базе данных или ошибке хэширования пароля.
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
                department: Some("Network Operations".to_string()),
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
    ///
    /// Используется для проверки квот (максимум 4 и минимум 1 суперпользователь).
    ///
    /// # Возвращаемое значение
    /// Количество пользователей с флагом `is_superuser = 1`.
    ///
    /// # Ошибки
    /// Возвращает [`AppError`] при ошибке чтения из базы данных.
    pub async fn count_superusers(&self) -> Result<i64> {
        let count_row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_superuser = 1")
            .fetch_one(self.db.reader())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        Ok(count_row.0)
    }

    /// Создать нового пользователя системы
    ///
    /// Выполняет валидацию логина и пароля, проверку квоты суперпользователей (максимум 4),
    /// хэширует пароль алгоритмом Argon2id, сохраняет запись в БД и привязывает указанные роли.
    ///
    /// # Аргументы
    /// * `dto` — Параметры создания пользователя ([`CreateUserDto`]).
    ///
    /// # Возвращаемое значение
    /// Созданная и сохраненная в базе модель [`User`] с назначенными ролями и правами.
    ///
    /// # Ошибки
    /// * [`AppError::validation`] — если логин пустой, пароль короче 4 символов или превышена квота суперпользователей (макс. 4).
    /// * [`AppError::conflict`] — если пользователь с таким логином уже существует в системе.
    /// * [`AppError::database`] — при сбое выполнения SQL-запроса.
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
        let must_change_password = match dto.must_change_password {
            Some(val) => val,
            None => {
                let kv = crate::db::kv::KvStore::system(self.db.clone());
                if let Ok(Some(policies)) = kv.get::<serde_json::Value>("security_policies").await {
                    policies.get("mandatory_password_change").and_then(|v| v.as_bool()).unwrap_or(true)
                } else {
                    true
                }
            }
        };

        // Вставляем пользователя
        sqlx::query(
            r#"
            INSERT INTO users (id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, login_count, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)
            "#,
        )
        .bind(id.to_string())
        .bind(&username)
        .bind(&dto.full_name)
        .bind(&dto.email)
        .bind(&dto.department)
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
    ///
    /// # Аргументы
    /// * `id` — Уникальный идентификатор пользователя ([`Uuid`]).
    ///
    /// # Возвращаемое значение
    /// Модель [`User`] с ролями и правами.
    ///
    /// # Ошибки
    /// * [`AppError::not_found`] — если пользователь с указанным ID не найден.
    /// * [`AppError::database`] — при сбое запроса к базе данных.
    pub async fn get_user_by_id(&self, id: Uuid) -> Result<User> {
        let row: Option<(
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            i64,
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, login_count, created_at, updated_at, last_login_at
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

    /// Получить пользователя по логину (username) без учета регистра
    ///
    /// # Аргументы
    /// * `username` — Имя пользователя для поиска.
    ///
    /// # Возвращаемое значение
    /// Модель [`User`] с ролями и правами.
    ///
    /// # Ошибки
    /// * [`AppError::not_found`] — если пользователь с таким логином не существует.
    /// * [`AppError::database`] — при сбое запроса к базе данных.
    pub async fn get_user_by_username(&self, username: &str) -> Result<User> {
        let username_clean = username.trim().to_lowercase();
        let row: Option<(
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            i64,
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, login_count, created_at, updated_at, last_login_at
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
    /// Проверяет наличие пользователя, статус активности аккаунта (`is_active`)
    /// и совпадение пароля с Argon2id хэшем. При успехе обновляет поле `last_login_at` и увеличивает `login_count`.
    ///
    /// # Аргументы
    /// * `username` — Имя пользователя.
    /// * `password` — Открытый пароль для проверки.
    ///
    /// # Возвращаемое значение
    /// Аутентифицированная модель [`User`].
    ///
    /// # Ошибки
    /// * [`AppError::unauthorized`] — если учетные данные неверны или аккаунт отключен.
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

        // Обновляем время последнего входа и счетчик логинов
        let now = Utc::now().to_rfc3339();
        let _ = sqlx::query("UPDATE users SET last_login_at = ?, login_count = login_count + 1 WHERE id = ?")
            .bind(now)
            .bind(user.id.to_string())
            .execute(self.db.writer())
            .await;

        Ok(user)
    }

    /// Получить список всех зарегистрированных пользователей системы
    ///
    /// # Возвращаемое значение
    /// Вектор всех учетных записей [`User`], отсортированных по имени пользователя.
    ///
    /// # Ошибки
    /// Возвращает [`AppError`] при сбое запроса к базе данных.
    pub async fn list_users(&self) -> Result<Vec<User>> {
        let rows: Vec<(
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            i64,
            i64,
            i64,
            i64,
            String,
            String,
            Option<String>,
        )> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, login_count, created_at, updated_at, last_login_at
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
    ///
    /// Реализует проверки:
    /// 1. Запрет деактивации/блокировки суперпользователя (`Anti-Lockout`).
    /// 2. Контроль квоты суперпользователей $\le 4$ при назначении роли `superuser`.
    /// 3. Запрет понижения роли единственного оставшегося суперпользователя ($\ge 1$).
    /// 4. Автоматический сброс флага `must_change_password` при установке нового пароля.
    ///
    /// # Аргументы
    /// * `id` — Идентификатор пользователя ([`Uuid`]).
    /// * `dto` — Параметры обновления ([`UpdateUserDto`]).
    ///
    /// # Возвращаемое значение
    /// Обновленная модель [`User`].
    ///
    /// # Ошибки
    /// * [`AppError::not_found`] — если пользователь с указанным `id` не найден.
    /// * [`AppError::validation`] — при попытке заблокировать суперпользователя, нарушить квоты (1..=4).
    /// * [`AppError::database`] — при ошибке выполнения SQL-запроса.
    pub async fn update_user(&self, id: Uuid, dto: UpdateUserDto) -> Result<User> {
        let existing = self.get_user_by_id(id).await?;

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

        let new_username = if let Some(ref req_username) = dto.username {
            let req_username = req_username.trim();
            if req_username.is_empty() {
                existing.username.clone()
            } else if req_username != existing.username {
                // Смена логина разрешена ТОЛЬКО при первом входе (must_change_password == true или login_count <= 1) и не для root
                let can_change_username = (existing.must_change_password || existing.login_count <= 1)
                    && existing.username != "root";
                if !can_change_username {
                    return Err(AppError::validation(
                        "username",
                        "Username cannot be changed after initial setup or for root user",
                    ));
                }

                // Валидация формата логина
                if req_username.len() < 3 || req_username.len() > 32 {
                    return Err(AppError::validation(
                        "username",
                        "Username length must be between 3 and 32 characters",
                    ));
                }
                if !req_username
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
                {
                    return Err(AppError::validation(
                        "username",
                        "Username contains invalid characters (allowed: a-z, 0-9, _, -, .)",
                    ));
                }

                // Проверка уникальности логина
                if let Ok(dup) = self.get_user_by_username(req_username).await {
                    if dup.id != id {
                        return Err(AppError::conflict(format!(
                            "User with username '{}' already exists",
                            req_username
                        )));
                    }
                }

                req_username.to_string()
            } else {
                existing.username.clone()
            }
        } else {
            existing.username.clone()
        };

        let full_name = dto.full_name.or(existing.full_name);
        let email = dto.email.or(existing.email);
        let department = dto.department.or(existing.department);
        let is_active = dto.is_active.unwrap_or(existing.is_active);

        // Обновляем запись пользователя
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE users SET
                username = ?,
                full_name = ?,
                email = ?,
                department = ?,
                password_hash = ?,
                is_active = ?,
                is_superuser = ?,
                must_change_password = ?,
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&new_username)
        .bind(&full_name)
        .bind(&email)
        .bind(&department)
        .bind(&password_hash)
        .bind(if is_active { 1 } else { 0 })
        .bind(if new_is_superuser { 1 } else { 0 })
        .bind(if must_change_password { 1 } else { 0 })
        .bind(now)
        .bind(id.to_string())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(format!("Failed to update user: {}", e)))?;

        // Обновляем роли, если переданы
        if let Some(roles) = dto.roles {
            sqlx::query("DELETE FROM user_roles WHERE user_id = ?")
                .bind(id.to_string())
                .execute(self.db.writer())
                .await
                .map_err(|e| AppError::database(e.to_string()))?;

            for role in &roles {
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

    /// Удалить пользователя по его уникальному идентификатору (UUID)
    ///
    /// Проверяет защиту последнего суперпользователя: в системе всегда должен оставаться
    /// как минимум 1 суперпользователь.
    ///
    /// # Аргументы
    /// * `id` — Идентификатор удаляемого пользователя ([`Uuid`]).
    ///
    /// # Возвращаемое значение
    /// `true`, если пользователь был успешно удален из базы данных.
    ///
    /// # Ошибки
    /// * [`AppError::not_found`] — если пользователь не существует.
    /// * [`AppError::validation`] — при попытке удалить последнего оставшегося суперпользователя.
    /// * [`AppError::database`] — при ошибке выполнения SQL-запроса.
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

    /// Преобразовать кортеж строки таблицы `users` в модель [`User`] с подгрузкой назначенных ролей и прав
    ///
    /// # Аргументы
    /// * `r` — Кортеж значений строки SQLite.
    ///
    /// # Возвращаемое значение
    /// Заполненная структура [`User`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Database`] при сбое запросов к таблицам ролей или прав.
    async fn map_user_row(
        &self,
        r: (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            i64,
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
            department,
            password_hash,
            is_active_num,
            is_superuser_num,
            must_change_pwd_num,
            login_count,
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
            department,
            password_hash,
            is_active,
            is_superuser,
            must_change_password,
            login_count,
            roles,
            permissions,
            created_at,
            updated_at,
            last_login_at,
        })
    }
}
