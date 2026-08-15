// Хост-рантайм WASM-плагинов: Wasmtime Component Model, песочница и Host API (nms:core/*)
// Спецификация: MIGRATION_RUST_WASM.md, разделы 1.2.В (Sandboxing & Concurrency), 1.4.А (Host WIT API)

use crate::bus::{SystemEvent, EVENT_BUS};
use crate::db::{get_system_setting, set_system_setting};
use crate::notify::{NotificationSeverity, NotifyParams};
use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

// Генерация типизированных биндингов мира plugin из WIT-контракта nms:core@2.0.0
wasmtime::component::bindgen!({
    path: "../../wit",
    world: "plugin",
    imports: { default: async },
    exports: { default: async },
});

/// Лимит линейной памяти инстанса плагина по умолчанию (128 МБ)
pub const DEFAULT_MEMORY_LIMIT_BYTES: usize = 128 * 1024 * 1024;
/// Таймаут одного host-вызова внешнего I/O (tokio::time::timeout)
pub const PER_CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// Дедлайн гостевого кода в тиках эпохи (1 тик = 1 секунда системного таймера)
pub const GUEST_EPOCH_DEADLINE_TICKS: u64 = 10;

/// Состояние хоста, доступное каждому инстансу плагина (WasiCtx + сервисы ядра)
pub struct HostState {
    /// Идентификатор модуля (неймспейс шины событий и KV-хранилища)
    module_id: String,
    /// Белый список публикуемых топиков из manifest.yaml (защита от спуфинга)
    allowed_publishes: Vec<String>,
    /// Контекст WASI-песочницы (capabilities: fs, env, sockets)
    wasi_ctx: WasiCtx,
    resource_table: ResourceTable,
    /// Лимитер линейной памяти инстанса (ResourceLimiter)
    limits: StoreLimits,
    /// Пул БД для персистентного KV-хранилища (module:{id}:*)
    db_pool: Option<sqlx::SqlitePool>,
    /// In-Memory fallback KV-хранилище при отсутствии пула БД
    memory_kv: HashMap<String, Vec<u8>>,
    /// Зарегистрированные интервальные таймеры плагина
    timers: HashMap<String, f64>,
    /// Обязательные и опциональные зависимости (валидация rpc.call)
    deps: Vec<String>,
    /// HTTP-клиент ядра с пулом соединений (nms:core/net)
    http_client: reqwest::Client,
}

impl HostState {
    /// Создание состояния хоста для инстанса плагина с настройкой capabilities WASI
    pub fn new(
        module_id: &str,
        allowed_publishes: Vec<String>,
        deps: Vec<String>,
        db_pool: Option<sqlx::SqlitePool>,
        memory_limit_bytes: usize,
    ) -> Self {
        // Базовая песочница: без наследования env/args/сети; только stdout/stderr для отладки
        let wasi_ctx = WasiCtxBuilder::new()
            .inherit_stdout()
            .inherit_stderr()
            .build();
        Self {
            module_id: module_id.to_string(),
            allowed_publishes,
            wasi_ctx,
            resource_table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new()
                .memory_size(memory_limit_bytes)
                .build(),
            db_pool,
            memory_kv: HashMap::new(),
            timers: HashMap::new(),
            deps,
            http_client: reqwest::Client::new(),
        }
    }

    /// Префиксированный ключ изолированного KV-хранилища модуля
    fn kv_key(&self, key: &str) -> String {
        format!("module:{}:{}", self.module_id, key)
    }

    /// Проверка права публикации топика по белому списку манифеста
    fn check_publish_allowed(&self, topic: &str) -> Result<(), String> {
        let prefix = format!("{}.", self.module_id);
        if !topic.starts_with(&prefix) {
            return Err(format!(
                "topic '{}' is outside module namespace '{}'",
                topic, prefix
            ));
        }
        if !self.allowed_publishes.is_empty() && !self.allowed_publishes.iter().any(|t| t == topic)
        {
            return Err(format!(
                "topic '{}' is not declared in manifest events.publishes",
                topic
            ));
        }
        Ok(())
    }

    /// Доступ к лимитеру ресурсов Store (линейная память)
    pub fn limits_mut(&mut self) -> &mut StoreLimits {
        &mut self.limits
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi_ctx,
            table: &mut self.resource_table,
        }
    }
}

