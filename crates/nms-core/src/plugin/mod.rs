// Плагинный движок ядра: обнаружение, валидация, DAG-порядок и запуск WASM-песочниц
// Спецификация: MIGRATION_RUST_WASM.md, разделы 1.2 (жизненный цикл) и 1.4 (взаимодействие)

pub mod dag;
pub mod discovery;
pub mod host;
pub mod manifest;

use anyhow::Result;
use dag::{toposort, DagNode, TopoResult};
use discovery::{discover_plugins, DiscoveredPlugin};
use host::{
    build_engine, load_component_cached, spawn_epoch_ticker, spawn_plugin_actor, HostState,
    LifecycleHook, PluginCommand, RunningPlugin, DEFAULT_MEMORY_LIMIT_BYTES, RPC_REGISTRY,
};
use manifest::ModuleManifest;
use semver::Version;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};
use wasmtime::Engine;

/// Текущая версия ядра nms-core для проверки совместимости ABI манифестов
pub fn core_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 1, 0))
}

/// Брокер межмодульного RPC (используется Host API nms:core/rpc)
pub(crate) async fn engine_rpc_dispatch(
    target_module: &str,
    method: &str,
    params: &[u8],
) -> Result<Vec<u8>, String> {
    host::dispatch_rpc(target_module, method, params).await
}

/// Итог загрузки одного модуля
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleLoadStatus {
    /// Модуль успешно инстанциирован в песочнице
    Running,
    /// Манифест зарегистрирован, но backend.wasm отсутствует (UI-only модуль)
    ManifestOnly,
    /// Модуль заблокирован по результатам валидации
    Blocked(String),
}

/// Реестр загруженного модуля: манифест + статус
#[derive(Debug, Clone)]
pub struct ModuleRecord {
    pub manifest: ModuleManifest,
    pub status: ModuleLoadStatus,
}

/// Плагинный движок ядра: владеет Wasmtime Engine и реестром активных плагинов
pub struct PluginEngine {
    engine: Engine,
    /// Каталог с пакетами .nms-plugin / dev-каталогами
    modules_dir: PathBuf,
    /// Каталог AOT-кэша предкомпилированных компонентов
    cache_dir: PathBuf,
    /// Пул БД для KV-хранилищ и журнала событий плагинов
    db_pool: Option<sqlx::SqlitePool>,
    /// Разрешить загрузку неподписанных плагинов (dev-режим)
    pub allow_unsigned_plugins: bool,
    /// Доверенные публичные ключи Ed25519 для проверки подписей пакетов
    pub trusted_keys: Vec<ed25519_dalek::VerifyingKey>,
    /// Реестр модулей: манифесты и статусы загрузки
    pub registry: HashMap<String, ModuleRecord>,
    /// Активные акторы плагинов
    running: HashMap<String, RunningPlugin>,
    /// Фоновый таймер инкремента эпох Wasmtime
    _epoch_ticker: tokio::task::JoinHandle<()>,
}

impl PluginEngine {
    /// Создание движка плагинов с настройкой Wasmtime (epoch interruption + component model)
    pub fn new(
        modules_dir: impl Into<PathBuf>,
        cache_dir: impl Into<PathBuf>,
        db_pool: Option<sqlx::SqlitePool>,
        allow_unsigned_plugins: bool,
    ) -> Result<Self> {
        let engine = build_engine()?;
        let epoch_ticker = spawn_epoch_ticker(&engine);
        Ok(Self {
            engine,
            modules_dir: modules_dir.into(),
            cache_dir: cache_dir.into(),
            db_pool,
            allow_unsigned_plugins,
            trusted_keys: Vec::new(),
            registry: HashMap::new(),
            running: HashMap::new(),
            _epoch_ticker: epoch_ticker,
        })
    }

