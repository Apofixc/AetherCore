//! # Модели пользователей, ролей и прав доступа (RBAC)
//!
//! Модуль определяет структуры данных для управления пользователями платформы ([`User`]),
//! DTO для создания ([`CreateUserDto`]) и обновления ([`UpdateUserDto`]),
//! DTO безопасного ответа REST API ([`UserResponseDto`]),
//! модели ролей ([`Role`]), гранулярных прав ([`Permission`]) и клеймов токена ([`JwtClaims`]).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Модель учетной записи пользователя платформы
///
/// Хранит данные учетной записи, Argon2id хэш пароля, назначенные роли и агрегированные права доступа (RBAC).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    /// Уникальный идентификатор пользователя (UUID v4)
    pub id: Uuid,
    /// Имя пользователя (логин в нижнем регистре)
    pub username: String,
    /// Отображаемое полное имя или ФИО
    pub full_name: Option<String>,
    /// Контактный адрес электронной почты
    pub email: Option<String>,
    /// Подразделение / Департамент
    pub department: Option<String>,
    /// Хэш пароля в формате PHC (Argon2id), исключается из прямой JSON-сериализации
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// Флаг активности учетной записи (неактивные пользователи не могут пройти аутентификацию)
    pub is_active: bool,
    /// Флаг суперпользователя (предоставляет безусловный доступ ко всем операциям ядра)
    pub is_superuser: bool,
    /// Требуется ли обязательная смена пароля при следующем входе
    pub must_change_password: bool,
    /// Список назначенных пользователю ролей (например, `["admin"]`, `["viewer"]`)
    pub roles: Vec<String>,
    /// Агрегированный дедуплицированный список прав доступа (из назначенных ролей и индивидуальных прав)
    pub permissions: Vec<String>,
    /// Количество успешных аутентификаций пользователя
    pub login_count: i64,
    /// Количество последовательных неудачных попыток входа
    #[serde(default)]
    pub failed_login_attempts: i64,
    /// Дата и время, до которого учетная запись заблокирована из-за превышения попыток (UTC)
    pub locked_until: Option<DateTime<Utc>>,
    /// Дата и время создания учетной записи (UTC)
    pub created_at: DateTime<Utc>,
    /// Дата и время последнего обновления профиля (UTC)
    pub updated_at: DateTime<Utc>,
    /// Дата и время последнего успешного входа в систему (UTC)
    pub last_login_at: Option<DateTime<Utc>>,
}

/// DTO системных политик безопасности
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPoliciesDto {
    /// Требовать обязательную аутентификацию в Web UI
    #[serde(default = "default_true")]
    pub web_ui_auth: bool,
    /// Обязательная смена пароля при первом входе
    #[serde(default = "default_true")]
    pub mandatory_password_change: bool,
    /// Принудительная двухфакторная аутентификация
    #[serde(default = "default_false")]
    pub force_2fa: bool,
    /// Максимальное число неудачных попыток входа
    #[serde(default = "default_max_login_attempts")]
    pub max_login_attempts: u32,
    /// Длительность блокировки в минутах
    #[serde(default = "default_lockout_duration")]
    pub lockout_duration: u32,
    /// Время жизни сессии в часах
    #[serde(default = "default_session_ttl")]
    pub session_ttl: u32,
    /// Таймаут неактивности пользователя в минутах
    #[serde(default = "default_inactivity_timeout")]
    pub inactivity_timeout: u32,
    /// Минимальная длина пароля
    #[serde(default = "default_min_password_length")]
    pub min_password_length: u32,
    /// Требование заглавных букв в пароле
    #[serde(default = "default_true")]
    pub require_uppercase: bool,
    /// Требование цифр в пароле
    #[serde(default = "default_true")]
    pub require_digits: bool,
    /// Требование спецсимволов в пароле
    #[serde(default = "default_true")]
    pub require_special: bool,
    /// Белый список разрешенных IP-адресов / подсетей через запятую или пробел
    #[serde(default = "default_ip_whitelist")]
    pub ip_whitelist: String,
}

fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_max_login_attempts() -> u32 {
    5
}
fn default_lockout_duration() -> u32 {
    30
}
fn default_session_ttl() -> u32 {
    12
}
fn default_inactivity_timeout() -> u32 {
    30
}
fn default_min_password_length() -> u32 {
    8
}
fn default_ip_whitelist() -> String {
    String::new()
}

impl Default for SecurityPoliciesDto {
    fn default() -> Self {
        Self {
            web_ui_auth: true,
            mandatory_password_change: true,
            force_2fa: false,
            max_login_attempts: 5,
            lockout_duration: 30,
            session_ttl: 12,
            inactivity_timeout: 30,
            min_password_length: 8,
            require_uppercase: true,
            require_digits: true,
            require_special: true,
            ip_whitelist: default_ip_whitelist(),
        }
    }
}

