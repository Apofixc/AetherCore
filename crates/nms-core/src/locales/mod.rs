// Сборка сообщений локализации бэкенда из встроенных JSON-файлов локализации (1-в-1 с Python backend/core/locales)
use std::collections::HashMap;

/// Формирует словарь сообщений бэкенда из встроенных файлов src/locales/ru.json и src/locales/en.json
pub fn _build_messages() -> HashMap<String, HashMap<String, String>> {
    let mut result: HashMap<String, HashMap<String, String>> = HashMap::new();

    if let Ok(ru_map) = serde_json::from_str::<HashMap<String, String>>(include_str!("ru.json")) {
        for (key, val) in ru_map {
            result.entry(key).or_default().insert("ru".to_string(), val);
        }
    }

    if let Ok(en_map) = serde_json::from_str::<HashMap<String, String>>(include_str!("en.json")) {
        for (key, val) in en_map {
            result.entry(key).or_default().insert("en".to_string(), val);
        }
    }

    result
}
