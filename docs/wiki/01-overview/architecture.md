# Архитектура Next-Gen Universal Core Platform (Rust)

Платформа спроектирована как высокопроизводительное, безопасное и расширяемое микроядро на Rust с модульной плагинной архитектурой.

## Слои архитектуры

1. **Базовый фундамент (`aethercore-common`)**:
   - `i18n`: движок мультиязычной локализации с поддержкой динамических словарей плагинов.
   - `AppError`: единая типизированная система ошибок со сквозной локализацией.
   - `models`: модели учетных записей, ролевой модели (RBAC) и событий шины.
   - `manifest`: строгий декларативный контракт модуля `manifest.yaml` и DAG-резолвер зависимостей.

2. **Слой данных и коммуникации (`aethercore-core/db`, `aethercore-core/bus`)**:
   - **SQLite в WAL-режиме**: Single-Writer пул (монопольная фоновая запись без блокировок `database is locked`) и пул масштабируемого чтения.
   - **Изолированное KV-хранилище**: автоматическое разделение пространств имен `module:{id}:{key}`.
   - **Гибридная шина событий**: Live Broadcast (`tokio::sync::broadcast`) + Reliable Event Journal (SQLite WAL).

3. **Службы безопасности и сервисы (`aethercore-core/auth`, `aethercore-core/users`, `aethercore-core/services`)**:
   - Пароли: Argon2id.
   - Токены: JWT с проверкой гранулярных прав RBAC.
   - Сервисы: аудит (`AuditService`), уведомления (`NotifyService`), управление пользователями (`UserService`).

4. **WASM-песочница и плагины (`aethercore-core/plugins`, `wit/`)**:
   - Zero-Unpack загрузка плагинов напрямую из единых архивов `.aether-plugin` (ZIP) в память.
   - Цифровая Ed25519 подпись пакетов.
   - Интеграция интерфейсов по спецификации WIT `aether:core@2.0.0`.

5. **Сетевой шлюз и Web-сервер (`aethercore-server`)**:
   - Axum HTTP/WS сервер.
   - Core REST API (`/api/v1/auth`, `/api/v1/users`, `/api/v1/modules`, `/api/v1/system`, `/api/v1/events`).
   - WebSocket стриминг событий (`/ws/events`).
   - Потоковая раздача фронтенд-ассетов плагинов (`/modules/{id}/*path`).
