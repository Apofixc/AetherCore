# 📝 WIT Интерфейс `nms:core/logger`

## 1. Назначение и роль в ядре

Интерфейс `nms:core/logger` позволяет гостевому коду плагина направлять структурированные логи в централизованную систему трассировки ядра с автоматическим тегированием `plugin_id`.

---

## 2. Полный код WIT спецификации

```wit
interface logger {
    enum log-level {
        trace,
        debug,
        info,
        warn,
        error,
    }

    /// Записать сообщение в централизованный лог ядра
    log: func(level: log-level, message: string);
}
```

---

## 3. Пример использования на Rust (Гостевой код плагина)

```rust
use crate::bindings::nms::core::logger::{self, LogLevel};

pub fn process_data(device_id: &str) {
    logger::log(LogLevel::Info, &format!("Processing data for device: {}", device_id));
}
```
