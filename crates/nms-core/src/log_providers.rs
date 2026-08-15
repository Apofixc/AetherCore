// Система провайдеров системных и удаленных логов NMS
// Реализует чтение, фильтрацию (level, search), очистку ANSI escape-кодов, скачивание логов и реестр провайдеров

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Результат получения лог-записей
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogDataResult {
    pub id: String,
    pub name: String,
    pub content: Vec<String>,
    pub total_lines: usize,
    pub matched_lines: usize,
}

/// Информация о провайдере логов для отдачи через REST API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogProviderInfo {
    pub id: String,
    pub name: String,
    pub category: String,
    pub exists: bool,
    pub size_bytes: u64,
}

/// Результат скачивания целого файла логов (содержимое, имя файла, media_type)
#[derive(Debug, Clone)]
pub struct DownloadLogResult {
    pub content: Vec<u8>,
    pub filename: String,
    pub media_type: String,
}

/// Асинхронный интерфейс провайдера логов (аналог BaseLogProvider из Python)
#[async_trait]
pub trait LogProvider: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn category(&self) -> &str;

    async fn is_available(&self) -> bool;
    async fn get_logs(&self, lines: usize, level: &str, search: &str) -> Result<LogDataResult>;
    async fn download_log(&self) -> Result<DownloadLogResult>;
    async fn get_info(&self) -> LogProviderInfo;
}

/// Очистка текста от управления терминалом ANSI escape-кодами (алиас для 1-в-1 соответствия Python API clean_ansi)
pub fn clean_ansi(text: &str) -> String {
    clean_ansi_codes(text)
}

/// Очистка текста от управления терминалом ANSI escape-кодами
pub fn clean_ansi_codes(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_escape = false;

    for ch in text.chars() {
        if ch == '\x1B' {
            in_escape = true;
            continue;
        }
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }
        result.push(ch);
    }
    result
}

/// Нормализация имени уровня логирования (WARNING -> WARN, CRITICAL/FATAL -> ERROR)
fn normalize_level(lvl: &str) -> &str {
    match lvl {
        "WARNING" => "WARN",
        "CRITICAL" | "FATAL" => "ERROR",
        other => other,
    }
}

/// Точная проверка соответствия строки указанному уровню лога (INFO, WARN/WARNING, ERROR/CRITICAL, DEBUG, TRACE)
pub fn matches_log_level(line: &str, target_level: &str) -> bool {
    let target = target_level.trim().to_uppercase();
    if target.is_empty() || target == "ALL" {
        return true;
    }

    let norm_target = normalize_level(&target);
    let line_upper = line.to_uppercase();

    // 1. Поиск структурированной метки формата [INFO], | INFO |, INFO:
    if line_upper.contains(&format!(" {} ", norm_target))
        || line_upper.contains(&format!("| {} ", norm_target))
        || line_upper.contains(&format!("[{}]", norm_target))
        || line_upper.contains(&format!("{}:", norm_target))
    {
        return true;
    }

    // Поддержка эквивалентных синонимов (например, поиск WARN находит WARNING)
    if norm_target == "WARN" && line_upper.contains("WARNING") {
        return true;
    }
    if norm_target == "ERROR" && (line_upper.contains("CRITICAL") || line_upper.contains("FATAL")) {
        return true;
    }

    // 2. Фолбэк по слову
    line_upper.contains(norm_target)
}

/// Провайдер для чтения локальных файлов логов на сервере
#[derive(Debug, Clone)]
pub struct LocalFileLogProvider {
    pub id: String,
    pub name: String,
    pub category: String,
    pub file_path: PathBuf,
}

impl LocalFileLogProvider {
    pub fn new(id: impl Into<String>, name: impl Into<String>, file_path: PathBuf) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category: "system".to_string(),
            file_path,
        }
    }
}

#[async_trait]
impl LogProvider for LocalFileLogProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn category(&self) -> &str {
        &self.category
    }

    async fn is_available(&self) -> bool {
        tokio::fs::metadata(&self.file_path).await.is_ok()
    }

    async fn get_logs(&self, lines: usize, level: &str, search: &str) -> Result<LogDataResult> {
        if !self.is_available().await {
            return Ok(LogDataResult {
                id: self.id.clone(),
                name: self.name.clone(),
                content: vec![],
                total_lines: 0,
                matched_lines: 0,
            });
        }

        let bytes = tokio::fs::read(&self.file_path).await?;
        let file_content = String::from_utf8_lossy(&bytes);
        let all_lines: Vec<&str> = file_content.lines().collect();
        let total_lines = all_lines.len();

        let search_lower = search.trim().to_lowercase();
        let mut filtered = Vec::new();

        for raw_line in all_lines {
            let clean_line = clean_ansi_codes(raw_line);

            if !search_lower.is_empty() && !clean_line.to_lowercase().contains(&search_lower) {
                continue;
            }

            if !matches_log_level(&clean_line, level) {
                continue;
            }

            filtered.push(clean_line);
        }

        let matched_lines = filtered.len();
        let limit = lines.clamp(1, 2000);
        let start_idx = matched_lines.saturating_sub(limit);

        let content = filtered[start_idx..].to_vec();

        Ok(LogDataResult {
            id: self.id.clone(),
            name: self.name.clone(),
            content,
            total_lines,
            matched_lines,
        })
    }

    async fn download_log(&self) -> Result<DownloadLogResult> {
        let content = if self.is_available().await {
            tokio::fs::read(&self.file_path).await?
        } else {
            Vec::new()
        };

        let filename = self
            .file_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("{}.log", self.id));

        Ok(DownloadLogResult {
            content,
            filename,
            media_type: "text/plain; charset=utf-8".to_string(),
        })
    }

    async fn get_info(&self) -> LogProviderInfo {
        let metadata = tokio::fs::metadata(&self.file_path).await;
        let exists = metadata.is_ok();
        let size_bytes = metadata.map(|m| m.len()).unwrap_or(0);

        LogProviderInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            category: self.category.clone(),
            exists,
            size_bytes,
        }
    }
}

