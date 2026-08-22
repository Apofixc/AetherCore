# 🧩 Спецификация WIT интерфейсов ядра (`aether:core@2.0.0`)

Спецификация интерфейсов плагинов описана на языке **WebAssembly Interface Types (WIT)** в файле [`wit/aethercore-core.wit`](file:///opt/AetherCore/wit/aethercore-core.wit).

---

## 1. Host Interfaces (Импортируются плагином из ядра)

Плагин взаимодействует с системными сервисами ядра через стандартизированные WIT интерфейсы:

- **`aether:core/events`**:
  - `publish-telemetry(topic: string, payload-json: string) -> result<_, string>`: отправка высокочастотных метрик в Live Broadcast канал.
  - `publish-reliable(topic: string, payload-json: string) -> result<_, string>`: гарантированная отправка системных событий в персистентный журнал SQLite.
- **`aether:core/storage`**:
  - `get(key: string) -> result<option<string>, string>`: чтение значения из изолированного KV-хранилища плагина.
  - `set(key: string, value-json: string) -> result<_, string>`: сохранение значения.
  - `delete(key: string) -> result<bool, string>`: удаление ключа.
- **`aether:core/logger`**:
  - `log(level: log-level, message: string)`: вывод структурированного лога в систему трассировки ядра с тегом модуля.
- **`aether:core/notify`**:
  - `send-alert(severity: alert-severity, title: string, message: string)`: отправка оповещения администраторам (Email, Telegram, Webhook).
- **`aether:core/i18n`**:
  - `translate(key: string, params: list<tuple<string, string>>) -> string`: перевод строки по зарегистрированным словарям.
- **`aether:core/rpc`**:
  - `call(target-module: string, method: string, params-json: string) -> result<string, string>`: синхронный межизоляционный вызов между плагинами.

---

## 2. Guest Interfaces (Экспортируются плагином в ядро)

Ядро вызывает функции плагина через следующие точки входа:

- **`aether:core/lifecycle`**:
  - `init() -> result<_, string>`: первоначальная инициализация при старте модуля.
  - `on-enable() -> result<_, string>`: вызов при активации модуля.
  - `on-disable() -> result<_, string>`: вызов при деактивации модуля (остановка таймеров, освобождение ресурсов).
- **`aether:core/event-consumer`**:
  - `handle-event(topic: string, source: string, payload-json: string) -> result<_, string>`: обработка событий шины, на которые подписан модуль.
- **`aether:core/timer-consumer`**:
  - `tick(timer-id: string) -> result<_, string>`: периодический тик планировщика ядра.
- **`aether:core/rpc-handler`**:
  - `handle-rpc(caller: string, method: string, params-json: string) -> result<string, string>`: обработка входящих межмодульных RPC вызовов.