    /// Полный цикл загрузки: Discovery -> Validation -> DAG -> Sandbox Instantiation
    pub async fn load_all(&mut self) -> Result<TopoResult> {
        let core = core_version();

        // 1. Обнаружение пакетов в каталоге modules/
        let discovered: Vec<DiscoveredPlugin> = discover_plugins(&self.modules_dir)
            .into_iter()
            .filter_map(|r| match r {
                Ok(p) => Some(p),
                Err(e) => {
                    warn!("Plugin package rejected during discovery: {}", e);
                    None
                }
            })
            .collect();

        // 2. Семантическая валидация манифестов и политики подписи
        let mut valid: Vec<DiscoveredPlugin> = Vec::new();
        for plugin in discovered {
            let id = plugin.manifest.id.clone();
            if let Err(e) = plugin.manifest.validate(&core) {
                warn!("Module '{}' blocked by manifest validation: {}", id, e);
                self.registry.insert(
                    id,
                    ModuleRecord {
                        manifest: plugin.manifest.clone(),
                        status: ModuleLoadStatus::Blocked(e.to_string()),
                    },
                );
                continue;
            }
            // Политика подписей: невалидная подпись блокируется всегда,
            // отсутствие подписи блокируется в продакшн-режиме
            match plugin.signature_status(&self.trusted_keys) {
                Some(false) => {
                    warn!("Module '{}' blocked: invalid Ed25519 signature", id);
                    self.registry.insert(
                        id,
                        ModuleRecord {
                            manifest: plugin.manifest.clone(),
                            status: ModuleLoadStatus::Blocked(
                                "invalid Ed25519 signature".to_string(),
                            ),
                        },
                    );
                    continue;
                }
                None if !self.allow_unsigned_plugins => {
                    warn!(
                        "Unsigned module '{}' blocked: allow_unsigned_plugins = false",
                        id
                    );
                    self.registry.insert(
                        id,
                        ModuleRecord {
                            manifest: plugin.manifest.clone(),
                            status: ModuleLoadStatus::Blocked(
                                "unsigned plugin rejected".to_string(),
                            ),
                        },
                    );
                    continue;
                }
                None => warn!("Unsigned module '{}' loaded in dev mode", id),
                Some(true) => {}
            }
            valid.push(plugin);
        }

        // 3. Построение DAG и топологический порядок инициализации
        let nodes: Vec<DagNode> = valid
            .iter()
            .map(|p| DagNode {
                id: p.manifest.id.clone(),
                deps: p.manifest.deps.clone(),
                optional_deps: p.manifest.optional_deps.clone(),
            })
            .collect();
        let topo = toposort(&nodes)?;
        let by_id: HashMap<String, DiscoveredPlugin> = valid
            .into_iter()
            .map(|p| (p.manifest.id.clone(), p))
            .collect();

        // 4. Инстанциирование в порядке DAG: провайдеры раньше потребителей
        for module_id in &topo.order {
            let plugin = &by_id[module_id];
            let status = match &plugin.wasm_bytes {
                Some(wasm_bytes) => match self.instantiate(plugin, wasm_bytes).await {
                    Ok(()) => ModuleLoadStatus::Running,
                    Err(e) => {
                        // Сбой модуля изолируется без влияния на ядро и другие модули
                        error!("Module '{}' failed to start: {}", module_id, e);
                        ModuleLoadStatus::Blocked(e.to_string())
                    }
                },
                None => ModuleLoadStatus::ManifestOnly,
            };
            self.registry.insert(
                module_id.clone(),
                ModuleRecord {
                    manifest: plugin.manifest.clone(),
                    status,
                },
            );
        }

        info!(
            "Plugin engine loaded {} module(s), {} running",
            self.registry.len(),
            self.running.len()
        );
        Ok(topo)
    }

    /// Инстанциирование одного плагина: AOT-кэш, песочница, актор и подписки шины
    async fn instantiate(&mut self, plugin: &DiscoveredPlugin, wasm_bytes: &[u8]) -> Result<()> {
        let manifest = &plugin.manifest;
        let component =
            load_component_cached(&self.engine, &self.cache_dir, &manifest.id, wasm_bytes)?;

        let host_state = HostState::new(
            &manifest.id,
            manifest.events.publishes.clone(),
            manifest
                .deps
                .iter()
                .chain(manifest.optional_deps.iter())
                .cloned()
                .collect(),
            self.db_pool.clone(),
            DEFAULT_MEMORY_LIMIT_BYTES,
        );

        let running = spawn_plugin_actor(&self.engine, component, host_state).await?;

        // Регистрация Mailbox в брокере RPC
        RPC_REGISTRY
            .lock()
            .await
            .insert(manifest.id.clone(), running.mailbox.clone());

        // Мост шина событий -> Mailbox актора по подпискам манифеста
        for pattern in &manifest.events.subscribes {
            let mailbox = running.mailbox.clone();
            let pattern_owned = pattern.clone();
            crate::bus::EVENT_BUS.subscribe(
                pattern_owned,
                std::sync::Arc::new(move |event: &crate::bus::SystemEvent| {
                    let payload = serde_json::to_vec(&event.payload).unwrap_or_default();
                    let _ = mailbox.try_send(PluginCommand::Event {
                        topic: event.topic.clone(),
                        payload,
                    });
                }),
            );
        }

        // Хук включения модуля (on-enable)
        let (tx, rx) = tokio::sync::oneshot::channel();
        running
            .mailbox
            .send(PluginCommand::Lifecycle {
                hook: LifecycleHook::Enable,
                reply: tx,
            })
            .await
            .ok();
        if let Ok(Err(e)) = rx.await {
            warn!(
                "Module '{}' on-enable hook returned error: {}",
                manifest.id, e
            );
        }

        self.running.insert(manifest.id.clone(), running);
        Ok(())
    }

    /// Контролируемая остановка модуля: хук on-disable, дерегистрация RPC и остановка актора
    pub async fn stop_module(&mut self, module_id: &str) -> Result<()> {
        if let Some(running) = self.running.remove(module_id) {
            let (tx, rx) = tokio::sync::oneshot::channel();
            running
                .mailbox
                .send(PluginCommand::Lifecycle {
                    hook: LifecycleHook::Disable,
                    reply: tx,
                })
                .await
                .ok();
            let _ = rx.await;
            // Атомарная дерегистрация: последующие rpc.call вернут Err(NotAvailable)
            RPC_REGISTRY.lock().await.remove(module_id);
            host::shutdown_plugin(&running).await.ok();
            if let Some(record) = self.registry.get_mut(module_id) {
                record.status = ModuleLoadStatus::Blocked("disabled by administrator".to_string());
            }
        }
        Ok(())
    }

    /// Список идентификаторов активных модулей
    pub fn running_modules(&self) -> Vec<String> {
        self.running.keys().cloned().collect()
    }

    /// Каталог пакетов плагинов
    pub fn modules_dir(&self) -> &Path {
        &self.modules_dir
    }
}
