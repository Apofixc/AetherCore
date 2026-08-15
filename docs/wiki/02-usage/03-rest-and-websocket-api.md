# 📡 Справочник REST API и WebSocket Шлюза

Базовый префикс всех REST эндпоинтов ядра: `/api/v1`.

---

## 1. Аутентификация (`/api/v1/auth`)

### `POST /api/v1/auth/login`
Вход пользователя по логину и паролю с получением JWT токена.
- **Тело запроса**:
  ```json
  {
    "username": "admin",
    "password": "your-password"
  }
  ```
- **Ответ (200 OK)**:
  ```json
  {
    "success": true,
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "user": {
      "id": "c1f7a0e2-892b-426b-b461-825d733519c7",
      "username": "admin",
      "full_name": "System Administrator",
      "email": "admin@nms.local",
      "is_active": true,
      "is_superuser": true,
      "roles": ["admin"],
      "permissions": ["system.view", "system.manage", "users.view", "users.manage", "modules.view", "modules.manage", "events.view"],
      "created_at": "2026-08-15T12:00:00Z",
      "last_login_at": "2026-08-15T18:00:00Z"
    }
  }
  ```

### `GET /api/v1/auth/me`
Получение профиля текущего пользователя по Bearer токену.
- **Заголовки**: `Authorization: Bearer <token>`
- **Ответ (200 OK)**: объект `UserResponseDto`.

---

## 2. Управление пользователями (`/api/v1/users`)

Все методы требуют заголовок `Authorization: Bearer <token>`.

| Метод | Путь | Требуемое право | Описание |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/users` | `users.view` | Список всех пользователей |
| `POST` | `/api/v1/users` | `users.manage` | Создание нового пользователя |
| `GET` | `/api/v1/users/{id}` | `users.view` | Получение пользователя по UUID |
| `PUT` | `/api/v1/users/{id}` | `users.manage` | Обновление профиля/пароля пользователя |
| `DELETE` | `/api/v1/users/{id}` | `users.manage` | Удаление пользователя |

---

## 3. Управление модулями (`/api/v1/modules`)

| Метод | Путь | Требуемое право | Описание |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/modules` | `modules.view` | Список установленных плагинов с манифестами и статусом активности |
| `GET` | `/api/v1/modules/{id}` | `modules.view` | Детальная информация о плагине |
| `POST` | `/api/v1/modules/{id}/enable` | `modules.manage` | Включение модуля |
| `POST` | `/api/v1/modules/{id}/disable` | `modules.manage` | Отключение модуля |
| `GET` | `/api/v1/modules/{id}/config` | `modules.view` | Получение текущей конфигурации модуля |
| `PUT` | `/api/v1/modules/{id}/config` | `modules.manage` | Сохранение настроек модуля (валидация по JSON Schema) |

---

## 4. Прямая раздача фронтенд-ассетов модулей (`/modules/{id}/*path`)

- **Маршрут**: `GET /modules/{id}/{*path}`
- **Описание**: потоковое чтение статических файлов интерфейса (ESM-бандлы `dist/ui.js`, SFC `views/*.vue`, стили, иконки) напрямую из архива `.nms-plugin` без распаковки на диск.
- **Заголовки кэширования**: `Cache-Control: public, max-age=31536000, immutable`.

---

## 5. Системные эндпоинты (`/api/v1/system`)

- `GET /api/v1/system/info` — статус ядра, версия, время непрерывной работы (uptime), флаги `--dev` и `--safe-mode`.
- `GET /api/v1/system/i18n/{locale}` — выгрузка полного словаря локализации (ядро + все активные плагины) для фронтенда.
- `GET /api/v1/system/audit` — чтение журнала аудита действий пользователей.

---

## 6. WebSocket Шлюз Событий (`/ws/events`)

- **Подключение**: `ws://<host>:<port>/ws/events?token=<jwt_token>`
- **Формат сообщений**: JSON объекты `EventMessage`.
- **Пример полезной нагрузки**:
  ```json
  {
    "id": "e8d4a942-019a-4c28-9896-d2ef65bf92a5",
    "topic": "example-plugin.status_updated",
    "event_type": "telemetry",
    "source": "example-plugin",
    "payload": {
      "status": "online",
      "latency_ms": 14.5
    },
    "timestamp": "2026-08-15T18:10:00Z"
  }
  ```