/// Провайдер для получения логов с удаленного узла по HTTP API
#[derive(Debug, Clone)]
pub struct RemoteHTTPLogProvider {
    pub id: String,
    pub name: String,
    pub category: String,
    pub url: String,
    pub api_token: Option<String>,
}

impl RemoteHTTPLogProvider {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        url: impl Into<String>,
        api_token: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category: "remote".to_string(),
            url: url.into(),
            api_token,
        }
    }
}

#[async_trait]
impl LogProvider for RemoteHTTPLogProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn category(&self) -> &str {
        &self.category
    }

    async fn is_available(&self) -> bool {
        let client = reqwest::Client::new();
        let mut req = client.get(&self.url);
        if let Some(token) = &self.api_token {
            req = req.bearer_auth(token);
        }
        match req.send().await {
            Ok(res) => res.status().is_success(),
            Err(_) => false,
        }
    }

    async fn get_logs(&self, lines: usize, level: &str, search: &str) -> Result<LogDataResult> {
        let client = reqwest::Client::new();
        let mut req = client.get(&self.url).query(&[
            ("lines", lines.to_string()),
            ("level", level.to_string()),
            ("search", search.to_string()),
        ]);

        if let Some(token) = &self.api_token {
            req = req.bearer_auth(token);
        }

        match req.send().await {
            Ok(res) if res.status().is_success() => {
                let data: LogDataResult = res.json().await?;
                Ok(data)
            }
            Ok(res) => Ok(LogDataResult {
                id: self.id.clone(),
                name: self.name.clone(),
                content: vec![format!(
                    "[ERROR] Remote log returned HTTP status {}",
                    res.status()
                )],
                total_lines: 1,
                matched_lines: 1,
            }),
            Err(err) => Ok(LogDataResult {
                id: self.id.clone(),
                name: self.name.clone(),
                content: vec![format!("[ERROR] Failed to load remote log: {}", err)],
                total_lines: 1,
                matched_lines: 1,
            }),
        }
    }

    async fn download_log(&self) -> Result<DownloadLogResult> {
        let client = reqwest::Client::new();
        let download_url = format!("{}/download", self.url.trim_end_matches('/'));
        let mut req = client.get(&download_url);

        if let Some(token) = &self.api_token {
            req = req.bearer_auth(token);
        }

        match req.send().await {
            Ok(res) if res.status().is_success() => {
                let bytes = res.bytes().await?.to_vec();
                Ok(DownloadLogResult {
                    content: bytes,
                    filename: format!("{}.log", self.id),
                    media_type: "text/plain; charset=utf-8".to_string(),
                })
            }
            _ => Ok(DownloadLogResult {
                content: Vec::new(),
                filename: format!("{}.log", self.id),
                media_type: "text/plain".to_string(),
            }),
        }
    }

    async fn get_info(&self) -> LogProviderInfo {
        let exists = self.is_available().await;
        LogProviderInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            category: self.category.clone(),
            exists,
            size_bytes: 0,
        }
    }
}

/// Потокобезопасный универсальный реестр провайдеров логов NMS
#[derive(Clone, Default)]
pub struct LogProviderRegistry {
    providers: Arc<RwLock<HashMap<String, Arc<dyn LogProvider>>>>,
}

impl LogProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Регистрация нового провайдера логов в системе
    pub async fn register(&self, provider: Arc<dyn LogProvider>) {
        let mut map = self.providers.write().await;
        map.insert(provider.id().to_string(), provider);
    }

    /// Удаление провайдера из реестра
    pub async fn unregister(&self, provider_id: &str) {
        let mut map = self.providers.write().await;
        map.remove(provider_id);
    }

    /// Получение провайдера логов по ID или имени
    pub async fn get(&self, id_or_name: &str) -> Option<Arc<dyn LogProvider>> {
        let map = self.providers.read().await;
        if let Some(p) = map.get(id_or_name) {
            return Some(p.clone());
        }
        for p in map.values() {
            if p.name() == id_or_name {
                return Some(p.clone());
            }
        }
        None
    }

    /// Получение списка всех доступных провайдеров логов с их статусом
    pub async fn list_all(&self) -> Vec<LogProviderInfo> {
        let map = self.providers.read().await;
        let mut list = Vec::new();
        for provider in map.values() {
            list.push(provider.get_info().await);
        }
        list
    }
}

