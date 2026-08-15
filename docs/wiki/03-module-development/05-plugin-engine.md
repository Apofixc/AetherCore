# Менеджер WASM-плагинов и WIT-спецификации (Plugin Engine & WIT Specs)

Документ описывает мозаичную архитектуру WASM-модулей, их макетирование `.nms-plugin` и интерфейсы спецификации Component Model WASI v2 в `nms-core`.

---

## 🏛️ Компоненты песочницы WASM

### 1. WIT-интерфейсы (`wit/nms-core.wit`)
Экспортирует 5 стандартных контрактов взаимодействия песочницы плагинов и ядра Rust:
- `nms:core/events@1.0.0`: Публикация и подписка на системные события шины (`publish`, `subscribe`).
- `nms:core/storage@1.0.0`: Изолированное Key-Value хранилище данных плагина (`get`, `set`).
- `nms:core/logger@1.0.0`: Вывод сообщений трассировки (`log-info`, `log-warn`, `log-error`).
- `nms:core/notify@1.0.0`: Отправка системных алертов.
- `nms:core/cron@1.0.0`: Запуск фоновых задач плагина.

### 2. Структура пакета `.nms-plugin` и `plugin.rs`
Плагин распространяется единым архивом, распаковываемым в `/modules/{module-id}/`:
- `manifest.yaml` — декларирует ID, имя, версию, роли `routes` и виджеты `widgets`.
- `backend.wasm` — серверный WASM-модуль бизнес-логики.
- `static/ui.js` — скомпилированный фронтенд-компонент (Vue 3 Web Component).

---

## 💡 Пример `manifest.yaml` плагина

```yaml
id: "ping-collector"
name: "Ping Network Collector"
version: "1.0.0"
description: "Периодический ICMP-опрос сетевых устройств"
enabled_by_default: true
plugin_type: "feature"
routes:
  - path: "/ping"
    name: "Ping Dashboard"
widgets:
  - id: "ping_summary"
    title: "Статус доступности узлов"
    component: "PingSummaryWidget"
```
