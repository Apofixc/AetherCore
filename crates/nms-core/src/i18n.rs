// Утилиты и реестр локализации бэкенда (i18n)
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// Извлекает двуквенный код языка из параметров запроса или заголовка Accept-Language.
/// По умолчанию возвращает "en".
pub fn get_lang(query_lang: Option<&str>, accept_header: Option<&str>) -> String {
    if let Some(q) = query_lang {
        let q_clean = q.trim().to_lowercase();
        if q_clean == "ru" || q_clean == "en" {
            return q_clean;
        }
    }
    if let Some(accept) = accept_header {
        let accept_clean = accept.to_lowercase();
        if accept_clean.contains("ru") {
            return "ru".to_string();
        }
    }
    "en".to_string()
}

/// Потокобезопасный реестр переводов для бэкенда NMS
#[derive(Clone, Debug)]
pub struct I18nEngine {
    /// Структура хранения: key -> (lang -> template)
    messages: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

impl Default for I18nEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl I18nEngine {
    /// Создает новый экземпляр реестра и инициализирует сообщения из модуля locales (1-в-1 с Python BACKEND_MESSAGES)
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(crate::locales::_build_messages())),
        }
    }

    /// Локализует строку по ключу с опциональной подстановкой именованных параметров
    pub fn tr(
        &self,
        lang: &str,
        key_or_ru: &str,
        fallback_en: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> String {
        let lang_code = if lang.to_lowercase().starts_with("ru") {
            "ru"
        } else {
            "en"
        };

        let template = {
            let guard = self.messages.read().unwrap_or_else(|e| e.into_inner());
            if let Some(lang_map) = guard.get(key_or_ru) {
                lang_map
                    .get(lang_code)
                    .cloned()
                    .or_else(|| lang_map.get("en").cloned())
                    .unwrap_or_else(|| key_or_ru.to_string())
            } else if let Some(en_val) = fallback_en {
                if lang_code == "ru" {
                    key_or_ru.to_string()
                } else {
                    en_val.to_string()
                }
            } else {
                key_or_ru.to_string()
            }
        };

        Self::format_template(&template, params)
    }

    /// Зарегистрировать или обновить переводы сообщений для модуля (1-в-1 с Python register_module_messages)
    pub fn register_module_messages(&self, messages: HashMap<String, HashMap<String, String>>) {
        let mut guard = self.messages.write().unwrap_or_else(|e| e.into_inner());
        for (key, lang_map) in messages {
            let entry = guard.entry(key).or_default();
            for (lang, template) in lang_map {
                entry.insert(lang.to_lowercase(), template);
            }
        }
    }

    /// Загружает файлы локализации (JSON, YAML, TOML) из директории `locales/` указанного модуля
    pub fn load_module_locales(&self, module_dir: impl AsRef<Path>) -> Result<usize, String> {
        let locales_dir = module_dir.as_ref().join("locales");
        if !locales_dir.is_dir() {
            return Ok(0);
        }

        let entries = fs::read_dir(&locales_dir)
            .map_err(|e| format!("Failed to read locales directory: {e}"))?;

        let mut loaded_count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if !matches!(ext.as_str(), "json" | "yaml" | "yml" | "toml") {
                continue;
            }

            let Some(lang) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };

            let parsed_map: Option<HashMap<String, String>> = match ext.as_str() {
                "json" => serde_json::from_str(&content).ok(),
                "yaml" | "yml" => serde_yaml::from_str(&content).ok(),
                "toml" => toml::from_str(&content).ok(),
                _ => None,
            };

            if let Some(map) = parsed_map {
                let mut batch = HashMap::new();
                for (key, val) in map {
                    let mut lang_map = HashMap::new();
                    lang_map.insert(lang.to_lowercase(), val);
                    batch.insert(key, lang_map);
                }
                self.register_module_messages(batch);
                loaded_count += 1;
            }
        }
        Ok(loaded_count)
    }

    /// Вспомогательная функция подстановки именованных параметров {name} в шаблон
    fn format_template(template: &str, params: Option<&[(&str, &str)]>) -> String {
        let Some(param_list) = params else {
            return template.to_string();
        };

        let mut result = template.to_string();
        for (key, val) in param_list {
            let placeholder = format!("{{{key}}}");
            result = result.replace(&placeholder, val);
        }
        result
    }
}
