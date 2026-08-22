//! # Спецификация и валидация манифеста плагина (`manifest.yaml`)
//!
//! Является единым декларативным контрактом между модулем (плагином)
//! и микроядром платформы (Universal Core).
//!
//! Манифест описывает:
//! - Метаданные плагина ([`ModuleManifest`]): `id`, `name`, `version`, `type`.
//! - Запрашиваемые системные права песочницы WASI ([`ModuleCapabilities`]).
//! - Публикуемые и прослушиваемые топики шины ([`ModuleEvents`]).
//! - Маршруты Vue Router для веб-интерфейса ([`ModuleRoute`]).
//! - Структуру навигационного меню ([`ModuleMenu`]) и виджеты Dashboard ([`ModuleWidget`]).
//! - Гранулярные права доступа RBAC ([`Permission`]).
//! - Схему конфигурации в формате JSON Schema.

use crate::error::{AppError, Result};
use crate::models::user::Permission;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Категория и архитектурное назначение модуля в платформе
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ModuleType {
    /// Системный модуль платформы (базовая инфраструктура ядра)
    System,
    /// Прикладной функциональный модуль (бизнес-логика, дашборды, отчеты)
    #[default]
    Feature,
    /// Драйвер опроса / интеграции сетевых устройств (SNMP, Netconf, SSH, ICMP)
    Driver,
}

/// Запрашиваемые системные права песочницы Wasmtime (WASI Capabilities)
///
/// Плагин объявляет минимально необходимые права доступа к хосту для безопасной изоляции.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleCapabilities {
    /// Сетевой доступ WASI (сокеты, разрешенные хосты)
    #[serde(default)]
    pub network: NetworkCapability,
    /// Доступ к файловой системе хоста (маппинг директорий)
    #[serde(default)]
    pub filesystem: FilesystemCapability,
    /// Доступ к переменным окружения хоста
    #[serde(default)]
    pub environment: EnvironmentCapability,
}

/// Сетевые права гостевого Wasm-модуля (WASI Sockets)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct NetworkCapability {
    /// Разрешение на создание raw/низкоуровневых WASI сокетов
    #[serde(default)]
    pub allow_raw_sockets: bool,
    /// Белый список разрешенных хостов и подсетей (например, `["192.168.1.0/24", "api.example.com"]`)
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

/// Файловые права гостевого Wasm-модуля
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FilesystemCapability {
    /// Пробрасываемые директории хоста с указанием режима доступа
    #[serde(default)]
    pub allow_host_dirs: Vec<HostDirMapping>,
}

/// Маппинг пробрасываемой директории хоста в песочницу Wasm
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostDirMapping {
    /// Абсолютный или относительный путь к директории на хосте
    pub path: String,
    /// Режим доступа: `"read_only"` или `"read_write"`
    pub mode: String,
}

/// Права доступа к переменным окружения хоста WASI
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EnvironmentCapability {
    /// Список разрешенных к чтению имен переменных окружения
    #[serde(default)]
    pub allow_env_vars: Vec<String>,
}

/// Декларация контрактов шины событий плагина
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleEvents {
    /// Топики, которые модуль имеет право публиковать (обязан быть префикс `{id}.*` для защиты от спуфинга)
    #[serde(default)]
    pub publishes: Vec<String>,
    /// Топики системной шины, на которые модуль подписывается
    #[serde(default)]
    pub subscribes: Vec<String>,
}

/// Декларация маршрута пользовательского веб-интерфейса (Vue Router)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleRoute {
    /// URL-путь маршрута (например, `"/network/topology"` или `"/settings/snmp"`)
    pub path: String,
    /// Уникальное имя маршрута во Vue Router
    pub name: String,
    /// Относительный путь к файлу Vue-компонента в директории `frontend/` плагина
    pub component: Option<String>,
    /// Метаданные страницы (заголовок, иконка, RBAC права)
    #[serde(default)]
    pub meta: ModuleRouteMeta,
}

