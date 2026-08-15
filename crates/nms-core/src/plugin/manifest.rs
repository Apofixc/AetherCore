// Декларативный контракт плагина: строго типизированная модель manifest.yaml
// Спецификация: MIGRATION_RUST_WASM.md, раздел 1.2 (Sandbox & UI Contract)

use semver::Version;
use serde::{Deserialize, Serialize};

/// Категория модуля: системный, прикладной или драйвер опроса устройств
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModuleType {
    System,
    #[default]
    Feature,
    Driver,
}

/// Запрос прямого сетевого доступа WASI (Режим 2: повышение прав)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NetworkCapabilities {
    /// Запрос сырых сокетов (всегда false для WASI; ICMP идет через nms:core/net)
    #[serde(default)]
    pub allow_raw_sockets: bool,
    /// Белый список хостов, к которым разрешен прямой доступ wasi:sockets
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

/// Режим доступа к проброшенной директории хоста
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostDirMode {
    ReadOnly,
    ReadWrite,
}

/// Запись о пробросе директории хоста внутрь песочницы (preopened_dir)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostDirGrant {
    pub path: String,
    pub mode: HostDirMode,
}

/// Запрос на проброс директорий файловой системы хоста
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FilesystemCapabilities {
    #[serde(default)]
    pub allow_host_dirs: Vec<HostDirGrant>,
}

/// Запрос доступа к переменным окружения ОС
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EnvironmentCapabilities {
    #[serde(default)]
    pub allow_env_vars: Vec<String>,
}

/// Системные возможности песочницы Wasmtime, запрашиваемые плагином
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Capabilities {
    #[serde(default)]
    pub network: NetworkCapabilities,
    #[serde(default)]
    pub filesystem: FilesystemCapabilities,
    #[serde(default)]
    pub environment: EnvironmentCapabilities,
}

/// Контракты шины событий: белые списки публикаций и подписок
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EventContracts {
    /// Топики, которые модуль имеет право публиковать (обязаны начинаться с "{id}.")
    #[serde(default)]
    pub publishes: Vec<String>,
    /// Топики, на которые модуль регистрирует подписку
    #[serde(default)]
    pub subscribes: Vec<String>,
}

/// Метаданные клиентского маршрута (страницы) плагина
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RouteMeta {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub requires_auth: Option<bool>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Декларация клиентской страницы плагина для Vue 3 Shell
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteDecl {
    pub path: String,
    pub name: String,
    /// Путь к компоненту: ESM dist/ui.js, Vue SFC views/*.vue или None (Schema-Driven)
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub meta: RouteMeta,
}

/// Пункт меню навигации
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuItem {
    pub path: String,
    pub label: String,
    #[serde(default)]
    pub icon: Option<String>,
}

/// Позиционирование пунктов плагина в меню навигации Shell
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MenuDecl {
    /// Расположение: sidebar или footer
    #[serde(default = "default_menu_location")]
    pub location: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub items: Vec<MenuItem>,
}

fn default_menu_location() -> String {
    "sidebar".to_string()
}

/// Декларация виджета сводной панели (Dashboard)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetDecl {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub refresh_interval: Option<u64>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub stream_endpoint: Option<String>,
    #[serde(default)]
    pub view_permission: Option<String>,
    #[serde(default)]
    pub control_permission: Option<String>,
}

/// Гранулярное разрешение, регистрируемое модулем в RBAC-матрице ядра
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermissionDecl {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Декларация жизненных хуков: имена функций, экспортируемых backend.wasm
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HooksDecl {
    #[serde(default)]
    pub install: Option<String>,
    #[serde(default)]
    pub uninstall: Option<String>,
    #[serde(default)]
    pub on_enable: Option<String>,
    #[serde(default)]
    pub on_disable: Option<String>,
}

/// Каталоги кэша и изолированного хранилища данных модуля
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AssetsDecl {
    #[serde(default)]
    pub cache_dirs: Vec<String>,
    #[serde(default)]
    pub data_dirs: Vec<String>,
}

/// Единый декларативный контракт модуля (manifest.yaml)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleManifest {
    /// Версия формата манифеста
    #[serde(default = "default_manifest_version")]
    pub manifest_version: u32,
    /// Уникальный идентификатор модуля (kebab-case, неймспейс шины и API)
    pub id: String,
    pub name: String,
    /// SemVer-версия модуля
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type", default)]
    pub module_type: ModuleType,
    /// Диапазон поддерживаемых версий ядра nms-core
    #[serde(default)]
    pub min_core_version: Option<String>,
    #[serde(default)]
    pub max_core_version: Option<String>,
    #[serde(default = "default_true")]
    pub enabled_by_default: bool,
    /// Обязательные зависимости (строят топологический порядок загрузки)
    #[serde(default)]
    pub deps: Vec<String>,
    /// Опциональные зависимости (мягкая деградация при отсутствии)
    #[serde(default)]
    pub optional_deps: Vec<String>,
    /// Идентификатор родительского модуля для иерархических субмодулей
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub capabilities: Capabilities,
    #[serde(default)]
    pub events: EventContracts,
    #[serde(default)]
    pub routes: Vec<RouteDecl>,
    #[serde(default)]
    pub menu: Option<MenuDecl>,
    #[serde(default)]
    pub widgets: Vec<WidgetDecl>,
    #[serde(default)]
    pub permissions: Vec<PermissionDecl>,
    /// JSON Schema настроек модуля (валидация + автогенерация формы)
    #[serde(default)]
    pub config_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub hooks: HooksDecl,
    #[serde(default)]
    pub assets: AssetsDecl,
}

