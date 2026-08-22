# ⚡ WIT Интерфейс `aether:core/events` — Гибридная Шина Сообщений

## 1. Назначение и архитектура шины событий

Интерфейс `aether:core/events` является фундаментальной транспортной магистралью микроядра платформы. Он обеспечивает асинхронную передачу сообщений между изолированными WASM-модулями, ядром и клиентским веб-интерфейсом через два независимых контура доставки:

```text
                                 ┌───────────────────────────────┐
                                 │     WASM Guest Component      │
                                 └───────────────┬───────────────┘
                                                 │
                     ┌───────────────────────────┴───────────────────────────┐
                     │                                                       │
                     ▼ (publish-telemetry)                                   ▼ (publish-reliable)
       ┌───────────────────────────┐                           ┌───────────────────────────┐
       │   Live Telemetry Bus      │                           │  Reliable Event Journal   │
       │   (tokio::broadcast)      │                           │  (tokio::mpsc + SQLite)   │
       └─────────────┬─────────────┘                           └─────────────┬─────────────┘
                     │                                                       │
         ┌───────────┴───────────┐                               ┌───────────┴───────────┐
         │ • WebSocket Clients   │                               │ • Persistence to Disk │
         │ • Real-time Dashboards│                               │ • Audit & Replay      │
         │ • Drop on slow client │                               │ • Zero Message Loss   │
         └───────────────────────┘                               └───────────────────────┘
```

---

## 2. Полный код WIT спецификации (`aethercore-core.wit`)

```wit
package aether:core@2.0.0;

interface events {
    /// Публикация высокочастотного телеметрического события (In-Memory Broadcast)
    /// 
    /// Используется для оперативных метрик (загрузка CPU, пинг, счетчики пакетов).
    /// Сообщения не сохраняются на диск. При переполнении буфера медленных клиентов
    /// устаревшие события автоматически перезаписываются.
    ///
    /// # Параметры:
    /// - `topic`: топик события (обязан начинаться с `{plugin_id}.`)
    /// - `payload-json`: сериализованные данные в формате JSON
    publish-telemetry: func(topic: string, payload-json: string) -> result<_, string>;

    /// Публикация надежного системного события (Persistent SQLite WAL Journal)
    ///
    /// Используется для критических изменений конфигурации, фиксации аварий и аудита.
    /// Сообщения гарантированно сохраняются в базу данных SQLite с присвоением ID и UUID.
    ///
    /// # Параметры:
    /// - `topic`: топик события (обязан начинаться с `{plugin_id}.`)
    /// - `payload-json`: сериализованные данные в формате JSON
    publish-reliable: func(topic: string, payload-json: string) -> result<_, string>;
}
```

---

## 3. Спецификация топиков и правила безопасности (Anti-Spoofing Rules)

Каждое событие шины типизируется строковым топиком (`topic`). В ядре платформы действует **строгое правило изоляции пространств имен**:

1. **Правило префикса плагина**: Модуль с идентификатором `my-sensor` имеет право публиковать события **исключительно** в топики вида:
   - `my-sensor.metric_tick`
   - `my-sensor.device_discovered`
   - `my-sensor.status.online`
2. **Блокировка попыток подделки (Spoofing)**: Если модуль попытается вызвать `publish_reliable("core.user_created", ...)` или `publish_telemetry("other-plugin.metric", ...)`, хост-трамплин Wasmtime немедленно прервет операцию и вернет ошибку:
   ```text
   Err("Forbidden: Module 'my-sensor' cannot publish to topic 'core.user_created'. Topic must start with 'my-sensor.'")
   ```
3. **Декларация в манифесте**: Все публикуемые топики должны быть заранее задекларированы в секции `events.publishes` файла `manifest.yaml`.

---

## 4. Полноценный практический пример на Rust (WASM Guest)

```rust
// src/lib.rs
#[allow(warnings)]
mod bindings;

use bindings::aether::core::events;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct NetworkMetricTick {
    pub interface: String,
    pub rx_bytes_sec: u64,
    pub tx_bytes_sec: u64,
    pub drops: u32,
}

pub fn emit_interface_metrics(iface: &str, rx: u64, tx: u64, drops: u32) -> Result<(), String> {
    let tick = NetworkMetricTick {
        interface: iface.to_string(),
        rx_bytes_sec: rx,
        tx_bytes_sec: tx,
        drops,
    };

    let payload_str = serde_json::to_string(&tick)
        .map_err(|e| format!("Serialization error: {}", e))?;

    // Отправка в высокоскоростной In-Memory Live Broadcast
    events::publish_telemetry("my-sensor.metric_tick", &payload_str)?;

    // Если зафиксированы потери — отправляем в персистентный журнал
    if drops > 0 {
        let alert_payload = serde_json::json!({
            "interface": iface,
            "dropped_packets": drops,
            "severity": "warning"
        });
        events::publish_reliable("my-sensor.drops_detected", &alert_payload.to_string())?;
    }

    Ok(())
}
```

---

## 5. Подписка на события шины (Guest Event Consumer)

Чтобы плагин мог получать сообщения шины, он экспортирует гостевой интерфейс `event-consumer`:

```wit
interface event-consumer {
    /// Обработчик входящего события шины
    handle-event: func(topic: string, source: string, payload-json: string) -> result<_, string>;
}
```

Ядро автоматически подписывает плагин на топики, перечисленные в секции `events.subscribes` манифеста, и вызывает `handle-event` в выделенном Tokio Mailbox актора плагина.