/// Метаданные страницы маршрута веб-интерфейса
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleRouteMeta {
    /// Заголовок страницы в меню и вкладке браузера (или i18n ключ)
    pub title: String,
    /// Имя иконки интерфейса (например, имя иконки из библиотеки Lucide)
    #[serde(default)]
    pub icon: Option<String>,
    /// Имя группы меню для иерархической группировки
    #[serde(default)]
    pub group: Option<String>,
    /// Требуется ли аутентификация пользователя для доступа к странице (по умолчанию `true`)
    #[serde(default = "default_true")]
    pub requires_auth: bool,
    /// Список требуемых прав доступа RBAC (например, `["devices.view"]`)
    #[serde(default)]
    pub permissions: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// Декларация структуры меню навигации плагина
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleMenu {
    /// Расположение меню (`"sidebar"`, `"header"` или `"footer"`)
    #[serde(default = "default_sidebar")]
    pub location: String,
    /// Группа меню для категоризации
    pub group: String,
    /// Элементы меню ([`ModuleMenuItem`])
    #[serde(default)]
    pub items: Vec<ModuleMenuItem>,
}

fn default_sidebar() -> String {
    "sidebar".into()
}

/// Пункт навигационного меню
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleMenuItem {
    /// Целевой URL-путь страницы
    pub path: String,
    /// Отображаемое название пункта меню (или i18n ключ)
    pub label: String,
    /// Опциональная иконка пункта меню
    #[serde(default)]
    pub icon: Option<String>,
}

/// Виджет для Dashboard рабочего стола платформы
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModuleWidget {
    /// Уникальный идентификатор виджета
    pub id: String,
    /// Заголовок виджета в интерфейсе
    pub title: String,
    /// Относительный путь к Vue-компоненту виджета
    pub component: Option<String>,
    /// Размер карточки виджета (`"small"`, `"medium"`, `"large"`)
    #[serde(default = "default_widget_size")]
    pub size: String,
    /// Интервал автообновления данных в секундах
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u32,
    /// Опциональный REST эндпоинт поставщика данных виджета
    #[serde(default)]
    pub endpoint: Option<String>,
    /// Требуемое право доступа для просмотра виджета
    #[serde(default)]
    pub view_permission: Option<String>,
    /// Требуемое право доступа для управления через виджет
    #[serde(default)]
    pub control_permission: Option<String>,
}

fn default_widget_size() -> String {
    "medium".into()
}

fn default_refresh_interval() -> u32 {
    30
}

/// Конфигурация изолированных директорий файловых ассетов модуля
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModuleAssets {
    /// Пути к каталогам временного кэша
    #[serde(default)]
    pub cache_dirs: Vec<String>,
    /// Пути к каталогам персистентных данных
    #[serde(default)]
    pub data_dirs: Vec<String>,
}

/// Полная структура декларативного манифеста плагина `manifest.yaml`
///
/// Является центральным контрактом модуля.
///
/// # Примеры
/// ```rust
/// use aethercore_common::manifest::ModuleManifest;
///
/// let yaml = r#"
/// manifest_version: 1
/// id: ping-collector
/// name: ICMP Ping Collector
/// version: 1.0.0
/// description: Сетевой мониторинг доступности хостов через ICMP Ping
/// "#;
///
/// let manifest = ModuleManifest::from_yaml(yaml).unwrap();
/// assert_eq!(manifest.id, "ping-collector");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleManifest {
    /// Версия спецификации манифеста (по умолчанию `1`)
    #[serde(default = "default_manifest_version")]
    pub manifest_version: u32,
    /// Уникальный идентификатор плагина в формате kebab-case (например, `"snmp-collector"`)
    pub id: String,
    /// Отображаемое название модуля
    pub name: String,
    /// Версия модуля по спецификации SemVer (например, `"1.0.0"`)
    pub version: String,
    /// Краткое описание назначения модуля
    pub description: String,
    /// Тип модуля ([`ModuleType`])
    #[serde(default)]
    pub r#type: ModuleType,
    /// Флаг включения плагина по умолчанию при первой установке
    #[serde(default = "default_true")]
    pub enabled_by_default: bool,
    /// Минимально совместимая версия ядра платформы (SemVer)
    pub min_core_version: Option<String>,
    /// Максимально совместимая версия ядра платформы (SemVer)
    pub max_core_version: Option<String>,

    /// Обязательные зависимости от других модулей (список строковых ID)
    #[serde(default)]
    pub deps: Vec<String>,
    /// Опциональные зависимости от других модулей
    #[serde(default)]
    pub optional_deps: Vec<String>,
    /// Родительский модуль для группировки в иерархии
    pub parent: Option<String>,

    /// Запрашиваемые системные права песочницы WASI ([`ModuleCapabilities`])
    #[serde(default)]
    pub capabilities: ModuleCapabilities,
    /// Декларация публикуемых и прослушиваемых топиков шины событий ([`ModuleEvents`])
    #[serde(default)]
    pub events: ModuleEvents,
    /// Маршруты пользовательского интерфейса ([`ModuleRoute`])
    #[serde(default)]
    pub routes: Vec<ModuleRoute>,
    /// Меню навигации ([`ModuleMenu`])
    pub menu: Option<ModuleMenu>,
    /// Виджеты для дашборда ([`ModuleWidget`])
    #[serde(default)]
    pub widgets: Vec<ModuleWidget>,
    /// Регистрируемые плагином гранулярные права доступа RBAC ([`Permission`])
    #[serde(default)]
    pub permissions: Vec<Permission>,
    /// Схема валидации настроек в формате JSON Schema Draft 7
    pub config_schema: Option<serde_json::Value>,
    /// Хуки жизненного цикла
    #[serde(default)]
    pub hooks: HashMap<String, String>,
    /// Директории статических и кэшируемых ассетов ([`ModuleAssets`])
    pub assets: Option<ModuleAssets>,
}

fn default_manifest_version() -> u32 {
    1
}

