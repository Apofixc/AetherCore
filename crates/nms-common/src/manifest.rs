//! # Спецификация и валидация манифеста плагина (`manifest.yaml`)
//!
//! Является единым декларативным контрактом между модулем (плагином)
//! и микроядром платформы (Universal Core).

use crate::error::{AppError, Result};
use crate::models::user::Permission;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Категория модуля
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModuleType {
    /// Системный модуль платформы
    System,
    /// Прикладной функциональный модуль
    #[default]
    Feature,
    /// Драйвер опроса / интеграции устройств
    Driver,
}

/// Запрашиваемые системные права песочницы Wasmtime (Capabilities)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleCapabilities {
    /// Сетевой доступ WASI
    #[serde(default)]
    pub network: NetworkCapability,
    /// Доступ к файловой системе хоста
    #[serde(default)]
    pub filesystem: FilesystemCapability,
    /// Доступ к переменным окружения хоста
    #[serde(default)]
    pub environment: EnvironmentCapability,
}

/// Сетевые права WASI
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct NetworkCapability {
    /// Разрешение на создание WASI сокетов
    #[serde(default)]
    pub allow_raw_sockets: bool,
    /// Белый список разрешенных хостов
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

/// Файловые права WASI
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FilesystemCapability {
    /// Пробрасываемые директории хоста
    #[serde(default)]
    pub allow_host_dirs: Vec<HostDirMapping>,
}

/// Маппинг пробрасываемой директории
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostDirMapping {
    pub path: String,
    pub mode: String, // "read_only" или "read_write"
}

/// Права доступа к окружению WASI
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EnvironmentCapability {
    /// Список доступных переменных окружения
    #[serde(default)]
    pub allow_env_vars: Vec<String>,
}

/// Декларация контрактов шины событий
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleEvents {
    /// Топики, которые модуль имеет право публиковать (обязан быть префикс `{id}.*`)
    #[serde(default)]
    pub publishes: Vec<String>,
    /// Топики, на которые модуль подписывается
    #[serde(default)]
    pub subscribes: Vec<String>,
}

/// Декларация маршрута пользовательского интерфейса (Vue Shell Route)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleRoute {
    pub path: String,
    pub name: String,
    pub component: Option<String>,
    #[serde(default)]
    pub meta: ModuleRouteMeta,
}

/// Метаданные маршрута UI
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleRouteMeta {
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default = "default_true")]
    pub requires_auth: bool,
    #[serde(default)]
    pub permissions: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// Декларация меню навигации
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleMenu {
    #[serde(default = "default_sidebar")]
    pub location: String, // "sidebar" или "footer"
    pub group: String,
    #[serde(default)]
    pub items: Vec<ModuleMenuItem>,
}

fn default_sidebar() -> String {
    "sidebar".into()
}

/// Пункт меню
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleMenuItem {
    pub path: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
}

/// Виджет для Dashboard
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleWidget {
    pub id: String,
    pub title: String,
    pub component: Option<String>,
    #[serde(default = "default_widget_size")]
    pub size: String,
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u32,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub view_permission: Option<String>,
    #[serde(default)]
    pub control_permission: Option<String>,
}

fn default_widget_size() -> String {
    "medium".into()
}

fn default_refresh_interval() -> u32 {
    30
}

/// Директории ассетов модуля
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleAssets {
    #[serde(default)]
    pub cache_dirs: Vec<String>,
    #[serde(default)]
    pub data_dirs: Vec<String>,
}

/// Полная структура манифеста плагина `manifest.yaml`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleManifest {
    #[serde(default = "default_manifest_version")]
    pub manifest_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub r#type: ModuleType,
    #[serde(default = "default_true")]
    pub enabled_by_default: bool,
    pub min_core_version: Option<String>,
    pub max_core_version: Option<String>,

    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub optional_deps: Vec<String>,
    pub parent: Option<String>,

    #[serde(default)]
    pub capabilities: ModuleCapabilities,
    #[serde(default)]
    pub events: ModuleEvents,
    #[serde(default)]
    pub routes: Vec<ModuleRoute>,
    pub menu: Option<ModuleMenu>,
    #[serde(default)]
    pub widgets: Vec<ModuleWidget>,
    #[serde(default)]
    pub permissions: Vec<Permission>,
    pub config_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub hooks: HashMap<String, String>,
    pub assets: Option<ModuleAssets>,
}

