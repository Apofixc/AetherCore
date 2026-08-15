# ⚡ WIT Интерфейс `nms:core/events`

## 1. Назначение и роль в ядре

Интерфейс `nms:core/events` предоставляет плагину доступ к гибридной шине сообщений микроядра:
- **Telemetry**: высокоскоростной Live Broadcast поток для передачи метрик на графики в реальном времени.
- **Reliable**: персистентный журнал событий с гарантированной записью в SQLite WAL.

---

## 2. Полный код WIT спецификации

```wit
interface events {
    /// Публикация высокочастотного телеметрического события (In-Memory Broadcast)
    publish-telemetry: func(topic: string, payload-json: string) -> result<_, string>;

    /// Публикация надежного события (Persistent SQLite Journal)
    publish-reliable: func(topic: string, payload-json: string) -> result<_, string>;
}
```

---

## 3. Описание структур данных и правил безопасности

- `topic`: строка формата `{plugin_id}.{event_name}`. Попытка опубликовать в чужой топик приведет к ошибке `INSUFFICIENT_PERMISSIONS` (защита от спуфинга).
- `payload-json`: валидная строка в формате JSON с полезной нагрузкой события.

---

## 4. Пример использования на Rust (Гостевой код плагина)

```rust
use crate::bindings::nms::core::events;

pub fn emit_status_update(status: &str, latency_ms: f64) {
    let payload = serde_json::json!({
        "status": status,
        "latency_ms": latency_ms,
    });

    // Отправка телеметрии
    let _ = events::publish_telemetry("my-plugin.status_updated", &payload.to_string());
}
```
