# 💾 WIT Интерфейс `nms:core/storage` — Изолированное KV-Хранилище

## 1. Назначение и модель изоляции данных

Интерфейс `nms:core/storage` предоставляет плагинам безопасное персистентное Key-Value хранилище, интегрированное в транзакционную базу данных SQLite микроядра.

### Архитектура пространств имен и изоляции (Namespace Sandboxing):
- Каждый плагин оперирует внутри собственного изолированного пространства имен `module:{plugin_id}:{key}`.
- Гостевой код передает относительный ключ (например, `"config"` или `"device_cache"`).
- Хост-трамплин автоматически конструирует уникальный составной первичный ключ в таблице `kv_store`:
  $$\text{Primary Key} = (\text{namespace}=\text{"module:"} + \text{plugin\_id},\; \text{key}=\text{user\_key})$$
- **Безопасность**: Плагин не имеет физической возможности прочитать, изменить или удалить данные чужого модуля или системные настройки ядра (`namespace="system"`).

---

## 2. Полный код WIT спецификации (`nms-core.wit`)

```wit
package nms:core@2.0.0;

interface storage {
    /// Получить значение по ключу из изолированного хранилища плагина
    ///
    /// # Параметры:
    /// - `key`: строковый идентификатор ключа
    ///
    /// # Возвращаемое значение:
    /// - `ok(some(string))`: значение найдено (JSON-строка)
    /// - `ok(none)`: ключ отсутствует в хранилище
    /// - `err(string)`: ошибка чтения из базы данных
    get: func(key: string) -> result<option<string>, string>;

    /// Сохранить значение по ключу (Операция UPSERT)
    ///
    /// Если ключ уже существует, его значение и дата `updated_at` обновляются.
    /// Если ключ отсутствует, создается новая запись.
    ///
    /// # Параметры:
    /// - `key`: строковый идентификатор ключа
    /// - `value-json`: сохраняемые данные в формате валидной JSON-строки
    set: func(key: string, value-json: string) -> result<_, string>;

    /// Удалить ключ из хранилища
    ///
    /// # Возвращаемое значение:
    /// - `ok(true)`: запись успешно удалена
    /// - `ok(false)`: ключ не существовал
    /// - `err(string)`: ошибка выполнения SQL-запроса
    delete: func(key: string) -> result<bool, string>;
}
```

---

## 3. Схема хранения данных в SQLite WAL

Таблица `kv_store` в базе данных платформы:

```sql
CREATE TABLE IF NOT EXISTS kv_store (
    namespace TEXT NOT NULL,       -- "module:ping-collector"
    key TEXT NOT NULL,             -- "scan_state"
    value_json TEXT NOT NULL,      -- '{"last_ip": "10.0.0.45", "scanned_total": 120}'
    updated_at TEXT NOT NULL,      -- "2026-08-15T18:00:00.000Z"
    PRIMARY KEY (namespace, key)
);
CREATE INDEX IF NOT EXISTS idx_kv_namespace ON kv_store(namespace);
```

---

## 4. Полноценный практический пример на Rust (WASM Guest)

Пример сохранения и восстановления структурированного состояния сканера с использованием `serde` и строгой типизацией:

```rust
// src/lib.rs
#[allow(warnings)]
mod bindings;

use bindings::nms::core::logger::{self, LogLevel};
use bindings::nms::core::storage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct PollerState {
    pub last_polled_timestamp: u64,
    pub active_hosts: Vec<String>,
    pub consecutive_failures: u32,
}

pub struct StateManager;

impl StateManager {
    const STATE_KEY: &'static str = "poller_runtime_state";

    /// Загрузить сохраненное состояние из KV-хранилища ядра
    pub fn load() -> PollerState {
        match storage::get(Self::STATE_KEY) {
            Ok(Some(json_str)) => {
                serde_json::from_str(&json_str).unwrap_or_else(|e| {
                    logger::log(
                        LogLevel::Warn,
                        &format!("Corrupted state JSON, resetting to default: {}", e),
                    );
                    PollerState::default()
                })
            }
            Ok(None) => {
                logger::log(LogLevel::Info, "No previous state found, initializing fresh state");
                PollerState::default()
            }
            Err(e) => {
                logger::log(LogLevel::Error, &format!("Storage read failure: {}", e));
                PollerState::default()
            }
        }
    }

    /// Атомарно сохранить состояние в KV-хранилище
    pub fn save(state: &PollerState) -> Result<(), String> {
        let json_str = serde_json::to_string(state)
            .map_err(|e| format!("Failed to serialize state: {}", e))?;

        storage::set(Self::STATE_KEY, &json_str)?;
        Ok(())
    }

    /// Сбросить состояние плагина
    pub fn clear() -> Result<bool, String> {
        storage::delete(Self::STATE_KEY)
    }
}
```

---

## 5. Рекомендации по оптимизации и лимиты размера значений

1. **Размер значений**: KV-хранилище оптимизировано для хранения конфигураций, структур состояния и кэшей размером от единиц байт до 5–10 MB на запись.
2. **Транзакционность**: Каждая операция `set` выполняется через оптимизированный `UPSERT` запрос в Single-Writer пуле ядра.
3. **Кэширование на стороне гостевого модуля**: Для высокочастотных операций в цикле (более 1000 запросов в секунду) рекомендуется кэшировать состояние в локальной оперативной памяти плагина и сбрасывать в `storage::set` периодически (по тикам таймера).