fn default_manifest_version() -> u32 {
    1
}

impl ModuleManifest {
    /// Распарсить манифест из YAML строки
    pub fn from_yaml(yaml_content: &str) -> Result<Self> {
        let manifest: Self = serde_yaml::from_str(yaml_content).map_err(|e| AppError::Validation {
            field: "manifest.yaml".into(),
            details: format!("YAML deserialization error: {}", e),
        })?;

        manifest.validate()?;
        Ok(manifest)
    }

    /// Валидация корректности манифеста и правил безопасности
    pub fn validate(&self) -> Result<()> {
        // 1. Проверка формата идентификатора модуля (kebab-case)
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(AppError::Validation {
                field: "id".into(),
                details: format!(
                    "Module id '{}' must contain only lowercase latin letters, digits, '-' and '_'",
                    self.id
                ),
            });
        }

        // 2. Проверка корректности SemVer версии модуля
        Version::parse(&self.version).map_err(|e| AppError::Validation {
            field: "version".into(),
            details: format!("Invalid SemVer version '{}': {}", self.version, e),
        })?;

        // 3. Валидация безопасности топиков событий:
        // Все публикуемые топики обязаны начинаться с '{id}.' (защита от спуфинга)
        let prefix = format!("{}.", self.id);
        for topic in &self.events.publishes {
            if !topic.starts_with(&prefix) {
                return Err(AppError::Forbidden {
                    permission: format!(
                        "Module '{}' cannot publish to topic '{}'. Topic must start with '{}'",
                        self.id, topic, prefix
                    ),
                });
            }
        }

        // 4. Проверка JSON Schema конфигурации (если указана)
        if let Some(schema_json) = &self.config_schema {
            jsonschema::validator_for(schema_json).map_err(|e| AppError::Validation {
                field: "config_schema".into(),
                details: format!("Invalid JSON Schema: {}", e),
            })?;
        }