/// Загрузить и зарегистрировать сохраненные в БД удаленные источники логов (1-в-1 с load_remote_sources_from_db в Python)
pub async fn load_remote_sources_from_db(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    registry: &LogProviderRegistry,
    secret_key: &str,
) -> Result<()> {
    use sqlx::Row;
    let rows = sqlx::query("SELECT id, name, url, api_token FROM remote_log_sources")
        .fetch_all(pool)
        .await?;

    for row in rows {
        let id: String = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let url: String = row.try_get("url")?;
        let api_token_enc: Option<String> = row.try_get("api_token")?;

        let api_token = match api_token_enc {
            Some(ref enc) if !enc.is_empty() => {
                crate::crypto::decrypt_secret(Some(enc), secret_key).unwrap_or(None)
            }
            _ => None,
        };
        let provider = Arc::new(RemoteHTTPLogProvider::new(id, name, url, api_token));
        registry.register(provider).await;
    }
    Ok(())
}

type StreamKey = (String, String, String);

struct StreamGroup {
    subscribers: HashMap<String, tokio::sync::mpsc::UnboundedSender<String>>,
    task_handle: tokio::task::JoinHandle<()>,
}

/// Централизованный менеджер подписчиков потоков логов (1-в-1 с SharedLogStreamManager в Python)
#[derive(Clone, Default)]
pub struct SharedLogStreamManager {
    streams: Arc<RwLock<HashMap<StreamKey, StreamGroup>>>,
}

impl SharedLogStreamManager {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Подписка на поток логов (1-в-1 subscribe)
    pub async fn subscribe(
        &self,
        sub_id: String,
        log_name: String,
        level: String,
        search: String,
        registry: LogProviderRegistry,
        tx: tokio::sync::mpsc::UnboundedSender<String>,
    ) {
        let key = (log_name.clone(), level.clone(), search.clone());
        let mut map = self.streams.write().await;

        if let Some(group) = map.get_mut(&key) {
            group.subscribers.insert(sub_id, tx);
        } else {
            let mut subscribers = HashMap::new();
            subscribers.insert(sub_id, tx);

            let streams_ref = self.streams.clone();
            let key_clone = key.clone();
            let task_handle = tokio::spawn(async move {
                Self::_stream_worker(log_name, level, search, registry, streams_ref, key_clone)
                    .await;
            });

            map.insert(
                key,
                StreamGroup {
                    subscribers,
                    task_handle,
                },
            );
        }
    }

    /// Отписка от потока логов (1-в-1 unsubscribe)
    pub async fn unsubscribe(&self, sub_id: &str, log_name: &str, level: &str, search: &str) {
        let key = (log_name.to_string(), level.to_string(), search.to_string());
        let mut map = self.streams.write().await;

        if let Some(group) = map.get_mut(&key) {
            group.subscribers.remove(sub_id);
            if group.subscribers.is_empty() {
                group.task_handle.abort();
                map.remove(&key);
            }
        }
    }

    /// Закрыть все активные потоки логов при остановке бэкенда (1-в-1 close_all)
    pub async fn close_all(&self) {
        let mut map = self.streams.write().await;
        for (_, group) in map.drain() {
            group.task_handle.abort();
        }
    }

    /// Внутренний фоновый воркер стриминга логов (1-в-1 _stream_worker)
    async fn _stream_worker(
        log_name: String,
        level: String,
        search: String,
        registry: LogProviderRegistry,
        streams: Arc<RwLock<HashMap<StreamKey, StreamGroup>>>,
        key: StreamKey,
    ) {
        let provider = match registry.get(&log_name).await {
            Some(p) => p,
            None => return,
        };

        let mut last_lines_count: i64 = -1;

        loop {
            if let Ok(data) = provider.get_logs(200, &level, &search).await {
                let content = data.content;
                let matched_len = content.len() as i64;

                if matched_len != last_lines_count {
                    last_lines_count = matched_len;

                    let payload = serde_json::json!({
                        "id": provider.id(),
                        "name": provider.name(),
                        "content": content,
                        "matched_lines": data.matched_lines,
                        "total_lines": data.total_lines,
                    })
                    .to_string();

                    let mut dead_subs = Vec::new();
                    {
                        let map = streams.read().await;
                        if let Some(group) = map.get(&key) {
                            for (sub_id, tx) in &group.subscribers {
                                if tx.send(payload.clone()).is_err() {
                                    dead_subs.push(sub_id.clone());
                                }
                            }
                        } else {
                            break;
                        }
                    }

                    if !dead_subs.is_empty() {
                        let mut map = streams.write().await;
                        if let Some(group) = map.get_mut(&key) {
                            for dead_id in dead_subs {
                                group.subscribers.remove(&dead_id);
                            }
                            if group.subscribers.is_empty() {
                                map.remove(&key);
                                break;
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}