// ===== Реализация Host API: nms:core/events =====
impl nms::core::events::Host for HostState {
    /// Публикация эфемерной телеметрии в broadcast-шину ядра
    async fn publish_telemetry(&mut self, topic: String, payload: Vec<u8>) -> Result<(), String> {
        self.check_publish_allowed(&topic)?;
        // Конвенция payload: стандартные топики несут JSON, *.stream/*.bin — MessagePack
        let value = if topic.ends_with(".stream") || topic.ends_with(".bin") {
            serde_json::json!({ "binary": true, "len": payload.len() })
        } else {
            serde_json::from_slice(&payload).unwrap_or(serde_json::Value::Null)
        };
        let event = SystemEvent::new(topic, value, self.module_id.clone());
        EVENT_BUS.publish(event, false).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Публикация надежного события с записью в персистентный журнал SQLite
    async fn publish_reliable(&mut self, topic: String, payload: Vec<u8>) -> Result<(), String> {
        self.check_publish_allowed(&topic)?;
        let value: serde_json::Value =
            serde_json::from_slice(&payload).unwrap_or(serde_json::Value::Null);
        if let Some(pool) = self.db_pool.clone() {
            crate::db::record_event_in_db(&pool, &topic, &value, None, Some(&topic))
                .await
                .map_err(|e| e.to_string())?;
        }
        let event = SystemEvent::new(topic, value, self.module_id.clone());
        EVENT_BUS.publish(event, false).map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ===== Реализация Host API: nms:core/storage =====
impl nms::core::storage::Host for HostState {
    /// Чтение значения из изолированного KV-хранилища модуля
    async fn get(&mut self, key: String) -> Result<Option<Vec<u8>>, String> {
        let full_key = self.kv_key(&key);
        if let Some(pool) = self.db_pool.clone() {
            let value =
                tokio::time::timeout(PER_CALL_TIMEOUT, get_system_setting(&pool, &full_key))
                    .await
                    .map_err(|_| "timed_out".to_string())?
                    .map_err(|e| e.to_string())?;
            Ok(value.map(|v| v.into_bytes()))
        } else {
            Ok(self.memory_kv.get(&full_key).cloned())
        }
    }

    /// Запись значения в изолированное KV-хранилище модуля
    async fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), String> {
        let full_key = self.kv_key(&key);
        if let Some(pool) = self.db_pool.clone() {
            let text = String::from_utf8_lossy(&value).to_string();
            tokio::time::timeout(
                PER_CALL_TIMEOUT,
                set_system_setting(&pool, &full_key, &text),
            )
            .await
            .map_err(|_| "timed_out".to_string())?
            .map_err(|e| e.to_string())?;
        } else {
            self.memory_kv.insert(full_key, value);
        }
        Ok(())
    }

    /// Удаление ключа из изолированного KV-хранилища модуля
    async fn delete(&mut self, key: String) -> Result<(), String> {
        let full_key = self.kv_key(&key);
        if let Some(pool) = self.db_pool.clone() {
            sqlx::query("DELETE FROM system_settings WHERE key = ?;")
                .bind(&full_key)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            self.memory_kv.remove(&full_key);
        }
        Ok(())
    }
}

// ===== Реализация Host API: nms:core/logger =====
impl nms::core::logger::Host for HostState {
    async fn log_trace(&mut self, msg: String) {
        tracing::trace!(module = %self.module_id, "{}", msg);
    }
    async fn log_debug(&mut self, msg: String) {
        debug!(module = %self.module_id, "{}", msg);
    }
    async fn log_info(&mut self, msg: String) {
        info!(module = %self.module_id, "{}", msg);
    }
    async fn log_warn(&mut self, msg: String) {
        warn!(module = %self.module_id, "{}", msg);
    }
    async fn log_error(&mut self, msg: String) {
        error!(module = %self.module_id, "{}", msg);
    }
}

// ===== Реализация Host API: nms:core/notify =====
impl nms::core::notify::Host for HostState {
    /// Отправка алерта пользователям через NotificationEngine ядра
    async fn send_alert(
        &mut self,
        severity: String,
        title: String,
        message: String,
    ) -> Result<(), String> {
        let sev = match severity.as_str() {
            // Критические алерты отображаются уровнем error (эскалация выполняется ядром)
            "critical" | "error" => NotificationSeverity::Error,
            "warning" => NotificationSeverity::Warning,
            "success" => NotificationSeverity::Success,
            _ => NotificationSeverity::Info,
        };
        let params = NotifyParams {
            user_id: "*".to_string(),
            severity: sev,
            title,
            body: message,
            module_id: self.module_id.clone(),
            ..Default::default()
        };
        let engine = crate::notify::NotificationEngine::new(EVENT_BUS.clone());
        tokio::time::timeout(PER_CALL_TIMEOUT, engine.notify(params))
            .await
            .map_err(|_| "timed_out".to_string())?
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ===== Реализация Host API: nms:core/cron =====
impl nms::core::cron::Host for HostState {
    /// Регистрация интервального таймера плагина (доставка через on-timer)
    async fn schedule_interval(
        &mut self,
        seconds: f64,
        timer_id: String,
    ) -> Result<String, String> {
        if seconds <= 0.0 {
            return Err("interval must be positive".to_string());
        }
        self.timers.insert(timer_id.clone(), seconds);
        Ok(timer_id)
    }

    /// Отмена ранее зарегистрированного таймера
    async fn cancel(&mut self, timer_id: String) -> Result<(), String> {
        self.timers
            .remove(&timer_id)
            .map(|_| ())
            .ok_or_else(|| format!("timer '{}' is not registered", timer_id))
    }
}

// ===== Реализация Host API: nms:core/rpc =====
impl nms::core::rpc::Host for HostState {
    /// Межмодульный RPC через брокер ядра: валидация deps и маршрутизация вызова
    async fn call(
        &mut self,
        target_module: String,
        method: String,
        params: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        // Ядро валидирует, что целевой модуль указан в deps/optional_deps манифеста
        if !self.deps.iter().any(|d| d == &target_module) {
            return Err(format!(
                "module '{}' is not declared in deps of '{}'",
                target_module, self.module_id
            ));
        }
        // Маршрутизация к активному инстансу целевого плагина выполняется брокером PluginEngine
        super::engine_rpc_dispatch(&target_module, &method, &params).await
    }
}

// ===== Реализация Host API: nms:core/net =====
impl nms::core::net::Host for HostState {
    /// ICMP Ping от имени ядра (Фаза 1: TCP-эмуляция до нативного ICMP-сокета)
    async fn ping(
        &mut self,
        host: String,
        timeout_ms: u32,
    ) -> Result<nms::core::net::PingResult, String> {
        // Нативный ICMP требует CAP_NET_RAW; в Фазе 1 применяется TCP-probe к echo-порту
        let started = std::time::Instant::now();
        let reachable = tokio::time::timeout(
            Duration::from_millis(timeout_ms as u64),
            tokio::net::TcpStream::connect((host.as_str(), 80u16)),
        )
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false);
        Ok(nms::core::net::PingResult {
            reachable,
            latency_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// HTTP/HTTPS запрос через пул соединений reqwest ядра
    async fn http_fetch(
        &mut self,
        method: String,
        url: String,
        body: Option<Vec<u8>>,
    ) -> Result<nms::core::net::HttpResponse, String> {
        let http_method: reqwest::Method = method
            .parse()
            .map_err(|_| format!("invalid method '{}'", method))?;
        let mut request = self.http_client.request(http_method, &url);
        if let Some(bytes) = body {
            request = request.body(bytes);
        }
        let response = tokio::time::timeout(PER_CALL_TIMEOUT, request.send())
            .await
            .map_err(|_| "timed_out".to_string())?
            .map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let bytes = tokio::time::timeout(PER_CALL_TIMEOUT, response.bytes())
            .await
            .map_err(|_| "timed_out".to_string())?
            .map_err(|e| e.to_string())?;
        Ok(nms::core::net::HttpResponse {
            status,
            body: bytes.to_vec(),
        })
    }

    /// Проверка доступности TCP-порта узла (probe)
    async fn tcp_probe(
        &mut self,
        host: String,
        port: u16,
        timeout_ms: u32,
    ) -> Result<bool, String> {
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms as u64),
            tokio::net::TcpStream::connect((host.as_str(), port)),
        )
        .await;
        Ok(matches!(result, Ok(Ok(_))))
    }
}

// ===== Реализация Host API: nms:core/i18n =====
impl nms::core::i18n::Host for HostState {
    /// Перевод ключа через системный движок i18n с подстановкой параметров
    async fn translate(&mut self, key: String, args: Vec<(String, String)>) -> String {
        let engine = crate::i18n::I18nEngine::new();
        let params: Vec<(&str, &str)> =
            args.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        engine.tr("en", &key, None, Some(&params))
    }
}

/// Создание сконфигурированного движка Wasmtime (Component Model + epoch interruption)
pub fn build_engine() -> Result<Engine> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    // Прерывание зацикленного гостевого кода по дедлайну эпох (контролируемый Trap)
    config.epoch_interruption(true);
    Engine::new(&config).map_err(|e| anyhow!("failed to build wasmtime engine: {e}"))
}

/// Запуск системного таймера инкремента эпох (1 тик в секунду)
pub fn spawn_epoch_ticker(engine: &Engine) -> tokio::task::JoinHandle<()> {
    let weak = engine.weak();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            match weak.upgrade() {
                Some(engine) => engine.increment_epoch(),
                None => break,
            }
        }
    })
}