impl ModuleManifest {
    /// Распарсить манифест плагина из YAML строки и выполнить валидацию
    ///
    /// # Аргументы
    /// * `yaml_content` — Содержимое файла `manifest.yaml` в текстовом виде.
    ///
    /// # Возвращаемое значение
    /// Провалидированный экземпляр [`ModuleManifest`].
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Validation`](crate::error::AppError), если YAML некорректен или нарушены правила безопасности.
    pub fn from_yaml(yaml_content: &str) -> Result<Self> {
        let manifest: Self = serde_yaml::from_str(yaml_content).map_err(|e| {
            AppError::validation("manifest.yaml", format!("YAML deserialization error: {}", e))
        })?;

        manifest.validate()?;
        Ok(manifest)
    }

    /// Проверить соблюдение инвариантов безопасности и корректности манифеста
    ///
    /// - Проверяет допустимость символов в `id` (только lowercase latin, digits, `-`, `_`).
    /// - Валидирует формат SemVer в поле `version`.
    /// - Защита от спуфинга событий: проверяет, что все публикуемые топики начинаются с `{id}.`.
    /// - Валидирует корректность JSON Schema в `config_schema`.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Validation`](crate::error::AppError) или [`AppError::Forbidden`](crate::error::AppError).
    pub fn validate(&self) -> Result<()> {
        // 1. Проверка формата идентификатора модуля (kebab-case)
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(AppError::validation(
                "id",
                format!(
                    "Module id '{}' must contain only lowercase latin letters, digits, '-' and '_'",
                    self.id
                ),
            ));
        }

        // 2. Проверка корректности SemVer версии модуля
        Version::parse(&self.version).map_err(|e| {
            AppError::validation("version", format!("Invalid SemVer version '{}': {}", self.version, e))
        })?;

        // 3. Валидация безопасности топиков событий:
        // Все публикуемые топики обязаны начинаться с '{id}.' (защита от спуфинга)
        let prefix = format!("{}.", self.id);
        for topic in &self.events.publishes {
            if !topic.starts_with(&prefix) {
                return Err(AppError::forbidden(format!(
                    "Module '{}' cannot publish to topic '{}'. Topic must start with '{}'",
                    self.id, topic, prefix
                )));
            }
        }

        // 4. Проверка JSON Schema конфигурации (если указана)
        if let Some(schema_json) = &self.config_schema {
            jsonschema::validator_for(schema_json).map_err(|e| {
                AppError::validation("config_schema", format!("Invalid JSON Schema: {}", e))
            })?;
        }

        Ok(())
    }

    /// Проверить совместимость манифеста плагина с текущей версией ядра платформы
    ///
    /// # Аргументы
    /// * `core_version_str` — Версия ядра в формате SemVer (например, `"2.0.0"`).
    ///
    /// # Возвращаемое значение
    /// `Ok(true)` если версия совместима с `min_core_version` и `max_core_version`.
    pub fn is_compatible_with_core(&self, core_version_str: &str) -> Result<bool> {
        let core_version =
            Version::parse(core_version_str).map_err(|e| {
                AppError::validation("core_version", e.to_string())
            })?;

        if let Some(min_ver) = &self.min_core_version {
            let req = VersionReq::parse(&format!(">={}", min_ver)).map_err(|e| {
                AppError::validation("min_core_version", e.to_string())
            })?;
            if !req.matches(&core_version) {
                return Ok(false);
            }
        }

        if let Some(max_ver) = &self.max_core_version {
            let req = VersionReq::parse(&format!("<={}", max_ver)).map_err(|e| {
                AppError::validation("max_core_version", e.to_string())
            })?;
            if !req.matches(&core_version) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Провалидировать пользовательские настройки модуля по схеме `config_schema`
    ///
    /// # Аргументы
    /// * `config` — JSON-значение конфигурации.
    ///
    /// # Ошибки
    /// Возвращает [`AppError::Validation`](crate::error::AppError) при несоответствии схеме.
    pub fn validate_config(&self, config: &serde_json::Value) -> Result<()> {
        if let Some(schema_json) = &self.config_schema {
            let validator = jsonschema::validator_for(schema_json).map_err(|e| {
                AppError::validation("config_schema", format!("Schema error: {}", e))
            })?;

            if let Err(err) = validator.validate(config) {
                return Err(AppError::validation("config", err.to_string()));
            }
        }
        Ok(())
    }
}

/// Выполнить топологическую сортировку графа зависимостей модулей (DAG)
///
/// Использует алгоритм Кана (Kahn's algorithm) для определения очередности загрузки.
/// Модули, не имеющие зависимостей, загружаются первыми.
///
/// # Аргументы
/// * `manifests` — Срез манифестов доступных в системе модулей.
///
/// # Возвращаемое значение
/// Вектор манифестов в порядке безопасной инициализации.
///
/// # Ошибки
/// - [`AppError::NotFound`](crate::error::AppError) — если требуемая зависимость отсутствует в системе.
/// - [`AppError::BadRequest`](crate::error::AppError) — при обнаружении циклической зависимости (Cyclic Dependency).
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
                return Err(AppError::not_found(format!(
                    "Required dependency '{}' for module '{}'",
                    dep, m.id
                )));
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
        return Err(AppError::bad_request("Cyclic dependency detected among modules"));
    }

    Ok(result)
}
