# Статус реализации компонентов платформы

| Модуль / Подсистема | Крейт / Путь | Статус | Тестовое покрытие |
| :--- | :--- | :--- | :--- |
| **Интернационализация (i18n)** | `crates/nms-common/src/i18n.rs` | ✅ Готово | 4 unit теста |
| **Единая система ошибок (AppError)** | `crates/nms-common/src/error.rs` | ✅ Готово | 3 unit теста |
| **Манифест и DAG резолвер** | `crates/nms-common/src/manifest.rs` | ✅ Готово | 4 unit теста |
| **База Данных (SQLite WAL)** | `crates/nms-core/src/db/mod.rs` | ✅ Готово | 1 unit тест |
| **Изолированное KV Хранилище** | `crates/nms-core/src/db/kv.rs` | ✅ Готово | 1 unit тест |
| **Гибридная шина событий** | `crates/nms-core/src/bus/mod.rs` | ✅ Готово | 1 unit тест |
| **Аутентификация (Argon2id + JWT)**| `crates/nms-core/src/auth/` | ✅ Готово | 2 unit теста |
| **Сервис пользователей (CRUD/RBAC)**| `crates/nms-core/src/users/` | ✅ Готово | 1 unit тест |
| **Сервис аудита и алертов** | `crates/nms-core/src/services/` | ✅ Готово | Интеграционный тест |
| **Zero-Unpack Загрузчик плагинов** | `crates/nms-core/src/plugins/loader.rs` | ✅ Готово | 1 unit тест |
| **Менеджер плагинов (PluginManager)**| `crates/nms-core/src/plugins/manager.rs`| ✅ Готово | 1 unit тест |
| **Axum Web-сервер и REST API** | `crates/nms-server/src/` | ✅ Готово | 3 интеграционных теста |
| **WebSocket Gateway** | `crates/nms-server/src/ws/` | ✅ Готово | Интеграционный тест |
| **CLI утилита (`nms` / `nms-cli`)** | `crates/nms-cli/src/main.rs` | ✅ Готово | Интеграционный тест |
| **Сквозной жизненный цикл** | `crates/nms-core/tests/` | ✅ Готово | 1 сквозной тест |
