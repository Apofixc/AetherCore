# Архитектура и блок-схемы работы Frontend (Vue 3 + Pinia + Vite)

Данный документ описывает ключевые сценарии функционирования фронтенд-приложения AetherCore NMS Next-Gen: жизненный цикл приложения, навигационные проверки роутера, процедуру входа (включая «Запомнить меня», «Забыли код?», Rate Limiting & Lockout), таймер неактивности, первичную настройку со сложностью паролей и управление операторами.

---

## 1. Блок-схема: Жизненный цикл, проверка токена и Router Guard

```mermaid
graph TD
    Start([Пользователь открывает страницу]) --> InitStore[Инициализация Pinia authStore]
    InitStore --> CheckStorage[Проверка токена: localStorage || sessionStorage]
    CheckStorage --> FetchCfg[GET /api/v1/auth/config<br/>Загрузка политик безопасности]
    FetchCfg --> CheckWebAuth{Политика<br/>web_ui_auth активна?}

    %% Режим без авторизации
    CheckWebAuth -- Нет (false) --> AnonMode[Режим Anonymous Admin<br/>Виртуальный суперпользователь]
    AnonMode --> CheckTargetAnon{Целевой маршрут?}
    CheckTargetAnon -- /login --> RedirDashAnon[Редирект на /dashboard]
    CheckTargetAnon -- Другой маршрут --> AllowAnon[Разрешить переход]

    %% Режим с авторизацией
    CheckWebAuth -- Да (true) --> HasToken{Токен обнаружен?}
    HasToken -- Да --> ValidateMe[GET /api/v1/auth/me]
    ValidateMe --> CheckMeResult{Токен валиден и IP разрешен?}
    CheckMeResult -- Да (200 OK) --> StartTimer[Запуск startInactivityTracker<br/>Слушатели: mouse, key, scroll]
    StartTimer --> CheckTargetAuth{Маршрут /login?}
    CheckTargetAuth -- Да --> RedirDashAuth[Редирект на /dashboard]
    CheckTargetAuth -- Нет --> AllowRoute[Разрешить переход на страницу]
    CheckMeResult -- Нет (401 / 403) --> DropToken[Очистка токенов authStore.logout]
    DropToken --> RedirLogin[Редирект на /login]
    HasToken -- Нет --> IsPublicRoute{Маршрут публичный?}
    IsPublicRoute -- Да (/login) --> ShowLogin[Отобразить форму входа]
    IsPublicRoute -- Нет --> RedirLogin

    %% Глобальный перехват ошибок API
    ApiReq([API запрос ApiClient]) --> OnApiError{Статус ответа?}
    OnApiError -- 401 Unauthorized --> DropToken
    OnApiError -- 403 Forbidden (IP) --> ShowIpError[Баннера запрета доступа по IP]
    OnApiError -- 200 OK --> ResetTimer[Сброс таймера активности]
```

---

## 2. Диаграмма последовательности: Авторизация, «Запомнить меня», Блокировка (Rate Limit) и Восстановление

```mermaid
sequenceDiagram
    autonumber
    actor User as Оператор / Администратор
    participant UI as LoginView.vue
    participant Store as authStore (Pinia)
    participant API as ApiClient (/api/v1/auth)
    participant Core as Backend Axum (UserService & JWT)
    participant DB as SQLite DB

    %% Сценарий 1: Обычный вход / Запомнить меня
    Note over User, DB: Сценарий 1: Успешный вход и выбор хранилища токена
    User->>UI: Ввод operatorId, accessCode + флаг "Запомнить меня"
    UI->>Store: login(operatorId, accessCode, rememberMe)
    Store->>API: POST /api/v1/auth/login
    API->>Core: Проверка IP Whitelist + Аутентификация
    Core->>DB: Проверка пароля и locked_until
    DB-->>Core: Успешно (сброс failed_login_attempts = 0)
    Core-->>API: 200 OK { token, user } (TTL = session_ttl * 3600)
    API-->>Store: Возврат токена и профиля
    alt rememberMe == true
        Store->>Store: localStorage.setItem('nms_token')
        Store->>Store: localStorage.setItem('nms_remembered_operator')
    else rememberMe == false
        Store->>Store: sessionStorage.setItem('nms_token')
    end
    Store->>Store: startInactivityTracker()
    Store-->>UI: Успех

    %% Сценарий 2: Ошибка и Блокировка (Lockout)
    Note over User, DB: Сценарий 2: Неверный пароль и превышение лимита попыток
    User->>UI: Ввод неверного пароля
    UI->>Store: login(...)
    Store->>API: POST /api/v1/auth/login
    API->>Core: Проверка пароля
    Core->>DB: Инкремент failed_login_attempts
    alt failed_login_attempts >= max_login_attempts (5)
        Core->>DB: Установка locked_until = now + lockout_duration min
        Core-->>API: 401 Unauthorized (Account locked)
    else failed_login_attempts < max_login_attempts
        Core-->>API: 401 Unauthorized (Invalid credentials)
    end
    API-->>UI: Отображение ошибки и времени блокировки

    %% Сценарий 3: Забыли код?
    Note over User, UI: Сценарий 3: Восстановление доступа ("Забыли код?")
    User->>UI: Клик на "Забыли код?"
    UI->>UI: showForgotCodeModal = true
    UI->>User: Показ модального окна со справкой и командой CLI
```

---