        Ok(())
    }

    /// Проверка совместимости версии ядра
    pub fn is_compatible_with_core(&self, core_version_str: &str) -> Result<bool> {
        let core_version =
            Version::parse(core_version_str).map_err(|e| AppError::Validation {
                field: "core_version".into(),
                details: e.to_string(),
            })?;

        if let Some(min_ver) = &self.min_core_version {
            let req = VersionReq::parse(&format!(">={}", min_ver)).map_err(|e| {
                AppError::Validation {
                    field: "min_core_version".into(),
                    details: e.to_string(),
                }
            })?;
            if !req.matches(&core_version) {
                return Ok(false);
            }
        }

        if let Some(max_ver) = &self.max_core_version {
            let req = VersionReq::parse(&format!("<={}", max_ver)).map_err(|e| {
                AppError::Validation {
                    field: "max_core_version".into(),
                    details: e.to_string(),
                }
            })?;
            if !req.matches(&core_version) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Провалидировать настройки пользователя по `config_schema`
    pub fn validate_config(&self, config: &serde_json::Value) -> Result<()> {
        if let Some(schema_json) = &self.config_schema {
            let validator = jsonschema::validator_for(schema_json).map_err(|e| {
                AppError::Validation {
                    field: "config_schema".into(),
                    details: format!("Schema error: {}", e),
                }
            })?;

            if let Err(err) = validator.validate(config) {
                return Err(AppError::Validation {
                    field: "config".into(),
                    details: err.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Топологическая сортировка DAG зависимостей модулей
pub fn resolve_module_dag(manifests: &[ModuleManifest]) -> Result<Vec<ModuleManifest>> {
    let mut manifest_map: HashMap<String, &ModuleManifest> = HashMap::new();
    let mut in_degrees: HashMap<String, usize> = HashMap::new();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();

    for m in manifests {
        manifest_map.insert(m.id.clone(), m);
        in_degrees.insert(m.id.clone(), 0);
        adj.insert(m.id.clone(), Vec::new());
    }

    // Построение ребер графа (dep -> dependent)
    for m in manifests {
        for dep in &m.deps {
            if !manifest_map.contains_key(dep) {
                return Err(AppError::NotFound {
                    resource: format!("Required dependency '{}' for module '{}'", dep, m.id),
                });
            }
            adj.get_mut(dep).unwrap().push(m.id.clone());
            *in_degrees.get_mut(&m.id).unwrap() += 1;
        }
    }

    // Алгоритм Кана (Kahn's algorithm)
    let mut queue: Vec<String> = in_degrees
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    let mut result: Vec<ModuleManifest> = Vec::new();
    let mut visited_count = 0;

    while let Some(node_id) = queue.pop() {
        visited_count += 1;
        result.push((*manifest_map.get(&node_id).unwrap()).clone());

        if let Some(neighbors) = adj.get(&node_id) {
            for neighbor in neighbors {
                let deg = in_degrees.get_mut(neighbor).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push(neighbor.clone());
                }
            }
        }
    }

    if visited_count != manifests.len() {
        return Err(AppError::BadRequest {
            details: "Cyclic dependency detected among modules".into(),
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MANIFEST: &str = r#"
manifest_version: 1
id: "example-plugin"
name: "Example Demo Plugin"
version: "1.0.0"
description: "Universal demo plugin for tests"
type: "feature"
enabled_by_default: true
min_core_version: "2.0.0"

events:
  publishes:
    - "example-plugin.status_updated"
  subscribes:
    - "core.system_started"

config_schema:
  type: "object"
  required: ["interval_sec"]
  properties:
    interval_sec:
      type: "integer"
      minimum: 1
"#;

    #[test]
    fn test_parse_and_validate_manifest() {
        let manifest = ModuleManifest::from_yaml(SAMPLE_MANIFEST).expect("Valid YAML");
        assert_eq!(manifest.id, "example-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert!(manifest.is_compatible_with_core("2.0.0").unwrap());
        assert!(manifest.is_compatible_with_core("2.5.0").unwrap());
        assert!(!manifest.is_compatible_with_core("1.9.0").unwrap());
    }

    #[test]
    fn test_spoofing_prevention() {
        let invalid_manifest = r#"
id: "evil-plugin"
name: "Evil Plugin"
version: "1.0.0"
description: "Attempts to spoof events"
events:
  publishes:
    - "other-plugin.secret_event"
"#;
        let res = ModuleManifest::from_yaml(invalid_manifest);
        assert!(res.is_err());
    }

    #[test]
    fn test_config_schema_validation() {
        let manifest = ModuleManifest::from_yaml(SAMPLE_MANIFEST).unwrap();

        let valid_config = serde_json::json!({"interval_sec": 10});
        assert!(manifest.validate_config(&valid_config).is_ok());

        let invalid_config = serde_json::json!({"interval_sec": 0});
        assert!(manifest.validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_dag_resolution() {
        let mut m1 = ModuleManifest::from_yaml(SAMPLE_MANIFEST).unwrap();
        m1.id = "mod-a".into();
        m1.events.publishes = vec!["mod-a.event".into()];

        let mut m2 = ModuleManifest::from_yaml(SAMPLE_MANIFEST).unwrap();
        m2.id = "mod-b".into();
        m2.events.publishes = vec!["mod-b.event".into()];
        m2.deps = vec!["mod-a".into()];

        let mut m3 = ModuleManifest::from_yaml(SAMPLE_MANIFEST).unwrap();
        m3.id = "mod-c".into();
        m3.events.publishes = vec!["mod-c.event".into()];
        m3.deps = vec!["mod-b".into()];

        let resolved = resolve_module_dag(&[m3.clone(), m1.clone(), m2.clone()]).unwrap();
        let ids: Vec<String> = resolved.into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["mod-a", "mod-b", "mod-c"]);
    }
}
