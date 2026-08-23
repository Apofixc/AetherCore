//! # Сервис управления системными логами и лог-провайдерами (LoggerService)
//!
//! Предоставляет:
//! - In-memory кольцевой буфер недавних логов для мгновенного доступа через REST API / WebSockets.
//! - Файловую запись логов с ограничением размера и ротацией.
//! - Реестр источников логов (`LogProvider`: ядро, плагины, внешние сервисы).
//! - Поиск, фильтрацию по уровням важности и скачивание лог-файлов.

use chrono::{DateTime, Utc};
use aethercore_common::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// Уровни важности сообщений лога
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    /// Трассировочные детальные сообщения (наивысшая степень детализации)
    Trace,
    /// Отладочные сообщения разработчиков
    Debug,
    /// Информационные штатные события системы
    Info,
    /// Предупреждения о нештатных ситуациях, не прерывающих работу
    Warn,
    /// Критические ошибки и исключения
    Error,
}

impl LogLevel {
    /// Преобразовать произвольную строку в [`LogLevel`] без учета регистра
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_uppercase().as_str() {
            "TRACE" => Some(Self::Trace),
            "DEBUG" => Some(Self::Debug),
            "INFO" => Some(Self::Info),
            "WARN" | "WARNING" => Some(Self::Warn),
            "ERROR" | "CRITICAL" | "FATAL" => Some(Self::Error),
            _ => None,
        }
    }

    /// Получить каноническое строковое представление уровня
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// Структурированная запись журнала логов
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Временная метка события в формате UTC
    pub timestamp: DateTime<Utc>,
    /// Уровень важности записи
    pub level: LogLevel,
    /// Целевой компонент или модуль-источник (target)
    pub target: String,
    /// Очищенное текстовое сообщение лога
    pub message: String,
    /// Исходная сырая строка записи лога
    pub raw: String,
}

/// Информация об источнике (провайдере) логов платформы
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogProvider {
    /// Уникальный идентификатор провайдера (например, `"system"`, `"plugin-snmp"`)
    pub id: String,
    /// Человекопонятное наименование источника
    pub name: String,
    /// Категория источника (`"system"`, `"module"`, `"remote"`)
    pub category: String,
    /// Опциональный путь к файлу лога на диске
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<PathBuf>,
    /// Флаг доступности провайдера для чтения
    pub available: bool,
}

/// Ответ на поисковый запрос выборки записей логов
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQueryResult {
    /// Идентификатор запрошенного провайдера
    pub provider_id: String,
    /// Общее количество записей, удовлетворяющих фильтру
    pub total: usize,
    /// Список извлеченных записей лога
    pub entries: Vec<LogEntry>,
}

/// Конфигурация сервиса логирования [`LoggerService`]
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    /// Максимальное количество записей в кольцевом буфере оперативной памяти
    pub buffer_capacity: usize,
    /// Путь к основному файлу системного лога (опционально)
    pub log_file_path: Option<PathBuf>,
    /// Максимальный размер лог-файла перед ротацией (в байтах, по умолчанию 10 МБ)
    pub max_file_size_bytes: u64,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            buffer_capacity: 5000,
            log_file_path: None,
            max_file_size_bytes: 10 * 1024 * 1024,
        }
    }
}

/// Сервис системного логирования и провайдеров логов
#[derive(Debug, Clone)]
pub struct LoggerService {
    inner: Arc<RwLock<LoggerInner>>,
    config: LoggerConfig,
}

#[derive(Debug)]
struct LoggerInner {
    buffer: VecDeque<LogEntry>,
    providers: HashMap<String, LogProvider>,
}

impl LoggerService {
    /// Создать новый экземпляр `LoggerService` с конфигурацией по умолчанию (5000 записей в буфере памяти)
    pub fn new() -> Self {
        Self::with_config(LoggerConfig::default())
    }

    /// Создать экземпляр `LoggerService` с указанным путем к системному файлу журнала
    ///
    /// # Аргументы
    /// * `path` — Путь к файлу лога (например, `"/var/log/aethercore/system.log"`).
    pub fn with_log_file(path: impl Into<PathBuf>) -> Self {
        let mut config = LoggerConfig::default();
        config.log_file_path = Some(path.into());
        Self::with_config(config)
    }