## 3. Блок-схема: Контроль неактивности пользователя (Inactivity Timeout)

```mermaid
graph TD
    UserActive["Пользователь выполняет действие<br/>(mousemove, mousedown, keydown, touch, scroll)"] --> ResetTimer["Сброс таймера:<br/>clearTimeout + setTimeout на inactivity_timeout мин"]
    ResetTimer --> IdleWait{"Пользователь активен<br/>до истечения таймера?"}
    
    IdleWait -- Да --> UserActive
    
    IdleWait -- Нет (таймаут истек) --> TriggerTimeout["Срабатывание таймера неактивности"]
    TriggerTimeout --> ClearAuth["authStore.logout:<br/>Очистка токенов и сброс слушателей"]
    ClearAuth --> SetReason["Выставление sessionExpired = true"]
    SetReason --> RedirectLogin["Редирект на /login?reason=inactivity"]
    RedirectLogin --> ShowBanner["Отображение предупреждения в LoginView"]
```

---

## 4. Диаграмма: Первичная настройка и проверка сложности паролей (Password Complexity)

```mermaid
sequenceDiagram
    autonumber
    actor User as Пользователь (Первый вход)
    participant UI as LoginView.vue (Modal)
    participant Store as authStore
    participant API as ApiClient (/api/v1/users)
    participant Core as Backend UserService

    Note over User, Core: При первом входе (!last_login_at || must_change_password)
    UI->>UI: Открытие модального окна "Первичная настройка"
    User->>UI: Ввод нового логина и пароля
    
    loop Интерактивная валидация в UI
        UI->>UI: Проверка passwordRequirements
        UI->>User: Динамическая подсветка пунктов чеклиста
    end

    User->>UI: Клик "Сохранить и войти"
    UI->>API: PUT /api/v1/users/:id { username, password, must_change_password: false }
    API->>Core: validate_password_complexity(password, policy)
    alt Пароль не соответствует политике
        Core-->>API: 422 Unprocessable Entity (Validation Error)
        API-->>UI: Вывод ошибки валидации
    else Пароль соответствует
        Core->>Core: Argon2id hash + сброс failed_login_attempts и locked_until
        Core-->>API: 200 OK (User updated)
        API-->>UI: Успешное сохранение
        UI->>UI: router.push('/dashboard')
    end
```

---

## 5. Тест-кейсы для верификации Frontend изменений

### ТК-1: Запоминание оператора и разделение хранилищ токена
* **Given**: Пользователь открывает экран логина `/login`.
* **When**: Пользователь вводит логин `operator1`, отмечает «Запомнить меня» и успешно входит в систему.
* **Then**: Токен записан в `localStorage.getItem('nms_token')`, логин сохранен в `localStorage.getItem('nms_remembered_operator')`. При следующем открытии страницы логин подставляется автоматически.
* **When 2**: Пользователь выходит и входит без галочки «Запомнить меня».
* **Then 2**: Токен записан только в `sessionStorage.getItem('nms_token')`, в `localStorage` токен отсутствует.

### ТК-2: Автоматический выход по таймауту неактивности
* **Given**: Пользователь авторизован, в политиках безопасности установлен `inactivity_timeout = 15`.
* **When**: Пользователь не совершает действий (мышь, клавиатура, скролл) в течение 15 минут.
* **Then**: `authStore` вызывает `logout()`, очищает хранилище и перенаправляет на `/login?reason=inactivity` с выводом сообщения «Сессия завершена из-за длительного отсутствия активности».

### ТК-3: Модальное окно «Забыли код?»
* **Given**: Пользователь находится на странице `/login`.
* **When**: Пользователь нажимает на ссылку «Забыли код?».
* **Then**: Открывается модальное окно `BaseModal` с контактами администратора и готовой CLI-командой для аварийного сброса пароля.

### ТК-4: Визуальный чеклист сложности пароля при первом входе
* **Given**: Новый пользователь выполняет первый вход с временным паролем.
* **When**: Появляется модальное окно первичной настройки и пользователь начинает ввод нового пароля.
* **Then**: Под полем ввода динамически подсвечиваются требования (минимальная длина, заглавная буква, цифра, спецсимвол). Кнопка подтверждения блокирует отправку, пока все требования активной политики безопасности не будут удовлетворены.

---

## 6. Блок-схема: Управление политиками безопасности и сессиями в UI

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Администратор
    participant View as AccessIdentityView.vue
    participant Store as authStore
    participant API as ApiClient (/api/v1/settings)
    participant Core as Backend Axum

    Admin->>View: Изменение настроек (таймауты, политики, матрица)
    Admin->>View: Клик "Применить изменения"
    
    %% Последовательное сохранение для избежания race condition
    View->>API: 1. PUT /api/v1/settings/permissions (матрица прав)
    API->>Core: Сохранение матрицы
    Core-->>API: 200 OK
    
    View->>API: 2. PUT /api/v1/settings/security (политики безопасности)
    API->>Core: Запись настроек в SQLite KV Store
    Core-->>API: 200 OK
    
    View->>Store: authStore.checkAuthConfig()
    
    alt Переход: web_ui_auth включен (из выключенного состояния)
        View->>Store: authStore.logout() (сброс анонимной сессии)
        View->>View: window.location.href = '/login' (переход на форму авторизации)
    else Авторизация уже была активна или осталась выключена
        View->>View: Отображение плашки "Изменения применены!" (сессия сохраняется)
    end
```

