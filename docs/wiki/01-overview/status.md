# Статус реализации компонентов платформы

| Модуль / Подсистема | Крейт / Путь | Статус | Тестовое покрытие |
| :--- | :--- | :--- | :--- |
| **Интернационализация (i18n)** | `crates/aethercore-common/src/i18n.rs` | ✅ Готово | 4 unit теста |
| **Единая система ошибок (AppError)** | `crates/aethercore-common/src/error.rs` | ✅ Готово | 3 unit теста |
| **Манифест и DAG резолвер** | `crates/aethercore-common/src/manifest.rs` | ✅ Готово | 4 unit теста |
| **База Данных (SQLite WAL)** | `crates/aethercore-core/src/db/mod.rs` | ✅ Готово | 1 unit тест |
| **Изолированное KV Хранилище** | `crates/aethercore-core/src/db/kv.rs` | ✅ Готово | 1 unit тест |
| **Гибридная шина событий** | `crates/aethercore-core/src/bus/mod.rs` | ✅ Готово | 1 unit тест |
| **Аутентификация (Argon2id + JWT)**| `crates/aethercore-core/src/auth/` | ✅ Готово | 2 unit теста |
| **Сервис пользователей (CRUD/RBAC)**| `crates/aethercore-core/src/users/` | ✅ Готово | 1 unit тест |
| **Сервис аудита и алертов** | `crates/aethercore-core/src/services/` | ✅ Готово | Интеграционный тест |
| **Zero-Unpack Загрузчик плагинов** | `crates/aethercore-core/src/plugins/loader.rs` | ✅ Готово | 1 unit тест |
| **Менеджер плагинов (PluginManager)**| `crates/aethercore-core/src/plugins/manager.rs`| ✅ Готово | 1 unit тест |
| **Axum Web-сервер и REST API** | `crates/aethercore-server/src/` | ✅ Готово | 3 интеграционных теста |
| **WebSocket Gateway** | `crates/aethercore-server/src/ws/` | ✅ Готово | Интеграционный тест |
| **CLI утилита (`nms` / `aethercore-cli`)** | `crates/aethercore-cli/src/main.rs` | ✅ Готово | Интеграционный тест |
| **Сквозной жизненный цикл** | `crates/aethercore-core/tests/` | ✅ Готово | 1 сквозной тест |