/// AOT Precompiled Cache: путь к кэшированному бинарнику cache/modules/{id}-{hash}.cwasm
pub fn aot_cache_path(cache_dir: &Path, module_id: &str, wasm_bytes: &[u8]) -> PathBuf {
    let hash = Sha256::digest(wasm_bytes);
    cache_dir.join(format!("{}-{:x}.cwasm", module_id, hash))
}

/// Загрузка компонента с AOT-кэшем: Component::deserialize при валидном кэше,
/// иначе Cranelift AOT-компиляция и сохранение через component.serialize()
pub fn load_component_cached(
    engine: &Engine,
    cache_dir: &Path,
    module_id: &str,
    wasm_bytes: &[u8],
) -> Result<Component> {
    let cache_path = aot_cache_path(cache_dir, module_id, wasm_bytes);
    if cache_path.exists() {
        // Кэш валиден по хэшу SHA-256: мгновенная десериализация (< 5 мс)
        // SAFETY: файл кэша создается только самим ядром через component.serialize()
        match unsafe { Component::deserialize_file(engine, &cache_path) } {
            Ok(component) => {
                info!("Loaded module '{}' from AOT cache", module_id);
                return Ok(component);
            }
            Err(e) => warn!(
                "AOT cache for '{}' is stale ({}), recompiling",
                module_id, e
            ),
        }
    }
    // Cranelift AOT-компиляция и сохранение результата в кэш
    let component = Component::new(engine, wasm_bytes)
        .map_err(|e| anyhow!("failed to compile component: {e}"))?;
    if let Ok(serialized) = component.serialize() {
        std::fs::create_dir_all(cache_dir).ok();
        if let Err(e) = std::fs::write(&cache_path, serialized) {
            warn!("Failed to write AOT cache for '{}': {}", module_id, e);
        }
    }
    Ok(component)
}

