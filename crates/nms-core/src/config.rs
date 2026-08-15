// Конфигурация ядра NMS приложения
// Поддерживает настройки порта, хоста, ключей шифрования, CORS и режимов работы (headless сервер / desktop GUI)

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

/// Структура конфигурации сервера NMS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Адрес и порт для прослушивания HTTP/WS запросов
    pub host: String,
    pub port: u16,
    /// Каталог хранения динамических WASM-модулей
    pub modules_dir: PathBuf,
    /// Флаг запуска в режиме серверного демона (Headless Daemon)
    pub is_server_mode: bool,
    /// Персистентный секретный ключ для шифрования данных at-rest и JWT
    pub secret_key: String,
    /// Разрешённые CORS origins
    pub cors_origins: Vec<String>,
    /// Флаг включения HSTS
    pub enable_hsts: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            modules_dir: PathBuf::from("./modules"),
            is_server_mode: false,
            secret_key: get_or_create_secret_key(),
            cors_origins: vec![
                "http://localhost:5173".to_string(),
                "http://127.0.0.1:5173".to_string(),
            ],
            enable_hsts: false,
        }
    }
}

impl AppConfig {
    /// Загрузить конфигурацию из переменных окружения NMS_* с фолбэками
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(val) = env::var("NMS_HOST") {
            if !val.trim().is_empty() {
                config.host = val.trim().to_string();
            }
        }

        if let Ok(val) = env::var("NMS_PORT") {
            if let Ok(p) = val.trim().parse::<u16>() {
                config.port = p;
            }
        }

        if let Ok(val) = env::var("NMS_MODULES_DIR") {
            if !val.trim().is_empty() {
                config.modules_dir = PathBuf::from(val.trim());
            }
        }

        if let Ok(val) = env::var("NMS_SERVER_MODE") {
            config.is_server_mode =
                matches!(val.trim().to_lowercase().as_str(), "true" | "1" | "yes");
        }

        if let Ok(val) = env::var("NMS_SECRET_KEY") {
            if !val.trim().is_empty() {
                config.secret_key = val.trim().to_string();
            }
        }

        if let Ok(val) = env::var("NMS_CORS_ORIGINS") {
            let origins: Vec<String> = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !origins.is_empty() {
                config.cors_origins = origins;
            }
        }

        if let Ok(val) = env::var("NMS_ENABLE_HSTS") {
            config.enable_hsts = matches!(val.trim().to_lowercase().as_str(), "true" | "1" | "yes");
        }

        config
    }

    /// Преобразование настроек хоста и порта в сетевой адрес SocketAddr
    pub fn socket_addr(&self) -> Result<SocketAddr, std::net::AddrParseError> {
        let addr_str = format!("{}:{}", self.host, self.port);
        addr_str.parse()
    }
}

/// Получить SECRET_KEY из env NMS_SECRET_KEY или персистентного файла data/.secret_key
pub fn get_or_create_secret_key() -> String {
    if let Ok(env_key) = env::var("NMS_SECRET_KEY") {
        if !env_key.trim().is_empty() {
            return env_key.trim().to_string();
        }
    }

    let data_dir = Path::new("data");
    let secret_file = data_dir.join(".secret_key");

    if secret_file.exists() {
        if let Ok(content) = fs::read_to_string(&secret_file) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    let mut bytes = [0u8; 64];
    let new_key = if getrandom::getrandom(&mut bytes).is_ok() {
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    } else {
        format!("sk_{}", uuid::Uuid::new_v4().simple())
    };

    if fs::create_dir_all(data_dir).is_ok() {
        if fs::write(&secret_file, &new_key).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&secret_file, fs::Permissions::from_mode(0o600));
            }
        }
    }

    new_key
}
