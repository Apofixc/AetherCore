//! # Конфигурация платформы (AppConfig)
//!
//! Модуль содержит структуры конфигурации для всех компонентов NMSNext-Gen:
//! сетевого сервера ([`ServerConfig`]), базы данных SQLite ([`DatabaseConfig`]),
//! подсистемы безопасности и JWT ([`SecurityConfig`]), песочницы Wasm-плагинов ([`PluginsConfig`])
//! и интернационализации ([`I18nConfig`]).

use crate::i18n::Locale;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Общая конфигурация ядра и сервисов платформы
///
/// Объединяет настройки всех подсистем и поддерживает сериализацию/десериализацию в JSON/YAML/TOML.
///
/// # Примеры
/// ```rust
/// use aethercore_common::config::AppConfig;
///
/// let config = AppConfig::default();
/// assert_eq!(config.server.port, 8080);
/// assert_eq!(config.server.host, "127.0.0.1");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Сетевые настройки HTTP/WebSocket сервера ([`ServerConfig`])
    #[serde(default)]
    pub server: ServerConfig,
    /// Настройки пула соединений и файла базы данных SQLite ([`DatabaseConfig`])
    #[serde(default)]
    pub database: DatabaseConfig,
    /// Настройки безопасности, ключей JWT и верификации подписей ([`SecurityConfig`])
    #[serde(default)]
    pub security: SecurityConfig,
    /// Настройки каталогов, лимитов памяти и таймаутов плагинов ([`PluginsConfig`])
    #[serde(default)]
    pub plugins: PluginsConfig,
    /// Настройки интернационализации и языка по умолчанию ([`I18nConfig`])
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

/// Конфигурация HTTP/WS сервера платформы
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Адрес привязки TCP-сокета (по умолчанию `"127.0.0.1"`)
    #[serde(default = "default_host")]
    pub host: String,
    /// Сетевой порт сервера (по умолчанию `8080`)
    #[serde(default = "default_port")]
    pub port: u16,
    /// Разрешить запуск в dev-режиме с ослабленными проверками подписей
    #[serde(default)]
    pub dev_mode: bool,
    /// Режим Safe-Mode (аварийный старт без загрузки пользовательских плагинов)
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

/// Конфигурация базы данных SQLite (Single-Writer / Multi-Reader)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Путь к файлу базы данных SQLite на диске (по умолчанию `"data/nms.db"`)
    #[serde(default = "default_db_path")]
    pub path: PathBuf,
    /// Максимальный размер пула соединений на параллельное чтение (по умолчанию `10`)
    #[serde(default = "default_max_read_connections")]
    pub max_read_connections: u32,
    /// Таймаут ожидания освобождения блокировки SQLite в миллисекундах (`busy_timeout`, по умолчанию `5000` мс)
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

/// Конфигурация подсистемы безопасности и аутентификации
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Секретный ключ для криптографической подписи JWT токенов (HMAC-SHA256)
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    /// Время жизни токена доступа в секундах (по умолчанию `86400` — 24 часа)
    #[serde(default = "default_jwt_ttl")]
    pub jwt_ttl_seconds: i64,
    /// Разрешить установку и запуск неподписанных плагинов (для локальной разработки)
    #[serde(default)]
    pub allow_unsigned_plugins: bool,
    /// Список доверенных публичных Ed25519 ключей (в hex/base64 формате) для проверки подписей модулей
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

/// Конфигурация каталогов и песочницы WebAssembly плагинов
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Каталог для хранения установленных плагинов (по умолчанию `"modules"`)
    #[serde(default = "default_plugins_dir")]
    pub dir: PathBuf,
    /// Каталог для AOT кэша скомпилированных `.cwasm` модулей (по умолчанию `"cache/modules"`)
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
    /// Лимит оперативной памяти песочницы на модуль в мегабайтах (по умолчанию `128` МБ)
    #[serde(default = "default_memory_limit_mb")]
    pub memory_limit_mb: usize,
    /// Таймаут выполнения гостевого кода в секундах (Epoch interruption, по умолчанию `5` сек)
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

/// Конфигурация интернационализации платформы
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nConfig {
    /// Языковая локаль по умолчанию ([`Locale`])
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