    /// Создать экземпляр с детальной пользовательской конфигурацией
    ///
    /// # Аргументы
    /// * `config` — Параметры сервиса логирования ([`LoggerConfig`]).
    pub fn with_config(config: LoggerConfig) -> Self {
        let mut providers = HashMap::new();

        // Регистрируем базовых системных провайдеров по умолчанию
        providers.insert(
            "system".to_string(),
            LogProvider {
                id: "system".to_string(),
                name: "Системный лог ядра (All Core)".to_string(),
                category: "system".to_string(),
                file_path: config.log_file_path.clone(),
                available: true,
            },
        );

        providers.insert(
            "server".to_string(),
            LogProvider {
                id: "server".to_string(),
                name: "HTTP & REST API Сервер".to_string(),
                category: "server".to_string(),
                file_path: None,
                available: true,
            },
        );

        providers.insert(
            "scheduler".to_string(),
            LogProvider {
                id: "scheduler".to_string(),
                name: "Планировщик фоновых задач".to_string(),
                category: "scheduler".to_string(),
                file_path: None,
                available: true,
            },
        );

        providers.insert(
            "auth".to_string(),
            LogProvider {
                id: "auth".to_string(),
                name: "Аутентификация и сессии".to_string(),
                category: "auth".to_string(),
                file_path: None,
                available: true,
            },
        );

        providers.insert(
            "database".to_string(),
            LogProvider {
                id: "database".to_string(),
                name: "База данных SQLite WAL".to_string(),
                category: "database".to_string(),
                file_path: None,
                available: true,
            },
        );

        providers.insert(
            "plugins".to_string(),
            LogProvider {
                id: "plugins".to_string(),
                name: "WASM Модули и шина".to_string(),
                category: "plugins".to_string(),
                file_path: None,
                available: true,
            },
        );

        let inner = LoggerInner {
            buffer: VecDeque::with_capacity(config.buffer_capacity),
            providers,
        };

        Self {
            inner: Arc::new(RwLock::new(inner)),
            config,
        }
    }

