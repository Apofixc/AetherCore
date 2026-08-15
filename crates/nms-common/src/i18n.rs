//! # Подсистема интернационализации и локализации (i18n)
//!
//! Обеспечивает сквозную локализацию сообщений об ошибках, системных событий,
//! аудит-логов и интерфейсов на русском и английском языках.
//! Поддерживает встроенные системные словари и динамическую регистрацию
//! словарей плагинов (модулей) с префиксами `module_id.key`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// Поддерживаемые языковые локали платформы
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    /// Русский язык (по умолчанию)
    #[default]
    Ru,
    /// Английский язык
    En,
}

impl Locale {
    /// Получить строковый код локали
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }

    /// Парсинг локали из заголовков (например, Accept-Language) или строки
    pub fn from_str_relaxed(s: &str) -> Self {
        let s = s.to_lowercase();
        if s.starts_with("en") {
            Self::En
        } else {
            Self::Ru
        }
    }
}

/// Потокобезопасный реестр словарей переводов
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
    /// Создать новый реестр с предзагруженными системными переводами
    pub fn new() -> Self {
        Self::default()
    }

    /// Загрузить встроенные системные словари ядра
    fn load_builtin_translations(&self) {
        let mut dicts = self.dictionaries.write().expect("Lock poisoned");

        // Словарь для русского языка
        let ru_map = dicts.entry(Locale::Ru).or_default();
        ru_map.insert("core.ok".into(), "Успешно".into());
        ru_map.insert("core.error.internal".into(), "Внутренняя ошибка сервера: {details}".into());
        ru_map.insert("core.error.not_found".into(), "Запрошенный ресурс '{resource}' не найден".into());
        ru_map.insert("core.error.unauthorized".into(), "Требуется авторизация".into());
        ru_map.insert("core.error.forbidden".into(), "Недостаточно прав доступа для выполнения операции: {permission}".into());
        ru_map.insert("core.error.bad_request".into(), "Некорректные параметры запроса: {details}".into());
        ru_map.insert("core.error.conflict".into(), "Конфликт данных: {details}".into());
        ru_map.insert("core.error.database".into(), "Ошибка базы данных: {details}".into());
        ru_map.insert("core.error.validation".into(), "Ошибка валидации поля '{field}': {details}".into());
        ru_map.insert("core.error.plugin_failed".into(), "Сбой плагина '{plugin_id}': {details}".into());
        ru_map.insert("core.error.plugin_timeout".into(), "Превышено время ожидания выполнения плагина '{plugin_id}'".into());
        ru_map.insert("core.error.rate_limited".into(), "Превышен лимит запросов. Повторите попытку через {retry_after} сек.".into());

        // Словарь для английского языка
        let en_map = dicts.entry(Locale::En).or_default();
        en_map.insert("core.ok".into(), "Success".into());
        en_map.insert("core.error.internal".into(), "Internal server error: {details}".into());
        en_map.insert("core.error.not_found".into(), "Requested resource '{resource}' was not found".into());
        en_map.insert("core.error.unauthorized".into(), "Authentication required".into());
        en_map.insert("core.error.forbidden".into(), "Forbidden: missing required permission: {permission}".into());
        en_map.insert("core.error.bad_request".into(), "Bad request: {details}".into());
        en_map.insert("core.error.conflict".into(), "Conflict: {details}".into());
        en_map.insert("core.error.database".into(), "Database error: {details}".into());
        en_map.insert("core.error.validation".into(), "Validation error for field '{field}': {details}".into());
        en_map.insert("core.error.plugin_failed".into(), "Plugin '{plugin_id}' failed: {details}".into());
        en_map.insert("core.error.plugin_timeout".into(), "Plugin '{plugin_id}' execution timed out".into());
        en_map.insert("core.error.rate_limited".into(), "Too many requests. Please retry after {retry_after}s".into());
    }

    /// Зарегистрировать внешний JSON-словарь (например, из архива плагина)
    /// с опциональным префиксом модуля
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

    /// Выполнить перевод ключа для указанной локали с интерполяцией параметров
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

    /// Экспортировать все ключи для указанной локали (для передачи на фронтенд)
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

/// Получить глобальный реестр переводов
pub fn global() -> &'static I18nRegistry {
    GLOBAL_REGISTRY.get_or_init(I18nRegistry::new)
}

/// Удобная глобальная функция перевода
pub fn tr(locale: Locale, key: &str, params: &[(&str, &str)]) -> String {
    global().translate(locale, key, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_translations() {
        let registry = I18nRegistry::new();

        let ru = registry.translate(Locale::Ru, "core.ok", &[]);
        assert_eq!(ru, "Успешно");

        let en = registry.translate(Locale::En, "core.ok", &[]);
        assert_eq!(en, "Success");
    }

    #[test]
    fn test_interpolation() {
        let registry = I18nRegistry::new();

        let err_ru = registry.translate(
            Locale::Ru,
            "core.error.not_found",
            &[("resource", "users/42")],
        );
        assert_eq!(err_ru, "Запрошенный ресурс 'users/42' не найден");

        let err_en = registry.translate(
            Locale::En,
            "core.error.not_found",
            &[("resource", "users/42")],
        );
        assert_eq!(err_en, "Requested resource 'users/42' was not found");
    }

    #[test]
    fn test_register_plugin_json() {
        let registry = I18nRegistry::new();
        let plugin_ru_json = r#"{"widget.title": "Статус сети", "status.online": "В сети"}"#;

        let registered = registry
            .register_json(Locale::Ru, Some("my_plugin"), plugin_ru_json)
            .expect("Failed to register json");
        assert_eq!(registered, 2);

        let translated = registry.translate(Locale::Ru, "my_plugin.widget.title", &[]);
        assert_eq!(translated, "Статус сети");
    }

    #[test]
    fn test_locale_parsing() {
        assert_eq!(Locale::from_str_relaxed("en-US,en;q=0.9"), Locale::En);
        assert_eq!(Locale::from_str_relaxed("ru-RU,ru;q=0.8"), Locale::Ru);
        assert_eq!(Locale::from_str_relaxed(""), Locale::Ru);
    }
}