/// Команда акторного Mailbox плагина (последовательная диспетчеризация вызовов)
pub enum PluginCommand {
    /// Доставка события шины (event-consumer.on-event)
    Event { topic: String, payload: Vec<u8> },
    /// Срабатывание таймера (timer-consumer.on-timer)
    Timer { timer_id: String },
    /// Входящий межмодульный RPC (rpc-handler.handle-rpc)
    Rpc {
        method: String,
        params: Vec<u8>,
        reply: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
    },
    /// Хук жизненного цикла: on-install / on-enable / on-disable / on-uninstall
    Lifecycle {
        hook: LifecycleHook,
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    /// Остановка актора плагина
    Shutdown,
}

/// Тип хука жизненного цикла плагина
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleHook {
    Install,
    Uninstall,
    Enable,
    Disable,
}

/// Запущенный инстанс плагина: канал Mailbox и хэндл актора
pub struct RunningPlugin {
    pub module_id: String,
    pub mailbox: mpsc::Sender<PluginCommand>,
    pub actor: tokio::task::JoinHandle<()>,
}

/// Инстанцирование плагина как изолированного актора (Single-Instance Actor)
/// Вызовы on-event / on-timer / handle-rpc диспетчеризуются строго последовательно.
pub async fn spawn_plugin_actor(
    engine: &Engine,
    component: Component,
    host_state: HostState,
) -> Result<RunningPlugin> {
    let module_id = host_state.module_id.clone();

    // Линкер компонента: WASI + все Host API интерфейсы nms:core/*
    let mut linker: Linker<HostState> = Linker::new(engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)
        .map_err(|e| anyhow!("failed to add WASI to linker: {e}"))?;
    Plugin::add_to_linker::<HostState, wasmtime::component::HasSelf<HostState>>(
        &mut linker,
        |state: &mut HostState| state,
    )
    .map_err(|e| anyhow!("failed to add nms:core host interfaces: {e}"))?;

    let mut store = Store::new(engine, host_state);
    // Лимит линейной памяти инстанса через ResourceLimiter
    store.limiter(|state| state.limits_mut());
    // Дедлайн гостевого кода: прерывание по эпохам с обновлением перед каждым вызовом
    store.set_epoch_deadline(GUEST_EPOCH_DEADLINE_TICKS);

    let plugin = Plugin::instantiate_async(&mut store, &component, &linker)
        .await
        .map_err(|e| anyhow!("failed to instantiate plugin component: {e}"))?;

    let (tx, mut rx) = mpsc::channel::<PluginCommand>(256);

    // Акторный цикл: последовательная обработка Mailbox (Non-Reentrancy Guard)
    let actor_id = module_id.clone();
    let actor = tokio::spawn(async move {
        while let Some(command) = rx.recv().await {
            // Обновление дедлайна эпох перед каждым гостевым вызовом
            store.set_epoch_deadline(GUEST_EPOCH_DEADLINE_TICKS);
            match command {
                PluginCommand::Event { topic, payload } => {
                    let result = plugin
                        .nms_core_event_consumer()
                        .call_on_event(&mut store, &topic, &payload)
                        .await;
                    handle_guest_result(&actor_id, "on-event", result);
                }
                PluginCommand::Timer { timer_id } => {
                    let result = plugin
                        .nms_core_timer_consumer()
                        .call_on_timer(&mut store, &timer_id)
                        .await;
                    handle_guest_result(&actor_id, "on-timer", result);
                }
                PluginCommand::Rpc {
                    method,
                    params,
                    reply,
                } => {
                    let result = plugin
                        .nms_core_rpc_handler()
                        .call_handle_rpc(&mut store, &method, &params)
                        .await;
                    let response = match result {
                        Ok(inner) => inner,
                        Err(trap) => {
                            error!("Plugin '{}' trapped in handle-rpc: {}", actor_id, trap);
                            Err(format!("plugin trap: {}", trap))
                        }
                    };
                    let _ = reply.send(response);
                }
                PluginCommand::Lifecycle { hook, reply } => {
                    let lifecycle = plugin.nms_core_lifecycle();
                    let result = match hook {
                        LifecycleHook::Install => lifecycle.call_on_install(&mut store).await,
                        LifecycleHook::Uninstall => lifecycle.call_on_uninstall(&mut store).await,
                        LifecycleHook::Enable => lifecycle.call_on_enable(&mut store).await,
                        LifecycleHook::Disable => lifecycle.call_on_disable(&mut store).await,
                    };
                    let response = match result {
                        Ok(inner) => inner,
                        Err(trap) => {
                            error!("Plugin '{}' trapped in lifecycle hook: {}", actor_id, trap);
                            Err(format!("plugin trap: {}", trap))
                        }
                    };
                    let _ = reply.send(response);
                }
                PluginCommand::Shutdown => break,
            }
        }
        info!("Plugin actor '{}' stopped", actor_id);
    });

    Ok(RunningPlugin {
        module_id,
        mailbox: tx,
        actor,
    })
}

/// Обработка результата гостевого вызова: изоляция Trap/OOM/Timeout без влияния на ядро
fn handle_guest_result(
    module_id: &str,
    call: &str,
    result: Result<Result<(), String>, wasmtime::Error>,
) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(guest_error)) => {
            warn!(
                "Plugin '{}' returned error from {}: {}",
                module_id, call, guest_error
            );
        }
        Err(trap) => {
            // Фатальный сбой WASM (Trap / OOM / Epoch Timeout): изоляция инцидента
            error!("Plugin '{}' trapped in {}: {}", module_id, call, trap);
        }
    }
}

