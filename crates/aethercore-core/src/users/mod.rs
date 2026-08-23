//! # Сервис управления пользователями и ролями (UserService)
//!
//! Предоставляет CRUD операции над учетными записями пользователей,
//! управление назначением RBAC-ролей, хэширование паролей алгоритмом Argon2id,
//! контроль лимитов суперпользователей, правила защиты от случайной блокировки (Anti-Lockout)
//! и проверку учетных данных при аутентификации.

use crate::auth::{hash_password, validate_password_complexity, verify_password};
use crate::db::Db;
use chrono::{DateTime, Utc};
use aethercore_common::error::{AppError, Result};
use aethercore_common::models::user::{CreateUserDto, SecurityPoliciesDto, UpdateUserDto, User};
use std::collections::HashSet;
use tracing::info;
use uuid::Uuid;

/// Сервис управления учетными записями пользователей и правами доступа
#[derive(Debug, Clone)]
pub struct UserService {
    db: Db,
}

#[derive(Debug, sqlx::FromRow)]
struct UserDbRow {
    pub id: String,
    pub username: String,
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub department: Option<String>,
    pub password_hash: String,
    pub is_active: i64,
    pub is_superuser: i64,
    pub must_change_password: i64,
    pub is_username_locked: i64,
    pub is_totp_enabled: i64,
    pub force_2fa: Option<i64>,
    pub totp_secret: Option<String>,
    pub totp_backup_codes: Option<String>,
    pub login_count: i64,
    pub failed_login_attempts: i64,
    pub locked_until: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
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

    /// Проверить наличие пользователей и инициализировать дефолтного суперпользователя (`root:root`), если база пуста
    ///
    /// # Ошибки
    /// Возвращает [`AppError`] при сбое запроса к базе данных или ошибке хэширования пароля.
    pub async fn ensure_default_admin(&self) -> Result<()> {
        let count_row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(self.db.reader())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        if count_row.0 == 0 {
            info!("No users found in database. Initializing default root user: 'root'");
            let id = Uuid::new_v4();
            let password_hash = hash_password("root")?;
            let now = Utc::now().to_rfc3339();

            sqlx::query(
                r#"
                INSERT INTO users (id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, is_username_locked, login_count, failed_login_attempts, locked_until, created_at, updated_at)
                VALUES (?, 'root', 'Root Administrator', 'root@aethercore.local', 'Core Operations', ?, 1, 1, 0, 1, 0, 0, NULL, ?, ?)
                "#,
            )
            .bind(id.to_string())
            .bind(&password_hash)
            .bind(&now)
            .bind(&now)
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(format!("Failed to insert default root: {}", e)))?;

            let _ = sqlx::query("INSERT OR IGNORE INTO user_roles (user_id, role_name) VALUES (?, 'admin')")
                .bind(id.to_string())
                .execute(self.db.writer())
                .await;
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
        let count_row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE is_superuser = 1 AND is_active = 1")
            .fetch_one(self.db.reader())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        Ok(count_row.0)
    }

