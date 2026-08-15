//! # Конфигурация платформы (AppConfig)

use crate::i18n::Locale;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Общая конфигурация ядра и сервисов платформы
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Сетевые настройки сервера
    #[serde(default)]
    pub server: ServerConfig,
    /// Настройки базы данных
    #[serde(default)]
    pub database: DatabaseConfig,
    /// Настройки безопасности и аутентификации
    #[serde(default)]
    pub security: SecurityConfig,
    /// Настройки плагинов и песочницы
    #[serde(default)]
    pub plugins: PluginsConfig,
    /// Настройки интернационализации
    #[serde(default)]
    pub i18n: I18nConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            security: SecurityConfig::default(),
            plugins: PluginsConfig::default(),
            i18n: I18nConfig::default(),
        }
    }
}

/// Конфигурация HTTP/WS сервера
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Адрес привязки сокета
    #[serde(default = "default_host")]
    pub host: String,
    /// Порт сервера
    #[serde(default = "default_port")]
    pub port: u16,
    /// Разрешить запуск в dev-режиме
    #[serde(default)]
    pub dev_mode: bool,
    /// Режим Safe-Mode (аварийный старт без плагинов)
    #[serde(default)]
    pub safe_mode: bool,
}

fn default_host() -> String {
    "127.0.0.1".into()
}

fn default_port() -> u16 {
    8080
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            dev_mode: false,
            safe_mode: false,
        }
    }
}

/// Конфигурация базы данных SQLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Путь к файлу базы данных SQLite
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
    /// Максимальный размер пула соединений на чтение
    #[serde(default = "default_max_read_connections")]
    pub max_read_connections: u32,
    /// Таймаут ожидания освобождения блокировки (мс)
    #[serde(default = "default_busy_timeout")]
    pub busy_timeout_ms: u64,
}

fn default_db_path() -> PathBuf {
    PathBuf::from("data/nms.db")
}

fn default_max_read_connections() -> u32 {
    10
}

fn default_busy_timeout() -> u64 {
    5000
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: default_db_path(),
            max_read_connections: default_max_read_connections(),
            busy_timeout_ms: default_busy_timeout(),
        }
    }
}

/// Конфигурация безопасности
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Секретный ключ для подписи JWT токенов (генерируется при первом старте)
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    /// Время жизни токена доступа в секундах (по умолчанию 24 часа)
    #[serde(default = "default_jwt_ttl")]
    pub jwt_ttl_seconds: i64,
    /// Разрешить неподписанные плагины (для dev-режима)
    #[serde(default)]
    pub allow_unsigned_plugins: bool,
    /// Список доверенных публичных Ed25519 ключей для проверки подписи плагинов
    #[serde(default)]
    pub trusted_public_keys: Vec<String>,
}

fn default_jwt_secret() -> String {
    "change-me-in-production-secure-random-secret-key-1234567890".into()
}

fn default_jwt_ttl() -> i64 {
    86400 // 24 часа
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            jwt_ttl_seconds: default_jwt_ttl(),
            allow_unsigned_plugins: false,
            trusted_public_keys: Vec::new(),
        }
    }
}

/// Конфигурация каталогов и песочницы плагинов
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Каталог для хранения установленных плагинов
    #[serde(default = "default_plugins_dir")]
    pub dir: PathBuf,
    /// Каталог для AOT кэша скомпилированных .cwasm модулей
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
    /// Лимит памяти песочницы на модуль в мегабайтах
    #[serde(default = "default_memory_limit_mb")]
    pub memory_limit_mb: usize,
    /// Таймаут выполнения гостевого кода в секундах (Epoch interruption)
    #[serde(default = "default_execution_timeout_sec")]
    pub execution_timeout_sec: u64,
}

fn default_plugins_dir() -> PathBuf {
    PathBuf::from("modules")
}

fn default_cache_dir() -> PathBuf {
    PathBuf::from("cache/modules")
}

fn default_memory_limit_mb() -> usize {
    128
}

fn default_execution_timeout_sec() -> u64 {
    5
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            dir: default_plugins_dir(),
            cache_dir: default_cache_dir(),
            memory_limit_mb: default_memory_limit_mb(),
            execution_timeout_sec: default_execution_timeout_sec(),
        }
    }
}

/// Конфигурация локализации
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nConfig {
    /// Язык по умолчанию
    #[serde(default)]
    pub default_locale: Locale,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            default_locale: Locale::Ru,
        }
    }
}
