# Архитектура и блок-схемы работы Frontend (Vue 3 + Pinia + Vite)

Данный документ описывает ключевые сценарии функционирования фронтенд-приложения AetherCore NMS Next-Gen: жизненный цикл приложения, навигационные проверки роутера, процедуру первого входа и управления операторами.

---

## 1. Блок-схема: Жизненный цикл и навигация Router Guard

```mermaid
graph TD
    Start([Пользователь открывает страницу]) --> InitStore[Инициализация Pinia authStore]
    InitStore --> FetchCfg[GET /api/v1/auth/config<br/>Проверка политик безопасности]
    FetchCfg --> CheckWebAuth{Политика<br/>web_ui_auth включена?}

    %% Режим без авторизации
    CheckWebAuth -- Нет (false) --> AnonMode[Режим Anonymous Admin<br/>Виртуальный суперпользователь]
    AnonMode --> CheckTargetAnon{Целевой маршрут?}
    CheckTargetAnon -- /login --> RedirDashAnon[Редирект на /dashboard]
    CheckTargetAnon -- Другой маршрут --> AllowAnon[Разрешить переход]

    %% Режим с авторизацией
    CheckWebAuth -- Да (true) --> CheckToken{Есть токен в localStorage?}
    CheckToken -- Да --> ValidateMe[GET /api/v1/auth/me]
    ValidateMe --> CheckMeResult{Токен валиден?}
    CheckMeResult -- Да --> CheckTargetAuth{Маршрут /login?}
    CheckTargetAuth -- Да --> RedirDashAuth[Редирект на /dashboard]
    CheckTargetAuth -- Нет --> AllowRoute[Разрешить переход на страницу]
    CheckMeResult -- Нет (401) --> DropToken[Очистка токена authStore.logout]
    DropToken --> RedirLogin[Редирект на /login]
    CheckToken -- Нет --> IsPublicRoute{Маршрут публичный?}
    IsPublicRoute -- Да (/login) --> ShowLogin[Отобразить форму логина]
    IsPublicRoute -- Нет --> RedirLogin

    %% Глобальный перехват 401
    ApiReq([Любой API запрос ApiClient]) --> On401{Статус ответа 401?}
    On401 -- Да (и не /login) --> DropToken
    On401 -- Нет --> ProcessResponse[Обработка данных]
```

---

## 2. Блок-схема: Процесс входа и первичная настройка (First-Time Setup)

```mermaid
sequenceDiagram
    autonumber
    actor User as Пользователь / Оператор
    participant UI as LoginView.vue
    participant Store as authStore (Pinia)
    participant API as ApiClient (/api/v1/auth)
    participant Core as Backend Axum

    User->>UI: Ввод логина и пароля
    UI->>Store: login(operatorId, accessCode)
    Store->>API: POST /api/v1/auth/login
    API->>Core: Аутентификация Argon2id + генерация JWT
    Core-->>API: 200 OK { token, user: { must_change_password, ... } }
    API-->>Store: Сохранение токена и профиля
    Store-->>UI: Успешный вход

    alt must_change_password == true (Первый вход)
        UI->>UI: Блокировка входа в Dashboard<br/>Открытие модалки "Первичная настройка"
        User->>UI: Ввод постоянного логина (опционально) + новый пароль
        UI->>API: PUT /api/v1/users/:id { username, password, must_change_password: false }
        API->>Core: Валидация логина (уникальность) + Argon2id hash
        Core-->>API: 200 OK (updated user)
        API-->>UI: Профиль обновлен
        UI->>UI: router.push('/dashboard')
    else must_change_password == false
        UI->>UI: router.push('/dashboard')
    end
```

---

## 3. Блок-схема: Управление пользователями администратором

```mermaid
graph TD
    OpenUsers([Администратор открывает раздел 'Пользователи']) --> LoadList[GET /api/v1/users]
    LoadList --> DisplayTable[Отображение таблицы операторов]

    %% Добавление
    DisplayTable --> ActionAdd[Клик 'Добавить пользователя']
    ActionAdd --> FillNew[Заполнение: ФИО, логин, email, пароль, роль]
    FillNew --> PolicyCheck{Политика mandatory_password_change?}
    PolicyCheck -- Да --> FlagTrue[must_change_password = true]
    PolicyCheck -- Нет --> FlagManual[По выбору чекбокса]
    FlagTrue --> SendCreate[POST /api/v1/users]
    FlagManual --> SendCreate
    SendCreate --> RefreshList[Обновление таблицы]

    %% Редактирование (Только роль и сброс пароля)
    DisplayTable --> ActionEdit[Клик 'Редактировать']
    ActionEdit --> ShowEditModal[Открытие модалки: Инфо-карточка пользователя]
    ShowEditModal --> EditRole[Изменение роли: Superuser/Admin/Operator/Viewer]
    ShowEditModal --> ResetPwd[Ввод нового пароля для сброса + must_change_password]
    EditRole --> SendUpdate[PUT /api/v1/users/:id]
    ResetPwd --> SendUpdate
    SendUpdate --> RefreshList

    %% Блокировка
    DisplayTable --> ActionLock[Клик 'Заблокировать / Разблокировать']
    ActionLock --> CheckSuper{Пользователь Superuser?}
    CheckSuper -- Да --> DenyLock[Действие заблокировано в UI и на бэкенде]
    CheckSuper -- Нет --> SendLock[PUT /api/v1/users/:id is_active: !current]
    SendLock --> RefreshList
```

---

## 4. Блок-схема: Управление политиками безопасности и инвалидация сессий

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Администратор
    participant View as AccessIdentityView.vue
    participant Store as authStore
    participant API as ApiClient (/api/v1/settings)
    participant Core as Backend Axum

    Admin->>View: Переключение web_ui_auth: false -> true
    Admin->>View: Клик "Применить изменения"
    
    %% Последовательное сохранение для избежания race condition
    View->>API: 1. PUT /api/v1/settings/permissions (матрица прав)
    API->>Core: Сохранение матрицы
    Core-->>API: 200 OK
    
    View->>API: 2. PUT /api/v1/settings/security (политики безопасности)
    API->>Core: Запись web_ui_auth: true в SQLite KV Store
    Core-->>API: 200 OK
    
    View->>Store: authStore.checkAuthConfig()
    View->>Store: authStore.logout() (удаление JWT)
    View->>View: window.location.href = '/login' (принудительный выброс на вход)
```