/// Реестр Mailbox активных плагинов: module_id -> канал команд актора
pub type RpcRegistry = Arc<Mutex<HashMap<String, mpsc::Sender<PluginCommand>>>>;

/// Глобальный реестр Mailbox активных плагинов для брокера RPC
pub static RPC_REGISTRY: std::sync::LazyLock<RpcRegistry> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Диспетчеризация межмодульного RPC-вызова через реестр активных плагинов
pub async fn dispatch_rpc(
    target_module: &str,
    method: &str,
    params: &[u8],
) -> Result<Vec<u8>, String> {
    let mailbox = {
        let registry = RPC_REGISTRY.lock().await;
        registry.get(target_module).cloned()
    };
    // Атомарный хост-трамплин: выключенный модуль возвращает типизированную ошибку NotAvailable
    let Some(mailbox) = mailbox else {
        return Err("NotAvailable".to_string());
    };
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    mailbox
        .send(PluginCommand::Rpc {
            method: method.to_string(),
            params: params.to_vec(),
            reply: reply_tx,
        })
        .await
        .map_err(|_| "NotAvailable".to_string())?;
    tokio::time::timeout(PER_CALL_TIMEOUT, reply_rx)
        .await
        .map_err(|_| "timed_out".to_string())?
        .map_err(|_| "NotAvailable".to_string())?
}

/// Утилита: контролируемая остановка актора плагина
pub async fn shutdown_plugin(plugin: &RunningPlugin) -> Result<()> {
    plugin
        .mailbox
        .send(PluginCommand::Shutdown)
        .await
        .map_err(|_| anyhow!("plugin '{}' mailbox already closed", plugin.module_id))?;
    Ok(())
}
