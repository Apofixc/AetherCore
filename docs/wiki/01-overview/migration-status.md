# Реестр статуса миграции NMS Next-Gen

Данный реестр отслеживает прогресс перевода подсистем приложения с Python на Rust + WASM.

---

## 📊 Матрица статуса компонентов

| Модуль / Подсистема | Компонент Rust | Статус | Документация |
| :--- | :--- | :---: | :--- |
| **Базовая структура** | Cargo Workspace (`nms-core`, `nms-cli`), Axum, Bus | 🟢 **Завершено** | `docs/wiki/01-overview/01-architecture.md` |
| **Системное логирование** | `LogProviders`, `tracing`, `LocalFileLogProvider` | 🟢 **Завершено** | `docs/wiki/03-module-development/01-logger.md` |
| **Планировщик задач** | Tokio Cron Scheduler (`SchedulerManager`) | 🟢 **Завершено** | `docs/wiki/03-module-development/02-scheduler.md` |
| **Auth & Users** | Argon2id, AES-256-GCM Crypto, JWT, WsTicketManager | 🟢 **Завершено** | `docs/wiki/02-usage/02-auth-users.md` |
| **Storage & Audit** | SQLx (SQLite/Postgres), Audit logger (`rotate_audit_logs`) | 🟢 **Завершено** | `docs/wiki/03-module-development/03-storage.md` |
| **Event Bus & WS** | `match_topic` wildcards, Axum WS `/api/v1/ws/events` | 🟢 **Завершено** | `docs/wiki/03-module-development/04-bus.md` |
| **Plugin Manager & WASM**| WIT Component Model specs, `PluginManager`, `manifest.yaml` | 🟢 **Завершено** | `docs/wiki/03-module-development/05-plugin-engine.md` |
| **Notifications Engine** | `NotificationEngine`, `send_notification`, `NotificationMessage` | 🟢 **Завершено** | `docs/wiki/03-module-development/06-notify.md` |
| **Rate Limiter** | `RateLimiter` (Sliding Window in-memory) | 🟢 **Завершено** | `docs/wiki/03-module-development/07-rate-limiter.md` |
| **Core REST API (Auth/Users)** | Axum `/api/v1/auth/*`, `/api/v1/users/*` | 🟢 **Завершено** | `docs/wiki/02-usage/02-auth-users.md` |
| **Core REST API (Notif/System)** | Axum `/api/v1/notifications/*`, `/api/v1/system/*` | 🟢 **Завершено** | `docs/wiki/02-usage/03-notifications-system.md` |
| **WASM Plugin Engine (spec 1.2)** | `PluginEngine`: discovery `.nms-plugin` (Zero-Unpack), `ModuleManifest` + JSON Schema, DAG toposort, Ed25519-подписи | 🟢 **Завершено** | `docs/wiki/03-module-development/12-plugin-manifest.md` |
| **Wasmtime Host Runtime** | Component Model + epoch interruption, лимиты памяти, actor-модель, Host API `nms:core@2.0.0` (events/storage/logger/notify/cron/rpc/net/i18n) | 🟢 **Завершено** | `docs/wiki/03-module-development/05-plugin-engine.md` |
| **WIT-контракты** | `wit/nms-core.wit` — мир `plugin`, экспорты lifecycle/event-consumer/timer-consumer/rpc-handler | 🟢 **Завершено** | `wit/nms-core.wit` |
| **Capability-based WASI (fs/env/сокеты)** | Проброс `allow_host_dirs`, `allow_env_vars`, `allowed_hosts` в WasiCtx | 🟡 **В разработке** | — |
| **Native ICMP (nms:core/net ping)** | Сейчас TCP-фолбэк; нативный ICMP — следующая фаза | 🟡 **В разработке** | — |
| **Tauri Desktop Mode** | Двухрежимный запуск (`--server` / GUI) | ⚪ **Ожидает** | — |
| **UI-плагины (ESM/SFC/Schema-Driven)** | Резолвер фронтенд-компонентов плагинов | ⚪ **Ожидает** | — |

---
*Обозначения: 🟢 Завершено | 🟡 В разработке | ⚪ Ожидает*