    /// Зарегистрировать дополнительный источник логов (например, для плагина или внешнего демона)
    ///
    /// # Аргументы
    /// * `provider` — Метаданные провайдера логов ([`LogProvider`]).
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Internal`](aethercore_common::error::AppError) при сбое блокировки внутреннего состояния.
    pub fn register_provider(&self, provider: LogProvider) -> Result<()> {
        let mut guard = self.inner.write().map_err(|e| AppError::internal(e.to_string()))?;
        guard.providers.insert(provider.id.clone(), provider);
        Ok(())
    }

    /// Получить отсортированный по ID список всех зарегистрированных провайдеров
    ///
    /// # Возвращаемое значение
    /// Список провайдеров [`LogProvider`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Internal`](aethercore_common::error::AppError) при ошибке блокировки.
    pub fn list_providers(&self) -> Result<Vec<LogProvider>> {
        let guard = self.inner.read().map_err(|e| AppError::internal(e.to_string()))?;
        let mut list: Vec<LogProvider> = guard.providers.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(list)
    }

    /// Записать событие в кольцевой буфер оперативной памяти и в файл на диске (если настроен)
    ///
    /// # Аргументы
    /// * `level` — Уровень важности ([`LogLevel`]).
    /// * `target` — Модуль или подсистема (например, `"auth"` или `"bus"`).
    /// * `message` — Текст сообщения.
    pub fn log(&self, level: LogLevel, target: &str, message: &str) {
        let now = Utc::now();
        let raw = format!(
            "{} | {:<5} | {} | {}",
            now.format("%Y-%m-%d %H:%M:%S"),
            level.as_str(),
            target,
            message
        );

        let entry = LogEntry {
            timestamp: now,
            level,
            target: target.to_string(),
            message: message.to_string(),
            raw: raw.clone(),
        };

        // 1. Добавляем в кольцевой буфер памяти
        if let Ok(mut guard) = self.inner.write() {
            if guard.buffer.len() >= self.config.buffer_capacity {
                guard.buffer.pop_front();
            }
            guard.buffer.push_back(entry);
        }

        // 2. Если задан файл лога, дописываем строку
        if let Some(ref path) = self.config.log_file_path {
            let _ = append_to_file_with_rotation(path, &raw, self.config.max_file_size_bytes);
        }
    }

    /// Записать структурированную ошибку платформы ([`AppError`])
    ///
    /// Ошибки со статусом `>= 500` логируются как [`LogLevel::Error`], остальные как [`LogLevel::Warn`].
    ///
    /// # Аргументы
    /// * `target` — Модуль-источник ошибки.
    /// * `error` — Экземпляр ошибки приложения [`AppError`].
    pub fn log_error(&self, target: &str, error: &AppError) {
        let level = if error.status_code >= 500 {
            LogLevel::Error
        } else {
            LogLevel::Warn
        };

        let msg = if error.details.is_null() || error.details == serde_json::json!({}) {
            format!("[{}] {}", error.code, error.message)
        } else {
            format!("[{}] {} (details: {})", error.code, error.message, error.details)
        };

        self.log(level, target, &msg);
    }

    /// Запросить выборку записей логов с фильтрацией по уровню и строке поиска
    ///
    /// # Аргументы
    /// * `provider_id` — Идентификатор провайдера (например, `"system"`).
    /// * `limit` — Максимальное количество записей (ограничивается диапазоном `1..=1000`).
    /// * `min_level` — Опциональный минимальный порог важности.
    /// * `search_query` — Опциональная подстрока поиска (регистронезависимая).
    ///
    /// # Возвращаемое значение
    /// Результат запроса [`LogQueryResult`] со списком подходящих записей.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::NotFound`](aethercore_common::error::AppError), если провайдер не зарегистрирован.
    pub fn get_logs(
        &self,
        provider_id: &str,
        limit: usize,
        min_level: Option<LogLevel>,
        search_query: Option<&str>,
    ) -> Result<LogQueryResult> {
        let guard = self.inner.read().map_err(|e| AppError::internal(e.to_string()))?;

        // Проверяем наличие провайдера
        let provider = guard
            .providers
            .get(provider_id)
            .ok_or_else(|| AppError::not_found(format!("Log provider '{}'", provider_id)))?;

        let limit = limit.clamp(1, 1000);
        let search = search_query.map(|s| s.to_lowercase());

        // Если у провайдера есть существующий файл на диске — читаем хвост файла
        if let Some(ref path) = provider.file_path {
            if path.exists() {
                let entries = read_tail_from_file(path, limit, min_level, search.as_deref())?;
                let total = entries.len();
                return Ok(LogQueryResult {
                    provider_id: provider_id.to_string(),
                    total,
                    entries,
                });
            }
        }

        // Иначе читаем из кольцевого in-memory буфера с фильтрацией по целевому источнику
        let mut matched = Vec::new();

        for entry in guard.buffer.iter().rev() {
            if !matches_provider(&entry.target, provider_id) {
                continue;
            }

            if let Some(lvl) = min_level {
                if entry.level < lvl {
                    continue;
                }
            }

            if let Some(ref q) = search {
                let in_msg = entry.message.to_lowercase().contains(q);
                let in_target = entry.target.to_lowercase().contains(q);
                let in_raw = entry.raw.to_lowercase().contains(q);
                if !in_msg && !in_target && !in_raw {
                    continue;
                }
            }

            matched.push(entry.clone());
            if matched.len() >= limit {
                break;
            }
        }

        // Возвращаем в хронологическом порядке
        matched.reverse();
        let total = matched.len();

        Ok(LogQueryResult {
            provider_id: provider_id.to_string(),
            total,
            entries: matched,
        })
    }

    /// Скачать полное содержимое журнала логов для указанного источника
    ///
    /// Если лог пишется в файл на диске и файл существует, читаются байты файла целиком.
    /// В противном случае журнал динамически генерируется из кольцевого in-memory буфера.
    ///
    /// # Аргументы
    /// * `provider_id` — Идентификатор провайдера (например, `"system"`).
    ///
    /// # Возвращаемое значение
    /// Кортеж `(бинарные_байты_лога, рекомендуемое_имя_файла)`.
    ///
    /// # Ошибки
    /// - [`AppError::NotFound`](aethercore_common::error::AppError) — если указанный провайдер не зарегистрирован.
    /// - [`AppError::Internal`](aethercore_common::error::AppError) — при ошибке чтения файла с диска.
    pub fn download_log(&self, provider_id: &str) -> Result<(Vec<u8>, String)> {
        let guard = self.inner.read().map_err(|e| AppError::internal(e.to_string()))?;
        let provider = guard
            .providers
            .get(provider_id)
            .ok_or_else(|| AppError::not_found(format!("Log provider '{}'", provider_id)))?;

        let filename = format!("{}.log", provider_id);

        if let Some(ref path) = provider.file_path {
            if path.exists() {
                let bytes = std::fs::read(path).map_err(|e| {
                    AppError::internal(format!("Failed to read log file {:?}: {}", path, e))
                })?;
                return Ok((bytes, filename));
            }
        }

        // Если файла нет на диске, генерируем из in-memory буфера
        let mut buffer = String::new();
        for entry in &guard.buffer {
            if matches_provider(&entry.target, provider_id) {
                buffer.push_str(&entry.raw);
                buffer.push('\n');
            }
        }

        Ok((buffer.into_bytes(), filename))
    }
}

