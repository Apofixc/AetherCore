# 📚 Next-Gen Universal Core Platform — Официальная Вики и База Знаний

База знаний и техническая документация микроядерной платформы **Next-Gen Universal Core Platform** (Rust, WebAssembly Component Model, SQLite WAL, Vue 3 Shell).

---

## 📑 Оглавление базы знаний

### 🏛️ Раздел 1. Архитектура и общие сведения (`01-overview/`)
1. [01-architecture.md](file:///opt/NMSNext-Gen/docs/wiki/01-overview/01-architecture.md) — Системный дизайн ядра, Dual-Mode (Server/Tauri), модель потоков, Single-Writer SQLite WAL, изоляция песочницы Wasmtime.
2. [02-sizing-and-benchmarks.md](file:///opt/NMSNext-Gen/docs/wiki/01-overview/02-sizing-and-benchmarks.md) — Сайзинг аппаратных ресурсов под масштаб сети, бенчмарки и сравнение производительности с Python.
3. [status.md](file:///opt/NMSNext-Gen/docs/wiki/01-overview/status.md) — Реестр статуса реализации компонентов и тестовое покрытие.

---

### 🚀 Раздел 2. Руководство по эксплуатации и использованию (`02-usage/`)
1. [01-deployment-and-configuration.md](file:///opt/NMSNext-Gen/docs/wiki/02-usage/01-deployment-and-configuration.md) — Установка, флаги CLI (`--server`, `--dev`, `--safe-mode`), `config.toml`, systemd сервис, управление пакетами `.nms-plugin`.
2. [02-security-and-rbac.md](file:///opt/NMSNext-Gen/docs/wiki/02-usage/02-security-and-rbac.md) — Модель безопасности: Argon2id, JWT, матрица прав доступа RBAC, журнал аудита действий.
3. [03-rest-and-websocket-api.md](file:///opt/NMSNext-Gen/docs/wiki/02-usage/03-rest-and-websocket-api.md) — Справочник REST API эндпоинтов, структуры ответов об ошибках, WebSocket шлюз `/ws/events`.
4. [04-troubleshooting-and-diagnostics.md](file:///opt/NMSNext-Gen/docs/wiki/02-usage/04-troubleshooting-and-diagnostics.md) — Диагностика, уровни трассировки `RUST_LOG`, сброс паролей и безопасный режим.
5. **Пользовательский интерфейс (Frontend UI)**:
   - [ui/01-shell-and-navigation.md](file:///opt/NMSNext-Gen/docs/wiki/02-usage/ui/01-shell-and-navigation.md) — Vue 3 Shell, реактивная навигация, WebSocket стримы.
   - [ui/02-plugin-manager.md](file:///opt/NMSNext-Gen/docs/wiki/02-usage/ui/02-plugin-manager.md) — Менеджер плагинов, автогенерация форм настроек (Schema-Driven Forms).

---

### 🧩 Раздел 3. Руководство разработчика модулей и плагинов (`03-module-development/`)
1. [quickstart.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/quickstart.md) — Пошаговое руководство создания и сборки первого модуля.
2. [01-environment-and-standards.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/01-environment-and-standards.md) — Настройка окружения (Rust `wasm32-wasip2`, `cargo-component`), стандарты кода.
3. [02-manifest-specification.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/02-manifest-specification.md) — Полная спецификация манифеста `manifest.yaml` (capabilities, routes, widgets, config_schema).
4. **Спецификации системных WIT-контрактов (`nms:core/*`)**:
   - [03-wit-core-services/01-events.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/03-wit-core-services/01-events.md) — Гибридная шина событий (`publish-telemetry`, `publish-reliable`).
   - [03-wit-core-services/02-storage.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/03-wit-core-services/02-storage.md) — Изолированное KV-хранилище (`get`, `set`, `delete`).
   - [03-wit-core-services/03-logger.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/03-wit-core-services/03-logger.md) — Структурированное логирование плагинов.
   - [03-wit-core-services/04-notify.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/03-wit-core-services/04-notify.md) — Отправка системных алертов.
   - [03-wit-core-services/05-i18n.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/03-wit-core-services/05-i18n.md) — Интернационализация и локализация.
   - [03-wit-core-services/06-rpc.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/03-wit-core-services/06-rpc.md) — Межмодульные RPC вызовы.
5. [04-ui-delivery-levels.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/04-ui-delivery-levels.md) — 3 уровня поставки интерфейса: Schema-driven, Vue SFC, ESM dist.
6. [05-debugging-and-profiling.md](file:///opt/NMSNext-Gen/docs/wiki/03-module-development/05-debugging-and-profiling.md) — Перехват паник (Wasmtime Traps), таймауты и профилирование.