/// DTO для регистрации/создания нового пользователя в системе
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateUserDto {
    /// Уникальное имя пользователя (логин, минимум 1 символ)
    pub username: String,
    /// Пароль в открытом виде (минимум 4 символа, будет захэширован через Argon2id)
    pub password: String,
    /// Полное имя или ФИО пользователя
    pub full_name: Option<String>,
    /// Контактный адрес электронной почты
    pub email: Option<String>,
    /// Подразделение / Департамент
    pub department: Option<String>,
    /// Флаг активности учетной записи (по умолчанию `true`)
    pub is_active: Option<bool>,
    /// Флаг суперпользователя (по умолчанию `false`)
    pub is_superuser: Option<bool>,
    /// Требовать ли обязательную смену пароля при первом входе
    pub must_change_password: Option<bool>,
    /// Список назначаемых ролей (по умолчанию `["viewer"]`)
    pub roles: Option<Vec<String>>,
}

/// DTO для частичного обновления учетной записи пользователя
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUserDto {
    /// Новый логин (разрешено менять только при первом входе, когда must_change_password = true или login_count <= 1)
    pub username: Option<String>,
    /// Новое полное имя (если передано `Some`)
    pub full_name: Option<String>,
    /// Новый адрес электронной почты (если передано `Some`)
    pub email: Option<String>,
    /// Подразделение / Департамент
    pub department: Option<String>,
    /// Новый открытый пароль (будет перехэширован алгоритмом Argon2id, если передан непустым)
    pub password: Option<String>,
    /// Новый статус активности аккаунта
    pub is_active: Option<bool>,
    /// Новый статус суперпользователя
    pub is_superuser: Option<bool>,
    /// Требовать ли обязательную смену пароля при следующем входе
    pub must_change_password: Option<bool>,
    /// Новый список назначенных ролей (перезаписывает предыдущий набор)
    pub roles: Option<Vec<String>>,
}

/// DTO для безопасного ответа REST API с публичной информацией о пользователе (без хэша пароля)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponseDto {
    /// Уникальный идентификатор пользователя
    pub id: Uuid,
    /// Имя пользователя (логин)
    pub username: String,
    /// Отображаемое полное имя
    pub full_name: Option<String>,
    /// Электронная почта
    pub email: Option<String>,
    /// Подразделение / Департамент
    pub department: Option<String>,
    /// Флаг активности учетной записи
    pub is_active: bool,
    /// Флаг суперпользователя
    pub is_superuser: bool,
    /// Флаг обязательной смены пароля
    pub must_change_password: bool,
    /// Количество успешных аутентификаций
    pub login_count: i64,
    /// Список назначенных ролей
    pub roles: Vec<String>,
    /// Агрегированный список прав доступа
    pub permissions: Vec<String>,
    /// Временная метка создания пользователя (UTC)
    pub created_at: DateTime<Utc>,
    /// Временная метка последнего успешного входа (UTC)
    pub last_login_at: Option<DateTime<Utc>>,
}

impl From<User> for UserResponseDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            full_name: u.full_name,
            email: u.email,
            department: u.department,
            is_active: u.is_active,
            is_superuser: u.is_superuser,
            must_change_password: u.must_change_password,
            login_count: u.login_count,
            roles: u.roles,
            permissions: u.permissions,
            created_at: u.created_at,
            last_login_at: u.last_login_at,
        }
    }
}

/// Модель роли в ролевой системе контроля доступа (RBAC)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Role {
    /// Уникальное имя роли (например, `"admin"`, `"operator"`, `"viewer"`)
    pub name: String,
    /// Описание назначения роли
    pub description: String,
    /// Список идентификаторов прав, назначенных роли (например, `["system.view", "modules.manage"]`)
    pub permissions: Vec<String>,
    /// Является ли роль встроенной/системной (системные роли защищены от удаления)
    pub is_system: bool,
}

/// Модель гранулярного системного права доступа
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Permission {
    /// Уникальный строковый идентификатор права (например, `"users.view"`, `"modules.manage"`, `"system.manage"`)
    pub id: String,
    /// Человекочитаемое название права
    pub name: String,
    /// Категория права (например, `"Users"`, `"System"`, `"Modules"`, `"Events"`)
    pub category: String,
    /// Подробное описание назначения и области действия права
    pub description: String,
}

/// Полезная нагрузка JWT токена аутентификации (Claims)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// ID пользователя (sub - subject claim)
    pub sub: Uuid,
    /// Логин пользователя
    pub username: String,
    /// Является ли пользователь суперпользователем
    pub is_superuser: bool,
    /// Назначенные права пользователя для быстрой проверки без обращения к БД
    pub permissions: Vec<String>,
    /// Временная метка выпуска токена в формате Unix timestamp (iat)
    pub iat: i64,
    /// Временная метка истечения срока действия токена в формате Unix timestamp (exp)
    pub exp: i64,
}