/// Сопоставление цели логирования с идентификатором провайдера
fn matches_provider(target: &str, provider_id: &str) -> bool {
    match provider_id {
        "system" => true,
        "server" => {
            target.starts_with("aethercore_server")
                || target.starts_with("tower_http")
                || target.starts_with("axum")
        }
        "scheduler" => target.contains("scheduler") || target.contains("task"),
        "auth" => target.contains("auth") || target.contains("jwt") || target.contains("user"),
        "database" => target.contains("db") || target.contains("sqlx") || target.contains("database"),
        "plugins" => target.contains("plugin") || target.contains("wasm") || target.contains("bus"),
        other => target.to_lowercase().contains(&other.to_lowercase()),
    }
}

/// Tracing Subscriber Layer, перенаправляющий события в [`LoggerService`]
#[derive(Clone)]
pub struct LoggerServiceLayer {
    logger: LoggerService,
}

impl LoggerServiceLayer {
    /// Создать новый tracing subscriber layer для сервиса логирования
    pub fn new(logger: LoggerService) -> Self {
        Self { logger }
    }
}

impl<S> tracing_subscriber::Layer<S> for LoggerServiceLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();
        let level = match *metadata.level() {
            tracing::Level::ERROR => LogLevel::Error,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::DEBUG => LogLevel::Debug,
            tracing::Level::TRACE => LogLevel::Trace,
        };
        let target = metadata.target();

        struct MessageVisitor {
            message: String,
            fields: Vec<String>,
        }

        impl tracing::field::Visit for MessageVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{:?}", value).trim_matches('"').to_string();
                } else {
                    self.fields.push(format!("{}={:?}", field.name(), value));
                }
            }

            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    self.message = value.to_string();
                } else {
                    self.fields.push(format!("{}={}", field.name(), value));
                }
            }
        }

        let mut visitor = MessageVisitor {
            message: String::new(),
            fields: Vec::new(),
        };
        event.record(&mut visitor);

        let final_message = if visitor.message.is_empty() {
            visitor.fields.join(" ")
        } else if visitor.fields.is_empty() {
            visitor.message
        } else {
            format!("{} ({})", visitor.message, visitor.fields.join(" "))
        };

        if !final_message.is_empty() {
            self.logger.log(level, target, &final_message);
        }
    }
}

impl Default for LoggerService {
    fn default() -> Self {
        Self::new()
    }
}

/// Дописать текстовую строку в файл лога с проверкой и ротацией при превышении размера
///
/// Если текущий размер файла превышает `max_bytes`, файл переименовывается в `.log.1` (одноуровневая ротация).
///
/// # Аргументы
/// * `path` — Путь к целевому файлу лога.
/// * `line` — Записываемая строка (без символа переноса строки).
/// * `max_bytes` — Максимально допустимый размер файла в байтах до срабатывания ротации.
///
/// # Ошибки
/// Возвращает [`std::io::Error`] при ошибке создания директорий или записи на диск.
fn append_to_file_with_rotation(path: &Path, line: &str, max_bytes: u64) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Проверяем размер для ротации
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() >= max_bytes {
            let backup_path = path.with_extension("log.1");
            let _ = std::fs::rename(path, backup_path);
        }
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

