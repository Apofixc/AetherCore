//! # Менеджер плагинов платформы (PluginManager)
//!
//! Отвечает за обнаружение, валидацию, загрузку по DAG зависимостей,
//! изоляцию настроек и управление жизненным циклом плагинов.

use super::loader::PluginPackage;
use crate::bus::EventBus;
use crate::db::kv::KvStore;
use crate::db::Db;
use aethercore_common::error::{AppError, Result};
use aethercore_common::i18n::{global, Locale};
use aethercore_common::manifest::{resolve_module_dag, ModuleManifest};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info};

/// Модель состояния установленного в системе плагина
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    /// Разобранный пакет плагина (манифест, Wasm байткод, ассеты, локали)
    pub package: PluginPackage,
    /// Флаг включения/активности плагина
    pub is_enabled: bool,
}

/// Центральный менеджер плагинов платформы
///
/// Управляет регистрацией плагинов, разрешением графа зависимостей (DAG),
/// валидацией конфигураций по JSON-схеме и доставкой событий об изменении настроек.
#[derive(Debug, Clone)]
pub struct PluginManager {
    db: Db,
    bus: EventBus,
    plugins: Arc<RwLock<HashMap<String, InstalledPlugin>>>,
}

impl PluginManager {
    /// Создать новый менеджер плагинов
    ///
    /// # Аргументы
    /// * `db` — Экземпляр базы данных платформы ([`Db`]).
    /// * `bus` — Шина событий для уведомления компонентов платформы ([`EventBus`]).
    pub fn new(db: Db, bus: EventBus) -> Self {
        Self {
            db,
            bus,
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Сканировать каталог модулей, разрешить зависимости и загрузить плагины в порядке DAG
    ///
    /// Поддерживает как упакованные архивы (`.nms-plugin`, `.zip`), так и распакованные папки плагинов.
    ///
    /// # Аргументы
    /// * `modules_dir` — Путь к каталогу с плагинами на диске.
    ///
    /// # Возвращаемое значение
    /// Количество успешно зарегистрированных плагинов.
    ///
    /// # Ошибки
    /// Возвращает [`AppError`] при циклических зависимостях или критических ошибках манифестов.
    pub async fn load_plugins_from_dir(&self, modules_dir: &Path) -> Result<usize> {
        if !modules_dir.exists() {
            let _ = tokio::fs::create_dir_all(modules_dir).await;
            return Ok(0);
        }

        let mut loaded_packages = Vec::new();

        if let Ok(mut entries) = tokio::fs::read_dir(modules_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_file() {
                    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if ext == "nms-plugin" || ext == "zip" {
                        match tokio::fs::read(&path).await {
                            Ok(bytes) => match PluginPackage::from_zip_bytes(&bytes) {
                                Ok(pkg) => {
                                    loaded_packages.push(pkg);
                                }
                                Err(e) => {
                                    error!("Failed to parse plugin {:?}: {}", path, e);
                                }
                            },
                            Err(e) => {
                                error!("Failed to read plugin file {:?}: {}", path, e);
                            }
                        }
                    }
                } else if path.is_dir() {
                    // Режим локальной распакованной папки
                    match PluginPackage::from_directory(&path) {
                        Ok(pkg) => {
                            loaded_packages.push(pkg);
                        }
                        Err(e) => {
                            debug!("Skipping directory {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        if loaded_packages.is_empty() {
            return Ok(0);
        }

        // Построение DAG зависимостей и топологическая сортировка
        let manifests: Vec<ModuleManifest> =
            loaded_packages.iter().map(|p| p.manifest.clone()).collect();
        let sorted_manifests = resolve_module_dag(&manifests)?;

        let mut package_map: HashMap<String, PluginPackage> = loaded_packages
            .into_iter()
            .map(|p| (p.manifest.id.clone(), p))
            .collect();

        let mut count = 0;
        for sorted_m in sorted_manifests {
            if let Some(package) = package_map.remove(&sorted_m.id) {
                self.register_plugin(package).await?;
                count += 1;
            }
        }

        info!("Successfully loaded and registered {} plugins", count);
        Ok(count)
    }

    /// Зарегистрировать разобранный пакет плагина в реестре платформы
    ///
    /// Автоматически регистрирует словари i18n плагина в глобальном реестре локализации
    /// и сохраняет метаданные плагина в потокобезопасном хранилище.
    ///
    /// # Аргументы
    /// * `package` — Пакет плагина [`PluginPackage`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError`] при сбое регистрации.
    pub async fn register_plugin(&self, package: PluginPackage) -> Result<()> {
        let plugin_id = package.manifest.id.clone();
        let is_enabled = package.manifest.enabled_by_default;

        // 1. Регистрируем локали плагина в глобальном реестре i18n
        for (lang_code, json_str) in &package.locales {
            let locale = Locale::from_str_relaxed(lang_code);
            let _ = global().register_json(locale, Some(&plugin_id), json_str);
        }

        // 2. Добавляем плагин в реестр
        let mut registry = self.plugins.write().expect("Lock poisoned");
        registry.insert(
            plugin_id.clone(),
            InstalledPlugin {
                package,
                is_enabled,
            },
        );

        info!("Registered plugin '{}' (enabled: {})", plugin_id, is_enabled);
        Ok(())
    }

    /// Получить список всех установленных в платформе плагинов
    ///
    /// # Возвращаемое значение
    /// Вектор установленных плагинов [`InstalledPlugin`].
    pub fn list_plugins(&self) -> Vec<InstalledPlugin> {
        let registry = self.plugins.read().expect("Lock poisoned");
        registry.values().cloned().collect()
    }

    /// Получить установленный плагин по его уникальному идентификатору
    ///
    /// # Аргументы
    /// * `plugin_id` — Идентификатор плагина (например, `"snmp"`).
    ///
    /// # Возвращаемое значение
    /// `Some(InstalledPlugin)`, если плагин зарегистрирован, иначе `None`.
    pub fn get_plugin(&self, plugin_id: &str) -> Option<InstalledPlugin> {
        let registry = self.plugins.read().expect("Lock poisoned");
        registry.get(plugin_id).cloned()
    }

    /// Активировать (включить) установленный плагин
    ///
    /// # Аргументы
    /// * `plugin_id` — Идентификатор плагина.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::NotFound`](aethercore_common::error::AppError), если плагин с таким ID не найден.
    pub async fn enable_plugin(&self, plugin_id: &str) -> Result<()> {
        let mut registry = self.plugins.write().expect("Lock poisoned");
        let plugin = registry.get_mut(plugin_id).ok_or_else(|| {
            AppError::module_not_found(plugin_id)
        })?;

        plugin.is_enabled = true;
        info!("Plugin '{}' enabled", plugin_id);
        Ok(())
    }

    /// Деактивировать (отключить) установленный плагин
    ///
    /// # Аргументы
    /// * `plugin_id` — Идентификатор плагина.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::NotFound`](aethercore_common::error::AppError), если плагин с таким ID не найден.
    pub async fn disable_plugin(&self, plugin_id: &str) -> Result<()> {
        let mut registry = self.plugins.write().expect("Lock poisoned");
        let plugin = registry.get_mut(plugin_id).ok_or_else(|| {
            AppError::module_not_found(plugin_id)
        })?;

        plugin.is_enabled = false;
        info!("Plugin '{}' disabled", plugin_id);
        Ok(())
    }

    /// Получить текущую сохраненную конфигурацию плагина
    ///
    /// Конфигурация извлекается из изолированного неймспейса `module:{plugin_id}` в KV-хранилище.
    ///
    /// # Аргументы
    /// * `plugin_id` — Идентификатор плагина.
    ///
    /// # Возвращаемое значение
    /// `Ok(Some(serde_json::Value))` с настройками плагина или `Ok(None)`, если настройки еще не задавались.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::NotFound`](aethercore_common::error::AppError), если плагин не зарегистрирован,
    /// или [`AppError::Database`](aethercore_common::error::AppError) при сбое запроса к БД.
    pub async fn get_plugin_config(&self, plugin_id: &str) -> Result<Option<serde_json::Value>> {
        let _ = self.get_plugin(plugin_id).ok_or_else(|| {
            AppError::module_not_found(plugin_id)
        })?;

        let kv = KvStore::for_plugin(self.db.clone(), plugin_id);
        kv.get("config").await
    }

    /// Сохранить и применить новую конфигурацию плагина
    ///
    /// 1. Валидирует переданный JSON-объект по `manifest.config_schema`.
    /// 2. Сохраняет настройки в изолированное хранилище `module:{plugin_id}`.
    /// 3. Публикует надежное событие `{plugin_id}.config_changed` в системную шину событий [`EventBus`].
    ///
    /// # Аргументы
    /// * `plugin_id` — Идентификатор плагина.
    /// * `config_value` — Валидируемый JSON-объект настроек.
    ///
    /// # Ошибки
    /// - [`AppError::NotFound`](aethercore_common::error::AppError) — плагин не найден.
    /// - [`AppError::Validation`](aethercore_common::error::AppError) — настройки не соответствуют схеме.
    /// - [`AppError::Database`](aethercore_common::error::AppError) — сбой сохранения в базе данных.
    pub async fn set_plugin_config(
        &self,
        plugin_id: &str,
        config_value: &serde_json::Value,
    ) -> Result<()> {
        let plugin = self.get_plugin(plugin_id).ok_or_else(|| {
            AppError::module_not_found(plugin_id)
        })?;

        // Валидация по JSON Schema
        plugin.package.manifest.validate_config(config_value)?;

        // Сохранение в изолированное хранилище
        let kv = KvStore::for_plugin(self.db.clone(), plugin_id);
        kv.set("config", config_value).await?;

        // Оповещение через шину событий
        let event = aethercore_common::models::events::EventMessage::reliable(
            format!("{}.config_changed", plugin_id),
            plugin_id,
            config_value.clone(),
        );
        self.bus.publish(event).await?;

        info!("Updated config for plugin '{}'", plugin_id);
        Ok(())
    }

    /// Получить бинарное содержимое статического веб-ассета плагина
    ///
    /// # Аргументы
    /// * `plugin_id` — Идентификатор плагина.
    /// * `asset_path` — Относительный путь к файлу (например, `"index.js"` или `"frontend/style.css"`).
    ///
    /// # Возвращаемое значение
    /// `Some(Vec<u8>)` с байтами файла, либо `None`, если плагин или ассет не найден.
    pub fn get_frontend_asset(&self, plugin_id: &str, asset_path: &str) -> Option<Vec<u8>> {
        let plugin = self.get_plugin(plugin_id)?;
        // Ищем точное совпадение или с префиксом "frontend/"
        let full_key = if asset_path.starts_with("frontend/") {
            asset_path.to_string()
        } else {
            format!("frontend/{}", asset_path)
        };

        plugin.package.frontend_assets.get(&full_key).cloned()
    }
}
