//! # Модели пользователей, ролей и прав доступа (RBAC)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Модель учетной записи пользователя
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct User {
    /// Уникальный идентификатор
    pub id: Uuid,
    /// Имя пользователя (логин)
    pub username: String,
    /// Отображаемое имя
    pub full_name: Option<String>,
    /// Электронная почта
    pub email: Option<String>,
    /// Хэш пароля (Argon2id)
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// Флаг активности аккаунта
    pub is_active: bool,
    /// Флаг суперпользователя (полный доступ ко всей системе)
    pub is_superuser: bool,
    /// Назначенные роли пользователя
    pub roles: Vec<String>,
    /// Назначенные права пользователя (агрегированные из ролей + индивидуальные)
    pub permissions: Vec<String>,
    /// Дата и время создания
    pub created_at: DateTime<Utc>,
    /// Дата и время последнего обновления
    pub updated_at: DateTime<Utc>,
    /// Дата и время последнего входа
    pub last_login_at: Option<DateTime<Utc>>,
}

/// DTO для создания нового пользователя в системе
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserDto {
    /// Уникальное имя пользователя (логин)
    pub username: String,
    /// Пароль в открытом виде (будет захэширован через Argon2id)
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
    /// Новое полное имя (если передано)
    pub full_name: Option<String>,
    /// Новый адрес электронной почты
    pub email: Option<String>,
    /// Новый пароль (будет перехэширован, если передан непустым)
    pub password: Option<String>,
    /// Новый статус активности
    pub is_active: Option<bool>,
    /// Новый статус суперпользователя
    pub is_superuser: Option<bool>,
    /// Новый список ролей
    pub roles: Option<Vec<String>>,
}

/// DTO для безопасного ответа API с публичной информацией о пользователе (без хэша пароля)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponseDto {
    /// Уникальный идентификатор пользователя
    pub id: Uuid,
    /// Имя пользователя (логин)
    pub username: String,
    /// Отображаемое имя
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
    /// Временная метка последнего успешного входа
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

/// Модель роли в системе RBAC
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Role {
    /// Уникальное имя роли (например, "admin", "operator", "viewer")
    pub name: String,
    /// Описание роли
    pub description: String,
    /// Список идентификаторов прав, назначенных роли
    pub permissions: Vec<String>,
    /// Является ли роль встроенной/системной (нельзя удалить)
    pub is_system: bool,
}

/// Модель гранулярного права доступа
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Permission {
    /// Уникальный идентификатор права (например, "users.view", "modules.manage")
    pub id: String,
    /// Человекочитаемое название
    pub name: String,
    /// Категория права (например, "Users", "System", "Monitoring")
    pub category: String,
    /// Подробное описание назначения права
    pub description: String,
}

/// Полезная нагрузка JWT токена аутентификации (Claims)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtClaims {
    /// ID пользователя (sub)
    pub sub: Uuid,
    /// Логин пользователя
    pub username: String,
    /// Является ли суперпользователем
    pub is_superuser: bool,
    /// Назначенные права пользователя
    pub permissions: Vec<String>,
    /// Время выпуска токена (iat)
    pub iat: i64,
    /// Время истечения срока действия токена (exp)
    pub exp: i64,
}
