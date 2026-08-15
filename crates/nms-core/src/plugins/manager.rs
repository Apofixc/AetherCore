//! # Менеджер плагинов платформы (PluginManager)
//!
//! Отвечает за обнаружение, валидацию, загрузку по DAG зависимостей,
//! изоляцию настроек и управление жизненным циклом плагинов.

use super::loader::PluginPackage;
use crate::bus::EventBus;
use crate::db::kv::KvStore;
use crate::db::Db;
use nms_common::error::{AppError, Result};
use nms_common::i18n::{global, Locale};
use nms_common::manifest::{resolve_module_dag, ModuleManifest};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info};

/// Состояние установленного плагина
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub package: PluginPackage,
    pub is_enabled: bool,
}

/// Центральный менеджер плагинов
#[derive(Debug, Clone)]
pub struct PluginManager {
    db: Db,
    bus: EventBus,
    plugins: Arc<RwLock<HashMap<String, InstalledPlugin>>>,
}

impl PluginManager {
    /// Создать новый менеджер плагинов
    pub fn new(db: Db, bus: EventBus) -> Self {
        Self {
            db,
            bus,
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Сканировать каталог модулей и загрузить плагины
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

    /// Зарегистрировать плагин в реестре
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

    /// Получить список всех установленных плагинов
    pub fn list_plugins(&self) -> Vec<InstalledPlugin> {
        let registry = self.plugins.read().expect("Lock poisoned");
        registry.values().cloned().collect()
    }

    /// Получить установленный плагин по ID
    pub fn get_plugin(&self, plugin_id: &str) -> Option<InstalledPlugin> {
        let registry = self.plugins.read().expect("Lock poisoned");
        registry.get(plugin_id).cloned()
    }

    /// Включить плагин
    pub async fn enable_plugin(&self, plugin_id: &str) -> Result<()> {
        let mut registry = self.plugins.write().expect("Lock poisoned");
        let plugin = registry.get_mut(plugin_id).ok_or_else(|| AppError::NotFound {
            resource: format!("Plugin '{}'", plugin_id),
        })?;

        plugin.is_enabled = true;
        info!("Plugin '{}' enabled", plugin_id);
        Ok(())
    }

    /// Отключить плагин
    pub async fn disable_plugin(&self, plugin_id: &str) -> Result<()> {
        let mut registry = self.plugins.write().expect("Lock poisoned");
        let plugin = registry.get_mut(plugin_id).ok_or_else(|| AppError::NotFound {
            resource: format!("Plugin '{}'", plugin_id),
        })?;

        plugin.is_enabled = false;
        info!("Plugin '{}' disabled", plugin_id);
        Ok(())
    }

    /// Получить настройки плагина
    pub async fn get_plugin_config(&self, plugin_id: &str) -> Result<Option<serde_json::Value>> {
        let _ = self.get_plugin(plugin_id).ok_or_else(|| AppError::NotFound {
            resource: format!("Plugin '{}'", plugin_id),
        })?;

        let kv = KvStore::for_plugin(self.db.clone(), plugin_id);
        kv.get("config").await
    }

    /// Сохранить настройки плагина с валидацией по manifest.config_schema
    pub async fn set_plugin_config(
        &self,
        plugin_id: &str,
        config_value: &serde_json::Value,
    ) -> Result<()> {
        let plugin = self.get_plugin(plugin_id).ok_or_else(|| AppError::NotFound {
            resource: format!("Plugin '{}'", plugin_id),
        })?;

        // Валидация по JSON Schema
        plugin.package.manifest.validate_config(config_value)?;

        // Сохранение в изолированное хранилище
        let kv = KvStore::for_plugin(self.db.clone(), plugin_id);
        kv.set("config", config_value).await?;

        // Оповещение через шину событий
        let event = nms_common::models::events::EventMessage::reliable(
            format!("{}.config_changed", plugin_id),
            plugin_id,
            config_value.clone(),
        );
        self.bus.publish(event).await?;

        info!("Updated config for plugin '{}'", plugin_id);
        Ok(())
    }

    /// Получить фронтенд-ассет плагина по относительному пути
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_manager_lifecycle() {
        let db = Db::init_in_memory().await.unwrap();
        let bus = EventBus::new(db.clone());
        let manager = PluginManager::new(db, bus);

        let manifest = ModuleManifest::from_yaml(
            r#"
id: "demo-plugin"
name: "Demo Plugin"
version: "1.0.0"
description: "Demo"
config_schema:
  type: "object"
  required: ["interval"]
  properties:
    interval:
      type: "integer"
      minimum: 1
"#,
        )
        .unwrap();

        let mut locales = HashMap::new();
        locales.insert("ru".into(), r#"{"status": "Работает"}"#.into());

        let mut frontend_assets = HashMap::new();
        frontend_assets.insert("frontend/dist/ui.js".into(), b"console.log('hi');".to_vec());

        let package = PluginPackage {
            manifest,
            manifest_raw: vec![],
            backend_wasm: None,
            signature: None,
            locales,
            frontend_assets,
        };

        manager.register_plugin(package).await.unwrap();

        // Проверяем i18n
        assert_eq!(
            global().translate(Locale::Ru, "demo-plugin.status", &[]),
            "Работает"
        );

        // Проверяем фронтенд ассет
        let asset = manager.get_frontend_asset("demo-plugin", "dist/ui.js");
        assert_eq!(asset, Some(b"console.log('hi');".to_vec()));

        // Проверяем валидацию и сохранение настроек
        let valid_config = serde_json::json!({"interval": 5});
        assert!(manager
            .set_plugin_config("demo-plugin", &valid_config)
            .await
            .is_ok());

        let invalid_config = serde_json::json!({"interval": 0});
        assert!(manager
            .set_plugin_config("demo-plugin", &invalid_config)
            .await
            .is_err());
    }
}
