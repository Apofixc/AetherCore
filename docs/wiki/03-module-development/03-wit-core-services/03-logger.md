# 📝 WIT Интерфейс `nms:core/logger` — Структурированное Логирование

## 1. Назначение и интеграция с системой трассировки `tracing`

Интерфейс `nms:core/logger` связывает гостевую песочницу WebAssembly с централизованной системой трассировки и логирования микроядра (`tracing` / `tracing-subscriber`).

### Преимущества единого логирования ядра:
- **Автоматический контекст**: Ядро автоматически обогащает каждую строку лога метаданными: `plugin_id`, точным таймстампом с микросекундами (UTC), потоком Tokio Worker и уровнем детализации.
- **Единая фильтрация**: Уровни логов гостевых модулей мгновенно реагируют на системную переменную окружения `RUST_LOG` (например, `RUST_LOG="nms_core=info,my_plugin=debug"`).
- **Ротация и экспорт**: Логи модулей автоматически попадают в системный лог-файл и журнал аудита без необходимости ручной ротации файлов внутри плагина.

---

## 2. Полный код WIT спецификации (`nms-core.wit`)

```wit
package nms:core@2.0.0;

interface logger {
    /// Уровни детализации логов
    enum log-level {
        /// Подробная пошаговая трассировка (циклы, дампы пакетов)
        trace,
        /// Отладочная информация для разработчиков
        debug,
        /// Стандартные информационные сообщения жизненного цикла
        info,
        /// Предупреждения о нештатных, но обработанных ситуациях
        warn,
        /// Ошибки, приводящие к прерыванию конкретной операции
        error,
    }

    /// Записать структурированное сообщение в централизованный лог ядра
    ///
    /// # Параметры:
    /// - `level`: уровень детализации сообщения
    /// - `message`: текст сообщения
    log: func(level: log-level, message: string);
}
```

---

## 3. Практический пример на Rust (WASM Guest) с макросами

Для максимального удобства в гостевом Rust-коде рекомендуется создать тонкие обертки (макросы), привычные любому Rust-разработчику:

```rust
// src/logger_compat.rs
#[allow(warnings)]
mod bindings;

use bindings::nms::core::logger::{self, LogLevel};

#[macro_export]
macro_rules! plugin_info {
    ($($arg:tt)*) => {
        $crate::bindings::nms::core::logger::log(
            $crate::bindings::nms::core::logger::LogLevel::Info,
            &format!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! plugin_warn {
    ($($arg:tt)*) => {
        $crate::bindings::nms::core::logger::log(
            $crate::bindings::nms::core::logger::LogLevel::Warn,
            &format!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! plugin_error {
    ($($arg:tt)*) => {
        $crate::bindings::nms::core::logger::log(
            $crate::bindings::nms::core::logger::LogLevel::Error,
            &format!($($arg)*),
        )
    };
}

#[macro_export]
macro_rules! plugin_debug {
    ($($arg:tt)*) => {
        $crate::bindings::nms::core::logger::log(
            $crate::bindings::nms::core::logger::LogLevel::Debug,
            &format!($($arg)*),
        )
    };
}

// Пример использования в бизнес-логике:
pub fn handle_packet(bytes_count: usize, peer_ip: &str) {
    plugin_debug!("Received packet from {} (size: {} bytes)", peer_ip, bytes_count);

    if bytes_count == 0 {
        plugin_warn!("Empty payload received from peer {}", peer_ip);
    } else {
        plugin_info!("Packet successfully processed for {}", peer_ip);
    }
}
```

---

## 4. Пример вывода в консоли сервера

```text
2026-08-15T18:30:15.123456Z  INFO nms_core::plugins::host: [plugin:my-sensor] Starting interface poller worker
2026-08-15T18:30:15.124512Z DEBUG nms_core::plugins::host: [plugin:my-sensor] Received packet from 10.0.0.1 (size: 64 bytes)
2026-08-15T18:30:15.125100Z  WARN nms_core::plugins::host: [plugin:my-sensor] Latency exceeded threshold: 124.5 ms
```