    /// Создать нового пользователя системы
    ///
    /// Выполняет валидацию логина и пароля по активной политике безопасности,
    /// проверку квоты суперпользователей (максимум 4), хэширует пароль алгоритмом Argon2id,
    /// сохраняет запись в БД и привязывает указанные роли.
    ///
    /// # Аргументы
    /// * `dto` — Параметры создания пользователя ([`CreateUserDto`]).
    ///
    /// # Возвращаемое значение
    /// Созданная и сохраненная в базе модель [`User`] с назначенными ролями и правами.
    ///
    /// # Ошибки
    /// * [`AppError::validation`] — если логин пустой, пароль не удовлетворяет политике или превышена квота суперпользователей (макс. 4).
    /// * [`AppError::conflict`] — если пользователь с таким логином уже существует в системе.
    /// * [`AppError::database`] — при сбое выполнения SQL-запроса.
    pub async fn create_user(&self, dto: CreateUserDto) -> Result<User> {
        let username = dto.username.trim().to_lowercase();
        if username.is_empty() {
            return Err(AppError::validation("username", "Username cannot be empty"));
        }

        // Загрузка политики безопасности
        let kv = crate::db::kv::KvStore::system(self.db.clone());
        let policy: SecurityPoliciesDto = kv
            .get("security_policies")
            .await
            .unwrap_or_default()
            .unwrap_or_default();

        // Проверка сложности пароля согласно политике
        validate_password_complexity(
            &dto.password,
            policy.min_password_length,
            policy.require_uppercase,
            policy.require_digits,
            policy.require_special,
        )?;

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
        let must_change_password = dto.must_change_password.unwrap_or(policy.mandatory_password_change);
        let is_username_locked = dto.is_username_locked.unwrap_or(false);

        // Вставляем пользователя
        sqlx::query(
            r#"
            INSERT INTO users (id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, is_username_locked, force_2fa, login_count, failed_login_attempts, locked_until, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, NULL, ?, ?)
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
        .bind(if is_username_locked { 1 } else { 0 })
        .bind(dto.force_2fa.map(|v| if v { 1 } else { 0 }))
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
        let row: Option<UserDbRow> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, is_username_locked, is_totp_enabled, force_2fa, totp_secret, totp_backup_codes, login_count, failed_login_attempts, locked_until, created_at, updated_at, last_login_at
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
        let row: Option<UserDbRow> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, is_username_locked, is_totp_enabled, force_2fa, totp_secret, totp_backup_codes, login_count, failed_login_attempts, locked_until, created_at, updated_at, last_login_at
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
    /// Проверяет наличие пользователя, статус активности аккаунта (`is_active`),
    /// блокировку (`locked_until`), а также соответствие пароля с Argon2id хэшем.
    ///
    /// При неудачном вводе увеличивает счетчик неудачных попыток и при превышении лимита
    /// блокирует аккаунт на `lockout_duration` минут согласно `SecurityPoliciesDto`.
    ///
    /// При успехе сбрасывает счетчик неудачных попыток, обновляет поле `last_login_at` и увеличивает `login_count`.
    ///
    /// # Аргументы
    /// * `username` — Имя пользователя.
    /// * `password` — Открытый пароль для проверки.
    ///
    /// # Возвращаемое значение
    /// Аутентифицированная модель [`User`].
    ///
    /// # Ошибки
    /// * [`AppError::unauthorized`] — если учетные данные неверны, аккаунт отключен или заблокирован.
    pub async fn authenticate(&self, username: &str, password: &str) -> Result<User> {
        let user = self.get_user_by_username(username).await.map_err(|_| {
            AppError::unauthorized("Invalid username or password")
                .with_i18n_key("core.auth.invalid_credentials")
        })?;

        if !user.is_active {
            return Err(AppError::unauthorized("Account is disabled")
                .with_i18n_key("core.auth.account_disabled"));
        }

        let now = Utc::now();

        // Проверка текущей блокировки
        if let Some(locked_until) = user.locked_until {
            if locked_until > now {
                let remaining_secs = (locked_until - now).num_seconds().max(1);
                let remaining_mins = (remaining_secs + 59) / 60;
                return Err(AppError::unauthorized(format!(
                    "Account is temporarily locked due to excessive failed attempts. Try again in {} min.",
                    remaining_mins
                ))
                .with_i18n_key("core.auth.account_locked")
                .with_details(serde_json::json!({
                    "minutes": remaining_mins.to_string(),
                    "details": format!("Account is locked for {} min", remaining_mins)
                })));
            }
        }

        let kv = crate::db::kv::KvStore::system(self.db.clone());
        let policy: SecurityPoliciesDto = kv
            .get("security_policies")
            .await
            .unwrap_or_default()
            .unwrap_or_default();

        if !verify_password(password, &user.password_hash)? {
            let new_failed_attempts = user.failed_login_attempts + 1;
            let mut locked_until_val: Option<String> = None;

            if new_failed_attempts >= policy.max_login_attempts as i64 {
                let lock_until = now + chrono::Duration::minutes(policy.lockout_duration as i64);
                locked_until_val = Some(lock_until.to_rfc3339());
            }

            let _ = sqlx::query(
                "UPDATE users SET failed_login_attempts = ?, locked_until = ? WHERE id = ?"
            )
            .bind(new_failed_attempts)
            .bind(locked_until_val.as_ref())
            .bind(user.id.to_string())
            .execute(self.db.writer())
            .await;

            if locked_until_val.is_some() {
                return Err(AppError::unauthorized(format!(
                    "Account is temporarily locked for {} min due to exceeding maximum login attempts.",
                    policy.lockout_duration
                ))
                .with_i18n_key("core.auth.account_locked")
                .with_details(serde_json::json!({
                    "minutes": policy.lockout_duration.to_string(),
                    "details": format!("Account is locked for {} min", policy.lockout_duration)
                })));
            }

            return Err(AppError::unauthorized("Invalid username or password")
                .with_i18n_key("core.auth.invalid_credentials"));
        }

        // Обновляем время последнего входа, увеличиваем счетчик логинов и сбрасываем блокировки
        let now_str = now.to_rfc3339();
        let _ = sqlx::query(
            "UPDATE users SET last_login_at = ?, login_count = login_count + 1, failed_login_attempts = 0, locked_until = NULL WHERE id = ?"
        )
        .bind(now_str)
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
        let rows: Vec<UserDbRow> = sqlx::query_as(
            r#"
            SELECT id, username, full_name, email, department, password_hash, is_active, is_superuser, must_change_password, is_username_locked, is_totp_enabled, force_2fa, totp_secret, totp_backup_codes, login_count, failed_login_attempts, locked_until, created_at, updated_at, last_login_at
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
    /// 4. Проверка сложности пароля согласно `SecurityPoliciesDto` при его обновлении.
    /// 5. Автоматический сброс флага `must_change_password`, `failed_login_attempts` и `locked_until` при установке нового пароля.
    /// 6. Разрешение смены логина строго до момента его фиксации (`is_username_locked == false`) и запрет смены для `root`.
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
    /// * [`AppError::validation`] — при попытке заблокировать суперпользователя, нарушить квоты или задать некорректный пароль.
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
            Some(ref pwd) if !pwd.trim().is_empty() => {
                if let Some(ref current_pwd) = dto.current_password {
                    if !crate::auth::verify_password(current_pwd, &existing.password_hash)? {
                        return Err(AppError::validation(
                            "current_password",
                            "Invalid current password",
                        ));
                    }
                }

                let kv = crate::db::kv::KvStore::system(self.db.clone());
                let policy: SecurityPoliciesDto = kv
                    .get("security_policies")
                    .await
                    .unwrap_or_default()
                    .unwrap_or_default();

                validate_password_complexity(
                    pwd,
                    policy.min_password_length,
                    policy.require_uppercase,
                    policy.require_digits,
                    policy.require_special,
                )?;

                hash_password(pwd)?
            }
            _ => existing.password_hash.clone(),
        };

        // Если пароль сменен, сбрасываем must_change_password в 0 (если не указано обратное)
        let must_change_password = if is_password_changed && dto.must_change_password.is_none() {
            false
        } else {
            dto.must_change_password.unwrap_or(existing.must_change_password)
        };

        let mut is_username_locked = dto.is_username_locked.unwrap_or(existing.is_username_locked);

        let new_username = if let Some(ref req_username) = dto.username {
            let req_username = req_username.trim();
            if req_username.is_empty() {
                existing.username.clone()
            } else if req_username != existing.username {
                // Смена логина разрешена ТОЛЬКО если логин еще не зафиксирован (is_username_locked == false) и не для root
                let can_change_username = !existing.is_username_locked && existing.username != "root";
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

                // При успешной смене логина фиксируем его навсегда
                is_username_locked = true;
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
        let force_2fa_val = if dto.force_2fa.is_some() {
            dto.force_2fa
        } else {
            existing.force_2fa
        };

        // Обновляем запись пользователя
        let now = Utc::now().to_rfc3339();
        if is_password_changed {
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
                    is_username_locked = ?,
                    force_2fa = ?,
                    failed_login_attempts = 0,
                    locked_until = NULL,
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
            .bind(if is_username_locked { 1 } else { 0 })
            .bind(force_2fa_val.map(|v| if v { 1 } else { 0 }))
            .bind(&now)
            .bind(id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        } else {
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
                    is_username_locked = ?,
                    force_2fa = ?,
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
            .bind(if is_username_locked { 1 } else { 0 })
            .bind(force_2fa_val.map(|v| if v { 1 } else { 0 }))
            .bind(&now)
            .bind(id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;
        }

        // Обновляем роли, если переданы
        if let Some(ref roles) = dto.roles {
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

    /// Удалить пользователя из системы
    ///
    /// Реализует проверки:
    /// 1. Запрет удаления суперпользователя (`Anti-Lockout`).
    /// 2. Запрет удаления пользователя `root`.
    /// 3. Удаление ассоциированных ролей (`user_roles`).
    ///
    /// # Аргументы
    /// * `id` — Идентификатор удаляемого пользователя ([`Uuid`]).
    ///
    /// # Ошибки
    /// * [`AppError::not_found`] — если пользователь с указанным `id` не найден.
    /// * [`AppError::validation`] — при попытке удалить суперпользователя или пользователя `root`.
    /// * [`AppError::database`] — при ошибке выполнения SQL-запроса.
    pub async fn delete_user(&self, id: Uuid) -> Result<()> {
        let existing = self.get_user_by_id(id).await?;

        // Запрет удаления root пользователя
        if existing.username == "root" {
            return Err(AppError::validation(
                "user_id",
                "Root user account cannot be deleted",
            ));
        }

        // Запрет удаления последнего оставшегося суперпользователя
        if existing.is_superuser {
            let current_superusers = self.count_superusers().await?;
            if current_superusers <= 1 {
                return Err(AppError::validation(
                    "user_id",
                    "Cannot delete the last remaining superuser in the system",
                ));
            }
        }

        // Удаляем назначенные роли
        sqlx::query("DELETE FROM user_roles WHERE user_id = ?")
            .bind(id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        // Удаляем пользователя
        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        Ok(())
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
    async fn map_user_row(&self, r: UserDbRow) -> Result<User> {
        let id_str = r.id;
        let id = Uuid::parse_str(&id_str).unwrap_or_default();
        let username = r.username;
        let full_name = r.full_name;
        let email = r.email;
        let department = r.department;
        let password_hash = r.password_hash;
        let is_active = r.is_active != 0;
        let is_superuser = r.is_superuser != 0;
        let must_change_password = r.must_change_password != 0;
        let is_username_locked = r.is_username_locked != 0;
        let is_totp_enabled = r.is_totp_enabled != 0;
        let force_2fa = r.force_2fa.map(|v| v != 0);
        let totp_secret = r.totp_secret;
        let totp_backup_codes = r.totp_backup_codes;
        let login_count = r.login_count;
        let failed_login_attempts = r.failed_login_attempts;

        let locked_until = r.locked_until.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });
        let created_at = DateTime::parse_from_rfc3339(&r.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&r.updated_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let last_login_at = r.last_login_at.and_then(|s| {
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
            is_username_locked,
            is_totp_enabled,
            force_2fa,
            totp_secret,
            totp_backup_codes,
            login_count,
            failed_login_attempts,
            locked_until,
            roles,
            permissions,
            created_at,
            updated_at,
            last_login_at,
        })
    }

    /// Включить двухфакторную аутентификацию (TOTP) для пользователя
    pub async fn enable_totp(
        &self,
        user_id: Uuid,
        secret: &str,
        raw_backup_codes: &[String],
    ) -> Result<()> {
        let backup_codes_json = crate::auth::totp::hash_and_serialize_backup_codes(raw_backup_codes)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE users SET is_totp_enabled = 1, totp_secret = ?, totp_backup_codes = ?, updated_at = ? WHERE id = ?"
        )
        .bind(secret.trim())
        .bind(&backup_codes_json)
        .bind(&now)
        .bind(user_id.to_string())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        Ok(())
    }

    /// Отключить двухфакторную аутентификацию (TOTP) для пользователя
    pub async fn disable_totp(&self, user_id: Uuid) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE users SET is_totp_enabled = 0, totp_secret = NULL, totp_backup_codes = NULL, updated_at = ? WHERE id = ?"
        )
        .bind(&now)
        .bind(user_id.to_string())
        .execute(self.db.writer())
        .await
        .map_err(|e| AppError::database(e.to_string()))?;

        Ok(())
    }

    /// Списать одноразовый резервный код восстановления
    /// Возвращает количество оставшихся кодов при успехе
    pub async fn consume_backup_code(&self, user_id: Uuid, raw_code: &str) -> Result<usize> {
        let user = self.get_user_by_id(user_id).await?;
        let backup_json = user.totp_backup_codes.as_deref().ok_or_else(|| {
            AppError::unauthorized("No backup codes configured")
                .with_i18n_key("core.auth.invalid_backup_code")
        })?;

        let updated_json = crate::auth::totp::verify_and_consume_backup_code(backup_json, raw_code)
            .ok_or_else(|| {
                AppError::unauthorized("Invalid backup code")
                    .with_i18n_key("core.auth.invalid_backup_code")
            })?;

        let remaining_count = crate::auth::totp::count_remaining_backup_codes(Some(&updated_json));
        let now = Utc::now().to_rfc3339();

        sqlx::query("UPDATE users SET totp_backup_codes = ?, updated_at = ? WHERE id = ?")
            .bind(&updated_json)
            .bind(&now)
            .bind(user_id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        Ok(remaining_count)
    }

    /// Сгенерировать и обновить резервные коды пользователя
    pub async fn regenerate_backup_codes(&self, user_id: Uuid, count: usize) -> Result<Vec<String>> {
        let user = self.get_user_by_id(user_id).await?;
        if !user.is_totp_enabled {
            return Err(AppError::validation(
                "2fa",
                "Cannot generate backup codes when 2FA is disabled",
            ));
        }

        let raw_codes = crate::auth::totp::generate_backup_codes(count);
        let backup_codes_json = crate::auth::totp::hash_and_serialize_backup_codes(&raw_codes)?;
        let now = Utc::now().to_rfc3339();

        sqlx::query("UPDATE users SET totp_backup_codes = ?, updated_at = ? WHERE id = ?")
            .bind(&backup_codes_json)
            .bind(&now)
            .bind(user_id.to_string())
            .execute(self.db.writer())
            .await
            .map_err(|e| AppError::database(e.to_string()))?;

        Ok(raw_codes)
    }

    /// Синхронизировать матрицу прав ролей с таблицей `role_permissions` в базе данных
    pub async fn sync_role_permissions(&self, roles_to_clear: &[&str], role_perms: &[(&str, &str)]) -> Result<()> {
        let pool = self.db.writer();
        let mut tx = pool.begin().await.map_err(|e| AppError::database(e.to_string()))?;

        for role in roles_to_clear {
            sqlx::query("DELETE FROM role_permissions WHERE role_name = ?")
                .bind(role)
                .execute(&mut *tx)
                .await
                .map_err(|e| AppError::database(e.to_string()))?;
        }

        for (role, perm) in role_perms {
            if roles_to_clear.contains(role) {
                sqlx::query("INSERT OR IGNORE INTO role_permissions (role_name, permission_id) VALUES (?, ?)")
                    .bind(role)
                    .bind(perm)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| AppError::database(e.to_string()))?;
            }
        }

        tx.commit().await.map_err(|e| AppError::database(e.to_string()))?;
        Ok(())
    }
}
