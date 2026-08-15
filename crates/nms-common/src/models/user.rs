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
    /// Хэш пароля в формате PHC (Argon2id), исключается из прямой JSON-сериализации
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// Флаг активности учетной записи (неактивные пользователи не могут пройти аутентификацию)
    pub is_active: bool,
    /// Флаг суперпользователя (предоставляет безусловный доступ ко всем операциям ядра)
    pub is_superuser: bool,
    /// Список назначенных пользователю ролей (например, `["admin"]`, `["viewer"]`)
    pub roles: Vec<String>,
    /// Агрегированный дедуплицированный список прав доступа (из назначенных ролей и индивидуальных прав)
    pub permissions: Vec<String>,
    /// Дата и время создания учетной записи (UTC)
    pub created_at: DateTime<Utc>,
    /// Дата и время последнего обновления профиля (UTC)
    pub updated_at: DateTime<Utc>,
    /// Дата и время последнего успешного входа в систему (UTC)
    pub last_login_at: Option<DateTime<Utc>>,
}

/// DTO для регистрации/создания нового пользователя в системе
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserDto {
    /// Уникальное имя пользователя (логин, минимум 1 символ)
    pub username: String,
    /// Пароль в открытом виде (минимум 4 символа, будет захэширован через Argon2id)
    pub password: String,
    /// Полное имя или ФИО пользователя
    pub full_name: Option<String>,
    /// Контактный адрес электронной почты
    pub email: Option<String>,
    /// Флаг активности учетной записи (по умолчанию `true`)
    pub is_active: Option<bool>,
    /// Флаг суперпользователя (по умолчанию `false`)
    pub is_superuser: Option<bool>,
    /// Список назначаемых ролей (по умолчанию `["viewer"]`)
    pub roles: Option<Vec<String>>,
}

/// DTO для частичного обновления учетной записи пользователя
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateUserDto {
    /// Новое полное имя (если передано `Some`)
    pub full_name: Option<String>,
    /// Новый адрес электронной почты (если передано `Some`)
    pub email: Option<String>,
    /// Новый открытый пароль (будет перехэширован алгоритмом Argon2id, если передан непустым)
    pub password: Option<String>,
    /// Новый статус активности аккаунта
    pub is_active: Option<bool>,
    /// Новый статус суперпользователя
    pub is_superuser: Option<bool>,
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
    /// Флаг активности учетной записи
    pub is_active: bool,
    /// Флаг суперпользователя
    pub is_superuser: bool,
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
            is_active: u.is_active,
            is_superuser: u.is_superuser,
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