fn default_manifest_version() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

/// Ошибка семантической валидации манифеста
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ManifestError {
    #[error("invalid module id '{0}': must be non-empty kebab-case")]
    InvalidId(String),
    #[error("invalid semver version '{0}': {1}")]
    InvalidVersion(String, String),
    #[error("published topic '{topic}' does not start with module prefix '{module_id}.'")]
    TopicSpoofing { module_id: String, topic: String },
    #[error("core version {core} is not within supported range [{min} .. {max}]")]
    IncompatibleCoreVersion {
        core: String,
        min: String,
        max: String,
    },
    #[error("module '{0}' declares dependency on itself")]
    SelfDependency(String),
    #[error("invalid config_schema: {0}")]
    InvalidConfigSchema(String),
    #[error("yaml parse error: {0}")]
    Parse(String),
}

impl ModuleManifest {
    /// Десериализация манифеста из YAML-текста
    pub fn from_yaml(yaml: &str) -> Result<Self, ManifestError> {
        serde_yaml::from_str(yaml).map_err(|e| ManifestError::Parse(e.to_string()))
    }

    /// Полная семантическая валидация манифеста относительно версии ядра
    pub fn validate(&self, core_version: &Version) -> Result<(), ManifestError> {
        // Идентификатор: непустой kebab-case (строчные буквы, цифры, дефис)
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(ManifestError::InvalidId(self.id.clone()));
        }

        // Версия модуля обязана быть корректным SemVer
        Version::parse(&self.version)
            .map_err(|e| ManifestError::InvalidVersion(self.version.clone(), e.to_string()))?;

        // Совместимость ABI: min_core_version <= core <= max_core_version
        let min = self
            .min_core_version
            .as_deref()
            .map(Version::parse)
            .transpose()
            .map_err(|e| {
                ManifestError::InvalidVersion(
                    self.min_core_version.clone().unwrap_or_default(),
                    e.to_string(),
                )
            })?;
        let max = self
            .max_core_version
            .as_deref()
            .map(Version::parse)
            .transpose()
            .map_err(|e| {
                ManifestError::InvalidVersion(
                    self.max_core_version.clone().unwrap_or_default(),
                    e.to_string(),
                )
            })?;
        let below_min = min.as_ref().map(|m| core_version < m).unwrap_or(false);
        let above_max = max.as_ref().map(|m| core_version > m).unwrap_or(false);
        if below_min || above_max {
            return Err(ManifestError::IncompatibleCoreVersion {
                core: core_version.to_string(),
                min: min.map(|v| v.to_string()).unwrap_or_else(|| "*".into()),
                max: max.map(|v| v.to_string()).unwrap_or_else(|| "*".into()),
            });
        }

        // Защита от спуфинга: публикуемые топики обязаны начинаться с "{id}."
        let prefix = format!("{}.", self.id);
        for topic in &self.events.publishes {
            if !topic.starts_with(&prefix) {
                return Err(ManifestError::TopicSpoofing {
                    module_id: self.id.clone(),
                    topic: topic.clone(),
                });
            }
        }

        // Запрет зависимости модуля от самого себя
        if self
            .deps
            .iter()
            .chain(self.optional_deps.iter())
            .any(|d| d == &self.id)
        {
            return Err(ManifestError::SelfDependency(self.id.clone()));
        }

        // Валидация config_schema как корректной JSON Schema
        if let Some(schema) = &self.config_schema {
            jsonschema::validator_for(schema)
                .map_err(|e| ManifestError::InvalidConfigSchema(e.to_string()))?;
        }

        Ok(())
    }

    /// Валидация пользовательской конфигурации модуля по config_schema
    pub fn validate_config(&self, config: &serde_json::Value) -> Result<(), String> {
        let Some(schema) = &self.config_schema else {
            return Ok(());
        };
        let validator = jsonschema::validator_for(schema).map_err(|e| e.to_string())?;
        let errors: Vec<String> = validator
            .iter_errors(config)
            .map(|e| e.to_string())
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}