/// Прочитать последние `limit` строк из лог-файла в обратном порядке с фильтрацией
///
/// # Аргументы
/// * `path` — Путь к файлу лога на диске.
/// * `limit` — Максимальное количество подходящих записей.
/// * `min_level` — Опциональный минимальный уровень важности для фильтрации.
/// * `search_lower` — Опциональная подстрока поиска в нижнем регистре.
///
/// # Возвращаемое значение
/// Список разобранных записей [`LogEntry`] в хронологическом порядке.
///
/// # Ошибки
/// Возвращает [`AppError::Internal`](aethercore_common::error::AppError) при сбое открытия или чтения файла.
fn read_tail_from_file(
    path: &Path,
    limit: usize,
    min_level: Option<LogLevel>,
    search_lower: Option<&str>,
) -> Result<Vec<LogEntry>> {
    let file = File::open(path).map_err(|e| AppError::internal(e.to_string()))?;
    let reader = BufReader::new(file);

    let mut matched_entries = Vec::new();
    let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

    for line in lines.iter().rev() {
        let clean = clean_ansi(line);
        let parsed = parse_log_line(&clean);

        if let Some(lvl) = min_level {
            if parsed.level < lvl {
                continue;
            }
        }

        if let Some(q) = search_lower {
            if !clean.to_lowercase().contains(q) {
                continue;
            }
        }

        matched_entries.push(parsed);
        if matched_entries.len() >= limit {
            break;
        }
    }

    matched_entries.reverse();
    Ok(matched_entries)
}

/// Распарсить строку лога в структурированную запись [`LogEntry`]
///
/// Поддерживает стандартный формат: `"YYYY-MM-DD HH:MM:SS | LEVEL | TARGET | MESSAGE"`.
/// Для неструктурированных строк выполняется эвристическое определение уровня.
///
/// # Аргументы
/// * `line` — Строка текста лога.
///
/// # Возвращаемое значение
/// Сформированный объект [`LogEntry`].
fn parse_log_line(line: &str) -> LogEntry {
    let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();

    if parts.len() >= 4 {
        let ts = DateTime::parse_from_str(parts[0], "%Y-%m-%d %H:%M:%S")
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let level = LogLevel::from_str_loose(parts[1]).unwrap_or(LogLevel::Info);
        let target = parts[2].to_string();
        let message = parts[3..].join(" | ");

        LogEntry {
            timestamp: ts,
            level,
            target,
            message,
            raw: line.to_string(),
        }
    } else {
        // Неструктурированная строка — эвристический поиск уровня
        let level = if line.contains("ERROR") || line.contains("FATAL") {
            LogLevel::Error
        } else if line.contains("WARN") {
            LogLevel::Warn
        } else if line.contains("DEBUG") {
            LogLevel::Debug
        } else if line.contains("TRACE") {
            LogLevel::Trace
        } else {
            LogLevel::Info
        };

        LogEntry {
            timestamp: Utc::now(),
            level,
            target: "system".to_string(),
            message: line.to_string(),
            raw: line.to_string(),
        }
    }
}

/// Очистить строку от управляющих ANSI escape-кодов терминала (цвета, курсор)
///
/// # Аргументы
/// * `text` — Исходный текст с возможными ANSI последовательностями.
///
/// # Возвращаемое значение
/// Очищенная строка в обычном текстовом виде.
fn clean_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_escape = false;

    for c in text.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            out.push(c);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_logger_service_in_memory() {
        let logger = LoggerService::new();
        logger.log(LogLevel::Info, "test_target", "Hello world");
        logger.log(LogLevel::Error, "test_target", "Something broke");
        logger.log(LogLevel::Debug, "test_target", "Debug details");

        let all = logger.get_logs("system", 50, None, None).unwrap();
        assert_eq!(all.total, 3);
        assert_eq!(all.entries[0].level, LogLevel::Info);
        assert_eq!(all.entries[1].level, LogLevel::Error);
        assert_eq!(all.entries[2].level, LogLevel::Debug);

        let only_errors = logger.get_logs("system", 50, Some(LogLevel::Error), None).unwrap();
        assert_eq!(only_errors.total, 1);
        assert_eq!(only_errors.entries[0].message, "Something broke");

        let search = logger.get_logs("system", 50, None, Some("world")).unwrap();
        assert_eq!(search.total, 1);
        assert_eq!(search.entries[0].message, "Hello world");
    }

    #[test]
    fn test_logger_file_and_download() {
        let dir = tempdir().unwrap();
        let log_file = dir.path().join("test.log");

        let logger = LoggerService::with_log_file(&log_file);
        logger.log(LogLevel::Info, "core", "Line 1 in file");
        logger.log(LogLevel::Warn, "core", "Line 2 warn in file");

        let (bytes, name) = logger.download_log("system").unwrap();
        assert_eq!(name, "system.log");
        let content = String::from_utf8(bytes).unwrap();
        assert!(content.contains("Line 1 in file"));
        assert!(content.contains("Line 2 warn in file"));
    }
}
