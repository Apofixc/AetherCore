//! # Подсистема интернационализации и локализации (i18n)
//!
//! Обеспечивает сквозную локализацию сообщений об ошибках, системных событий,
//! аудит-логов и интерфейсов на русском ([`Locale::Ru`]) и английском ([`Locale::En`]) языках.
//! Загружает базовые системные словари из каталога `locales/` и поддерживает
//! динамическую регистрацию словарей плагинов (модулей) с префиксами `{module_id}.{key}`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// Системный русский словарь, подключаемый на этапе сборки
const BUILTIN_RU_JSON: &str = include_str!("../locales/ru.json");
/// Системный английский словарь, подключаемый на этапе сборки
const BUILTIN_EN_JSON: &str = include_str!("../locales/en.json");

/// Поддерживаемые языковые локали платформы
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    /// Русский язык (локаль по умолчанию)
    #[default]
    Ru,
    /// Английский язык
    En,
}

impl Locale {
    /// Получить канонический двухбуквенный строковый код локали (`"ru"` или `"en"`)
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }

    /// Безопасный парсинг локали из HTTP-заголовков (например, `Accept-Language: ru-RU,ru;q=0.9`) или произвольной строки
    ///
    /// # Аргументы
    /// * `s` — Входная строка локали или заголовка.
    ///
    /// # Возвращаемое значение
    /// Возвращает [`Locale::En`], если строка начинается с `"en"`, иначе по умолчанию [`Locale::Ru`].
    pub fn from_str_relaxed(s: &str) -> Self {
        let s = s.to_lowercase();
        if s.starts_with("en") {
            Self::En
        } else {
            Self::Ru
        }
    }
}

/// Потокобезопасный реестр словарей переводов платформы
#[derive(Debug, Clone)]
pub struct I18nRegistry {
    /// Хранилище: Locale -> (Key -> Translation Template)
    dictionaries: Arc<RwLock<HashMap<Locale, HashMap<String, String>>>>,
}

impl Default for I18nRegistry {
    fn default() -> Self {
        let registry = Self {
            dictionaries: Arc::new(RwLock::new(HashMap::new())),
        };
        registry.load_builtin_translations();
        registry
    }
}

impl I18nRegistry {
    /// Создать новый экземпляр реестра с предзагруженными встроенными системными переводами ядра
    pub fn new() -> Self {
        Self::default()
    }

    /// Загрузить встроенные системные словари ядра из скомпилированных JSON файлов `locales/`
    fn load_builtin_translations(&self) {
        let _ = self.register_json(Locale::Ru, None, BUILTIN_RU_JSON);
        let _ = self.register_json(Locale::En, None, BUILTIN_EN_JSON);
    }

    /// Зарегистрировать внешний JSON-словарь (например, из архива установленного плагина)
    /// с опциональным префиксом модуля (`"{prefix}.{key}"`)
    ///
    /// # Аргументы
    /// * `locale` — Целевой язык ([`Locale`]).
    /// * `prefix` — Опциональный префикс пространства имен плагина (например, `"snmp"`).
    /// * `json_content` — Содержимое JSON словаря перевода в виде ключ-значение.
    ///
    /// # Возвращаемое значение
    /// Количество успешно зарегистрированных пар ключ-значение.
    ///
    /// # Ошибки
    /// Возвращает [`serde_json::Error`], если JSON синтаксически некорректен.
    pub fn register_json(
        &self,
        locale: Locale,
        prefix: Option<&str>,
        json_content: &str,
    ) -> Result<usize, serde_json::Error> {
        let map: HashMap<String, String> = serde_json::from_str(json_content)?;
        let mut dicts = self.dictionaries.write().expect("Lock poisoned");
        let locale_dict = dicts.entry(locale).or_default();

        let count = map.len();
        for (key, val) in map {
            let full_key = match prefix {
                Some(p) if !p.is_empty() => format!("{}.{}", p, key),
                _ => key,
            };
            locale_dict.insert(full_key, val);
        }

        Ok(count)
    }

    /// Выполнить перевод ключа для указанной локали с интерполяцией именованных параметров `{param}`
    ///
    /// При отсутствии ключа в указанной локали автоматически выполняется fallback в русский язык (`Locale::Ru`).
    /// Если перевод не найден ни в одной локали, возвращается форматированная строка вида `key[param1=val1]`.
    ///
    /// # Аргументы
    /// * `locale` — Запрошенная локаль ([`Locale`]).
    /// * `key` — Ключ перевода (например, `"core.error.unauthorized"`).
    /// * `params` — Срез кортежей `(имя_параметра, значение)` для шаблонной подстановки.
    ///
    /// # Возвращаемое значение
    /// Результирующий локализованный текст.
    pub fn translate(&self, locale: Locale, key: &str, params: &[(&str, &str)]) -> String {
        let dicts = self.dictionaries.read().expect("Lock poisoned");

        // Поиск перевода в запрошенной локали, затем fallback в русский
        let template_opt = dicts
            .get(&locale)
            .and_then(|m| m.get(key))
            .or_else(|| dicts.get(&Locale::Ru).and_then(|m| m.get(key)));

        match template_opt {
            Some(template) => interpolate(template, params),
            None => {
                // Если ключ не найден, возвращаем сам ключ с отформатированными параметрами
                if params.is_empty() {
                    key.to_string()
                } else {
                    let formatted_params: Vec<String> =
                        params.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
                    format!("{}[{}]", key, formatted_params.join(", "))
                }
            }
        }
    }

    /// Экспортировать полную карту переводов для указанной локали (для передачи на фронтенд / REST API)
    ///
    /// # Аргументы
    /// * `locale` — Запрашиваемый язык ([`Locale`]).
    ///
    /// # Возвращаемое значение
    /// Словарь `HashMap<String, String>` всех зарегистрированных ключей и шаблонов.
    pub fn export_locale(&self, locale: Locale) -> HashMap<String, String> {
        let dicts = self.dictionaries.read().expect("Lock poisoned");
        dicts.get(&locale).cloned().unwrap_or_default()
    }
}

/// Подстановка именованных аргументов `{name}` в шаблон
fn interpolate(template: &str, params: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (key, val) in params {
        let pattern = format!("{{{}}}", key);
        result = result.replace(&pattern, val);
    }
    result
}

/// Глобальный инстанс реестра локализации
static GLOBAL_REGISTRY: OnceLock<I18nRegistry> = OnceLock::new();

/// Получить ссылку на глобальный синглтон реестра переводов платформы [`I18nRegistry`]
pub fn global() -> &'static I18nRegistry {
    GLOBAL_REGISTRY.get_or_init(I18nRegistry::new)
}

/// Удобная глобальная функция для выполнения перевода строки
///
/// # Примеры
/// ```rust
/// use nms_common::i18n::{tr, Locale};
///
/// let msg = tr(Locale::Ru, "core.error.unauthorized", &[("details", "Неверный пароль")]);
/// ```
pub fn tr(locale: Locale, key: &str, params: &[(&str, &str)]) -> String {
    global().translate(locale, key, params)
}
