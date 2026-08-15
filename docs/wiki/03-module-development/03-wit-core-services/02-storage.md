# 💾 WIT Интерфейс `nms:core/storage`

## 1. Назначение и роль в ядре

Интерфейс `nms:core/storage` предоставляет изолированное Key-Value хранилище. Каждый плагин автоматически изолирован в собственном пространстве имен `module:{plugin_id}:{key}`.

---

## 2. Полный код WIT спецификации

```wit
interface storage {
    /// Получить JSON-значение по ключу
    get: func(key: string) -> result<option<string>, string>;

    /// Сохранить JSON-значение по ключу (UPSERT)
    set: func(key: string, value-json: string) -> result<_, string>;

    /// Удалить ключ
    delete: func(key: string) -> result<bool, string>;
}
```

---

## 3. Пример использования на Rust (Гостевой код плагина)

```rust
use crate::bindings::nms::core::storage;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct DeviceState {
    last_seen: u64,
    retry_count: u32,
}

pub fn save_state(state: &DeviceState) {
    let json_str = serde_json::to_string(state).unwrap();
    let _ = storage::set("device_state", &json_str);
}

pub fn load_state() -> Option<DeviceState> {
    if let Ok(Some(json_str)) = storage::get("device_state") {
        serde_json::from_str(&json_str).ok()
    } else {
        None
    }
}
```
