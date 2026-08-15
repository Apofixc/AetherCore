# Модуль асинхронной шины событий, журналов и WebSockets (Event Bus & WebSockets)

Документ описывает подсистему сообщений Pub/Sub `EventBus`, пакетного журналирования SQLite, сопоставления топиков и трансляции событий по WebSocket с фильтрацией в `nms-core`.

---

## 🏛️ Компоненты шины событий

### 1. Сопоставление топиков и In-Process Callbacks (`bus.rs`)
- Метод `match_topic(pattern, topic)` поддерживает сопоставление по маскам:
  - `*` / `#`: Совпадает с любым топиком.
  - `+` / `*` в середине (`device.+.down`): Совпадает с 1 позиционным сегментом.
  - `#` в конце (`core.#`): Совпадает со всеми хвостовыми подтопиками.
- **Защита системных топиков**: Публикация в зарезервированные топики вида `core.*` разрешена только ядру системного бэкенда (`is_core = true`).
- **In-Process Callback-подписчики**: `subscribe_callback(pattern, cb)` с изоляцией падений вызовов, отпиской `unsubscribe_callback` и получением статистики `get_stats()`.

### 2. Журналирование и повтор пропущенных событий (`db.rs`)
- **`EventJournalQueue`**: Асинхронная очередь пакетной записи событий в таблицу SQLite `system_events_journal`.
- **`get_missed_events_from_db`**: Функция выборки пропущенных событий по `seq_id` при переподключении клиента.

### 3. Потоковый WebSocket эндпоинт (`server.rs`)
- Эндпоинт `/api/v1/ws/events` с проверкой аутентификации пользователя (JWT / ticket) и лимитом `MAX_CONNECTIONS_PER_USER = 10` (`ConnectionManager`).
- **Команды сокета**:
  - `{"action": "subscribe", "topics": ["device.*"]}`: Динамическая подписка на топики.
  - `{"action": "unsubscribe", "topics": ["device.*"]}`: Отписка от топиков.
  - `{"action": "replay", "from_seq_id": N}`: Запрос повтора пропущенных событий из журнала БД.
- **Адресная доставка**: Фильтрация событий по `target_user_id` и подпискам клиентов.

---

## 💡 Пример использования в Rust

```rust
use std::sync::Arc;
use nms_core::{EventBus, SystemEvent, match_topic};

fn main() -> anyhow::Result<()> {
    let bus = EventBus::new(1024);

    // 1. Регистрация In-Process callback
    bus.subscribe_callback("device.#", Arc::new(|event| {
        println!("Received event: {} -> {:?}", event.topic, event.payload);
    }));

    // 2. Публикация адресованного события в шину
    let event = SystemEvent::new("device.ping.down", serde_json::json!({ "ip": "10.0.0.1" }), "ping_collector")
        .with_target_user("usr-admin-01");

    bus.publish(event, false)?;
    Ok(())
}
```
