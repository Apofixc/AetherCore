# 📄 Спецификация контракта манифеста (`manifest.yaml`)

Файл `manifest.yaml` является **единым декларативным контрактом** между плагином и микроядром платформы.

---

## 1. Пример полного манифеста

```yaml
manifest_version: 1
id: "example-plugin"
name: "Демонстрационный Модуль"
version: "1.0.0"
description: "Универсальный плагин платформы"
type: "feature"                  # system | feature | driver
enabled_by_default: true
min_core_version: "2.0.0"

# Граф зависимостей
deps:
  - "network-topology"
optional_deps:
  - "alert-telegram"

# Системные привилегии песочницы Wasmtime
capabilities:
  network:
    allow_raw_sockets: false
    allowed_hosts: []
  filesystem:
    allow_host_dirs: []
  environment:
    allow_env_vars: []

# Шина событий
events:
  publishes:
    - "example-plugin.status_updated"
    - "example-plugin.metric_tick"
  subscribes:
    - "core.system_started"

# Регистрация UI страниц в Shell оболочке
routes:
  - path: "/demo"
    name: "demo-dashboard"
    component: "frontend/dist/ui.js#DemoDashboardView"
    meta:
      title: "Демо Панель"
      icon: "activity"
      group: "general"
      requires_auth: true
      permissions: ["example.view"]

# Меню навигации
menu:
  location: "sidebar"
  group: "Обзор"
  items:
    - path: "/demo"
      label: "Демо Модуль"
      icon: "activity"

# Виджеты Dashboard
widgets:
  - id: "demo_summary_widget"
    title: "Статус Демо Плагина"
    component: "frontend/dist/ui.js#DemoSummaryWidget"
    size: "medium"
    refresh_interval: 15
    endpoint: "/api/v1/modules/example-plugin/summary"
    view_permission: "example.view"

# Регистрируемые права доступа RBAC
permissions:
  - id: "example.view"
    name: "Просмотр демонстрационного плагина"
    category: "Demo"
    description: "Разрешение на просмотр дашборда"
  - id: "example.manage"
    name: "Управление демонстрационным плагином"
    category: "Demo"
    description: "Разрешение на изменение настроек"

# Схема настроек (JSON Schema Draft-07)
config_schema:
  type: "object"
  required: ["refresh_interval_sec"]
  properties:
    refresh_interval_sec:
      type: "integer"
      minimum: 1
      maximum: 3600
      default: 10
    debug_mode:
      type: "boolean"
      default: false

# Хуки жизненного цикла
hooks:
  install: "init_schema"
  on_enable: "start_worker"
  on_disable: "stop_worker"

# Директории ассетов
assets:
  cache_dirs: ["cache/"]
  data_dirs: ["data/"]
```

---

## 2. Описание секций и правил валидации

1. **`id` (строка)**: kebab-case идентификатор. Все топики в `events.publishes` обязаны начинаться с `{id}.` (защита от перехвата чужих событий).
2. **`capabilities`**: объявление запрашиваемых привилегий песочницы (WASI сокеты, проброс путей ФС).
3. **`config_schema`**: JSON Schema настроек. Фронтенд автоматически строит по ней форму, а ядро валидирует ввод перед сохранением в `KvStore`.
4. **`deps` / `optional_deps`**: граф модулей. Ядро вычисляет топологический порядок инициализации перед стартом.
