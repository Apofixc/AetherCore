# Архитектура интеграции Frontend с REST API AetherCore

## Диаграмма взаимодействия (Mermaid)

```mermaid
sequenceDiagram
    autonumber
    actor Operator as Оператор / Администратор
    participant UI as Frontend (Vue 3 / Pinia)
    participant Client as ApiClient (fetch + JWT)
    participant Axum as Axum REST API (/api/v1)
    participant Core as AetherCore Services
    participant DB as SQLite DB & Event Bus

    %% Авторизация
    Operator->>UI: Ввод логина и пароля
    UI->>Client: authApi.login(username, password)
    Client->>Axum: POST /api/v1/auth/login
    Axum->>Core: UserService::authenticate
    Core-->>Axum: Ok(User + Token)
    Axum-->>Client: 200 OK { token, user }
    Client-->>UI: Сохранение токена в localStorage
    UI->>UI: Переход на /dashboard

    %% Загрузка модулей и пользователей
    UI->>Client: modulesApi.list() & usersApi.list()
    Client->>Axum: GET /api/v1/modules (Authorization: Bearer)
    Client->>Axum: GET /api/v1/users (Authorization: Bearer)
    Axum->>Core: PluginManager / UserService
    Core-->>Axum: Список объектов
    Axum-->>Client: 200 OK [ModuleSummaryDto, UserResponseDto]
    Client-->>UI: Обновление Pinia Store & UI
```

## Тест-кейсы для верификации

### ТК-1: Сквозная аутентификация и инициализация сессии
* **Given**: Пользователь находится на странице `/login`, токен в `localStorage` отсутствует.
* **When**: Пользователь вводит валидные `username` и `password` и нажимает "Войти".
* **Then**: Отправляется запрос `POST /api/v1/auth/login`. При ответе `200 OK` JWT-токен сохраняется в `localStorage`, обновляется `authStore.user`, и происходит редирект на `/dashboard`.

### ТК-2: Обработка истечения токена (401 Unauthorized)
* **Given**: Пользователь выполняет запрос с устаревшим или невалидным JWT-токеном.
* **When**: Бэкенд возвращает статус `401 Unauthorized`.
* **Then**: `ApiClient` сбрасывает сохраненный токен, и маршрутизатор перенаправляет пользователя на `/login`.
