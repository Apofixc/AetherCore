# Архитектура и блок-схемы работы Frontend (Vue 3 + Pinia + Vite)

Данный документ описывает ключевые сценарии функционирования фронтенд-приложения AetherCore Platform Next-Gen: жизненный цикл приложения, навигационные проверки роутера, процедуру входа (включая «Запомнить меня», «Забыли код?», Rate Limiting & Lockout), таймер неактивности, первичную настройку со сложностью паролей и управление операторами.

---

## 1. Блок-схема: Жизненный цикл, проверка токена и Router Guard

```mermaid
graph TD
    Start(["Пользователь открывает страницу"]) --> InitStore["Инициализация Pinia authStore"]
    InitStore --> CheckStorage["Проверка токена: localStorage или sessionStorage"]
    CheckStorage --> FetchCfg["GET /api/v1/auth/config<br/>Загрузка политик безопасности"]
    FetchCfg --> CheckWebAuth{"Политика<br/>web_ui_auth активна?"}
    %% Режим без авторизации
    CheckWebAuth -- "Нет (false)" --> AnonMode["Режим Anonymous Admin<br/>Виртуальный суперпользователь"]
    AnonMode --> CheckTargetAnon{"Целевой маршрут?"}
    CheckTargetAnon -- "/login" --> RedirDashAnon["Редирект на /dashboard"]
    CheckTargetAnon -- "Другой маршрут" --> AllowAnon["Разрешить переход"]
    %% Режим с авторизацией
    CheckWebAuth -- "Да (true)" --> HasToken{"Токен обнаружен?"}
    HasToken -- "Да" --> ValidateMe["GET /api/v1/auth/me"]
    ValidateMe --> CheckMeResult{"Токен валиден и IP разрешен?"}
    CheckMeResult -- "Да (200 OK)" --> StartTimer["Запуск startInactivityTracker<br/>Слушатели: mouse, key, scroll"]
    StartTimer --> CheckTargetAuth{"Маршрут /login?"}
    CheckTargetAuth -- "Да" --> RedirDashAuth["Редирект на /dashboard"]
    CheckTargetAuth -- "Нет" --> AllowRoute["Разрешить переход на страницу"]
    CheckMeResult -- "Нет (401 / 403)" --> DropToken["Очистка токенов authStore.logout"]
    DropToken --> RedirLogin["Редирект на /login"]
    HasToken -- "Нет" --> IsPublicRoute{"Маршрут публичный?"}
    IsPublicRoute -- "Да (/login)" --> ShowLogin["Отобразить форму входа"]
    IsPublicRoute -- "Нет" --> RedirLogin
    %% Глобальный перехват ошибок API
    ApiReq(["API запрос ApiClient"]) --> OnApiError{"Статус ответа?"}
    OnApiError -- "401 Unauthorized" --> DropToken
    OnApiError -- "403 Forbidden (IP)" --> ShowIpError["Баннера запрета доступа по IP"]
    OnApiError -- "200 OK" --> ResetTimer["Сброс таймера активности"]
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
    %% Сценарий 1: Обычный вход / 2FA Challenge / Запомнить меня
    Note over User, DB: Сценарий 1: Вход с проверкой 2FA и выбор хранилища токена
    User->>UI: Ввод operatorId, accessCode + флаг "Запомнить меня"
    UI->>Store: login(operatorId, accessCode, rememberMe)
    Store->>API: POST /api/v1/auth/login
    API->>Core: Проверка IP Whitelist + Аутентификация
    Core->>DB: Проверка пароля и locked_until
    DB-->>Core: Успешно (сброс failed_login_attempts = 0)
    alt is_totp_enabled == false
        Core-->>API: 200 OK { token, user, requires_2fa: false }
        API-->>Store: Возврат токена и профиля
        alt rememberMe == true
            Store->>Store: localStorage.setItem('aether_token')
            Store->>Store: localStorage.setItem('aether_remembered_operator')
        else rememberMe == false
            Store->>Store: sessionStorage.setItem('aether_token')
        end
        Store->>Store: startInactivityTracker()
        Store-->>UI: Редирект на /dashboard
    else is_totp_enabled == true
        Core-->>API: 200 OK { requires_2fa: true, temp_token: "JWT(2fa_pending)" }
        API-->>Store: store.tempToken = temp_token, store.requires2fa = true
        Store-->>UI: is2faStep = true (переключение на форму ввода 2FA)
        User->>UI: Ввод 6-значного TOTP или резервного кода XXXX-XXXX
        UI->>Store: verify2faLogin(code, isBackupCode)
        Store->>API: POST /api/v1/auth/2fa/verify-login { temp_token, code }
        API->>Core: Валидация temp_token + Проверка TOTP / Backup Code
        Core->>DB: При бэкап-коде: удаление использованного хэша из БД
        Core-->>API: 200 OK { token, user }
        API-->>Store: Сохранение токена в storage + startInactivityTracker()
        Store-->>UI: Редирект на /dashboard
    end
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
    IdleWait -- "Да" --> UserActive
    IdleWait -- "Нет (таймаут истек)" --> TriggerTimeout["Срабатывание таймера неактивности"]
    TriggerTimeout --> ClearAuth["authStore.logout:<br/>Очистка токенов и сброс слушателей"]
    ClearAuth --> SetReason["Выставление sessionExpired = true"]
    SetReason --> RedirectLogin["Редирект на /login?reason=inactivity"]
    RedirectLogin --> ShowBanner["Отображение предупреждения в LoginView"]
```

---

## 4. Блок-схема и Sequence-диаграмма: Пошаговый мастер первичной настройки (Onboarding Wizard)

### А. Блок-схема состояний онбординга и авторизации

```mermaid
graph TD
    LoginSuccess(["Успешный вход POST /api/v1/auth/login"]) --> CheckRoot{"Пользователь root?"}
    %% Аккаунт root минует мастер
    CheckRoot -- "Да (username == 'root')" --> DashDirect["Прямой переход на /dashboard"]
    %% Обычный пользователь
    CheckRoot -- "Нет" --> EvalConditions{"Проверка статуса аккаунта"}
    EvalConditions -->|"!is_username_locked && must_change_password"| FullWizard["Двухшаговый мастер: Шаг 1 Логин → Шаг 2 Пароль"]
    EvalConditions -->|"!is_username_locked && !must_change_password"| UsernameOnly["Окно настройки логина: Логин → Сохранить"]
    EvalConditions -->|"is_username_locked && must_change_password"| PasswordOnly["Окно смены пароля: Новый пароль → Сохранить"]
    EvalConditions -->|"is_username_locked && !must_change_password"| DashDirect
    %% Шаг 1: Логин
    FullWizard --> Step1["Шаг 1: Выбор постоянного логина"]
    UsernameOnly --> Step1Solo["Шаг 1: Выбор постоянного логина"]
    Step1 --> UserStep1Action{"Действие пользователя"}
    UserStep1Action -- "Оставить текущий" --> SetCurrentUname["Сохранить текущий username"] --> Step2["Шаг 2: Новый постоянный пароль"]
    UserStep1Action -- "Ввод логина + Далее" --> ValidateUname{"Валидация логина<br/>(3-32 симв., [a-z0-9._-])"}
    ValidateUname -- "Невалиден" --> ShowUnameErr["Ошибка формата логина"] --> Step1
    ValidateUname -- "Валиден" --> Step2
    UsernameOnly --> UserStep1SoloAction{"Действие пользователя"}
    UserStep1SoloAction -- "Сохранить и войти" --> SaveUsernameSolo["PUT /api/v1/users/:id<br/>{ username, is_username_locked: true }"] --> GoDash
    %% Шаг 2: Пароль
    PasswordOnly --> Step2Solo["Окно обязательной смены пароля"]
    Step2Solo --> SavePwdSolo["Ввод пароля + Сохранить"] --> SendUpdateSolo["PUT /api/v1/users/:id<br/>{ password, must_change_password: false }"] --> GoDash
    Step2 --> UserStep2Action{"Действие пользователя"}
    UserStep2Action -- "Клик Назад" --> Step1
    UserStep2Action -- "Ввод пароля + Сохранить и войти" --> ValidatePwd{"Чек-лист сложности:<br/>Длина, Заглавные, Цифры, Спецсимволы + Совпадение"}
    ValidatePwd -- "Ошибка" --> ShowPwdErr["Отображение ошибок в форме"] --> Step2
    ValidatePwd -- "Успех" --> SendUpdate["PUT /api/v1/users/:id<br/>{ username, password, is_username_locked: true, must_change_password: false }"]
    SendUpdate --> UpdateResult{"Ответ API"}
    UpdateResult -- "Ошибка (409/422)" --> ShowApiErr["Вывод ошибки в модальном окне"] --> Step2
    UpdateResult -- "200 OK" --> SetUser["authStore.user = updated"] --> CloseModal["Закрытие мастера"] --> GoDash["Редирект на /dashboard"]
```

### Б. Sequence-диаграмма взаимодействия UI, Store и Core

```mermaid
sequenceDiagram
    autonumber
    actor User as Пользователь
    participant UI as LoginView.vue (Wizard)
    participant Store as authStore
    participant API as ApiClient (/api/v1/users)
    participant Core as Backend UserService
    Note over User, Core: Вход пользователя (не root) при !is_username_locked || must_change_password
    UI->>UI: Открытие модального окна мастера
    alt Доступна смена логина (!is_username_locked)
        rect rgb(30, 41, 59)
            Note over User, UI: Шаг 1: Персонализация логина
            User->>UI: Ввод нового постоянного логина (или клик "Оставить текущий")
            alt Требуется также смена пароля
                User->>UI: Клик "Далее" (wizardNext)
                UI->>UI: Локальная валидация (длина 3-32, regex a-z0-9._-)
                UI->>UI: Переключение на wizardStep = 'password'
            else Смена пароля отключена
                User->>UI: Клик "Сохранить и войти"
                UI->>API: PUT /api/v1/users/:id { username: "new_login", is_username_locked: true }
            end
        end
    end
    alt Требуется смена пароля (must_change_password)
        rect rgb(30, 41, 59)
            Note over User, UI: Шаг 2: Установка постоянного пароля
            alt Пользователь хочет скорректировать логин (если был Шаг 1)
                User->>UI: Клик "Назад" (wizardBack)
                UI->>UI: Возврат на wizardStep = 'username'
                User->>UI: Корректировка логина и повторный клик "Далее"
            end
            User->>UI: Ввод нового пароля и подтверждения
            loop Динамическая проверка сложности
                UI->>UI: Проверка passwordRequirements
                UI->>User: Интерактивная подсветка чек-листа политики безопасности
            end
        end
    end
    %% Финальное сохранение
    Note over User, Core: Финальное сохранение учетных данных
    User->>UI: Клик "Сохранить и войти"
    UI->>API: PUT /api/v1/users/:id { username?, password?, is_username_locked: true, must_change_password: false }
    API->>Core: update_user (валидация логина + сложность пароля + Argon2id hash)
    alt Ошибка валидации или конфликт логина (409/422)
        Core-->>API: 409 Conflict / 422 Unprocessable Entity
        API-->>UI: Отображение сообщения об ошибке
    else Успешное сохранение
        Core->>Core: UPDATE users SET username = ..., password_hash = ..., is_username_locked = 1, must_change_password = 0
        Core-->>API: 200 OK (UserResponseDto)
        API-->>Store: authStore.user = updated
        API-->>UI: 200 OK
        UI->>UI: Закрытие модального окна
        UI->>UI: router.push('/dashboard')
    end
```

---

## 5. Тест-кейсы для верификации Frontend изменений

### ТК-1: Запоминание оператора и разделение хранилищ токена
* **Given**: Пользователь открывает экран логина `/login`.
* **When**: Пользователь вводит логин `operator1`, отмечает «Запомнить меня» и успешно входит в систему.
* **Then**: Токен записан в `localStorage.getItem('aether_token')`, логин сохранен в `localStorage.getItem('aether_remembered_operator')`. При следующем открытии страницы логин подставляется автоматически.
* **When 2**: Пользователь выходит и входит без галочки «Запомнить меня».
* **Then 2**: Токен записан только в `sessionStorage.getItem('aether_token')`, в `localStorage` токен отсутствует.

### ТК-2: Автоматический выход по таймауту неактивности
* **Given**: Пользователь авторизован, в политиках безопасности установлен `inactivity_timeout = 15`.
* **When**: Пользователь не совершает действий (мышь, клавиатура, скролл) в течение 15 минут.
* **Then**: `authStore` вызывает `logout()`, очищает хранилище и перенаправляет на `/login?reason=inactivity` с выводом сообщения «Сессия завершена из-за длительного отсутствия активности».

### ТК-3: Модальное окно «Забыли код?»
* **Given**: Пользователь находится на странице `/login`.
* **When**: Пользователь нажимает на ссылку «Забыли код?».
* **Then**: Открывается модальное окно `BaseModal` с контактами администратора и готовой CLI-командой для аварийного сброса пароля.

### ТК-4: Пошаговый мастер первичной настройки с разделением логина и пароля
* **Given**: Новый оператор создан с временным логином и флагом `must_change_password: true`.
* **When**: Пользователь входит в систему под временными учетными данными.
* **Then**:
  1. Открывается пошаговый мастер с индикатором (Шаг 1 из 2).
  2. На **Шаге 1** предлагается задать постоянный логин либо нажать «Оставить текущий». Переход происходит по кнопке «Далее» после валидации формата.
  3. На **Шаге 2** отображаются поля ввода нового пароля и подтверждения с интерактивным чеклистом требований сложности. Доступна кнопка «Назад» для возврата к шагу 1.
  4. Нажатие единой финальной кнопки **«Сохранить и войти»** атомарно сохраняет выбранный логин и пароль, сбрасывает флаг смены пароля и выполняет переход на `/dashboard`.

### ТК-5: Вход под системным пользователем root
* **Given**: Система инициализирована с дефолтным суперпользователем `root:root`.
* **When**: Выполняется вход под `root` / `root`.
* **Then**: Мастер первичной настройки не отображается, выполняется мгновенный переход на `/dashboard`. Логин `root` защищен от переименования и удаления.

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

---

## 7. Блок-схема: Иерархия ролей и доступность элементов управления во Frontend

```mermaid
graph TD
    UserLoaded(["Пользователь загружен в authStore"]) --> CalcLevel["Вычисление currentUserRoleLevel: Superuser = 4, Admin = 3, Operator = 2, Viewer = 1"]
    CalcLevel --> UsersPage["Страница /settings/users"]
    CalcLevel --> MatrixPage["Страница /settings/access-identity"]
    %% Управление пользователями
    UsersPage --> CheckCanManage{"canManageUsers?"}
    CheckCanManage -- "Нет" --> AddBtnDisabled["Кнопка 'Добавить пользователя' disabled"]
    CheckCanManage -- "Нет" --> TableActionsDisabled["Действия в таблице disabled"]
    CheckCanManage -- "Да" --> AddBtnEnabled["Кнопка 'Добавить пользователя' активна"]
    AddBtnEnabled --> FilterRoleOpts["Фильтрация createRoleOptions / editRoleOptions: Показывать роли <= currentUserRoleLevel"]
    %% Матрица прав доступа
    MatrixPage --> RenderMatrix["Отображение Permissions Matrix"]
    RenderMatrix --> CheckSuperCol["Колонка Superuser: всегда checked и disabled"]
    RenderMatrix --> CheckAdminCol{"isSuperuser?"}
    CheckAdminCol -- "Да" --> AdminColActive["Колонка Admin: редактируемая"]
    CheckAdminCol -- "Нет" --> AdminColDisabled["Колонка Admin: disabled"]
    RenderMatrix --> CheckSubCols{"currentUserRoleLevel >= 3?"}
    CheckSubCols -- "Да" --> SubColsActive["Колонки Operator и Viewer: редактируемые"]
    CheckSubCols -- "Нет" --> SubColsDisabled["Колонки Operator и Viewer: disabled (Readonly)"]
    MatrixPage --> CheckSavePerm{"canManageSecurity ИЛИ canManageRoles?"}
    CheckSavePerm -- "Нет" --> ApplyBtnDisabled["Кнопка 'Применить изменения' disabled"]
    CheckSavePerm -- "Да" --> ApplyBtnActive["Кнопка 'Применить изменения' активна"]
```

---

## 8. Блок-схема: Персональные настройки оператора, часовой пояс, формат времени и Searchable Select

```mermaid
graph TD
    Mount(["Монтирование UserProfileView.vue"]) --> LoadPrefs["Загрузка профиля и предпочтений:<br/>1. authStore.fetchUser()<br/>2. settingsApi.getUserPreferences()"]
    LoadPrefs --> ApplyState["Инициализация состояния Vue:<br/>• timezone (дефолт UTC / сохраненный)<br/>• time_format (24h_sec / 24h_min / 12h_sec / 12h_min / iso)<br/>• theme, locale, department, email, full_name"]
    ApplyState --> StartClock["Запуск таймера живых часов<br/>clockTimer = setInterval(updateClock, 1000)"]
    StartClock --> RenderClock["Форматирование часов:<br/>Intl.DateTimeFormat(locale, timezone, timeFormat)"]
    %% Взаимодействие с часовым поясом
    ApplyState --> TimezoneCard["Карточка Внешний вид и региональность<br/>BaseCard с overflow-visible"]
    TimezoneCard --> BaseSelectTz["Searchable BaseSelect"]
    BaseSelectTz --> ClickTzTrigger{"Клик по полю выбора пояса?"}
    ClickTzTrigger -- "Да" --> OpenDropdown["Открытие выпадающего меню z-index 100<br/>• Фокус на строке поиска<br/>• Генерация списка IANA зон<br/>• Расчет актуального смещения GMT"]
    OpenDropdown --> SearchInput["Ввод запроса в строку поиска<br/>(город, регион, IANA код или GMT)"]
    SearchInput --> FilterList["Мгновенная реактивная фильтрация списка<br/>filteredOptions"]
    FilterList --> SelectTzItem["Выбор часового пояса оператором"]
    SelectTzItem --> UpdateTz["1. timezone.value = selectedTz<br/>2. updateClock (мгновенный пересчет)<br/>3. Фоновое автосохранение: updateUserPreferences"]
    %% Кнопка автоопределения
    TimezoneCard --> ClickAutoDetect{"Клик Автоопределение?"}
    ClickAutoDetect -- "Да" --> ReadClientTz["Чтение Intl.resolvedOptions.timeZone"]
    ReadClientTz --> UpdateTz
    %% Взаимодействие с форматом времени
    TimezoneCard --> SelectTimeFmt["Выбор формата времени time_format:<br/>24h_sec / 24h_min / 12h_sec / 12h_min / iso"]
    SelectTimeFmt --> UpdateFmt["1. timeFormat.value = selectedFmt<br/>2. updateClock (мгновенный пересчет)<br/>3. Фоновое автосохранение: updateUserPreferences"]
    %% Полное сохранение профиля
    TimezoneCard --> ClickSaveAll{"Клик Сохранить изменения?"}
    ClickSaveAll -- "Да" --> SaveChain["Сквозное сохранение:<br/>1. usersApi.update (запись в таблицу users)<br/>2. settingsApi.updateUserPreferences (запись в KV store)<br/>3. authStore.fetchUser (обновление сессии Pinia)<br/>4. Анимация кнопки: savedNotice = true"]
```

### Диаграмма последовательности: Выбор и синхронизация региональных настроек (Timezone & Time Format)

```mermaid
sequenceDiagram
    autonumber
    actor Operator as Оператор
    participant UI as UserProfileView.vue
    participant Select as BaseSelect.vue (Searchable)
    participant API as ApiClient (/api/v1/settings)
    participant Core as Backend Axum (SettingsHandler)
    participant DB as SQLite DB (KV Store)

    Note over Operator, DB: 1. Открытие страницы и получение настроек
    Operator->>UI: Переход в /settings/profile
    UI->>API: GET /api/v1/settings/user-preferences
    API->>Core: get_user_preferences_handler()
    Core->>DB: KvStore::get("user:{id}:preferences")
    DB-->>Core: { timezone, time_format, theme, locale, ... }
    Core-->>API: 200 OK (UserPreferencesDto)
    API-->>UI: Применение предпочтений
    UI->>UI: updateClock() -> Live Clock в заголовке профиля

    Note over Operator, DB: 2. Поиск и выбор часового пояса внутри списка
    Operator->>Select: Клик по выпадающему списку часового пояса
    Select->>Select: isOpen = true, расчет смещений GMT для 430+ зон
    Operator->>Select: Ввод "Minsk" в строку поиска
    Select->>Select: Реактивная фильтрация -> Europe/Minsk (GMT+3)
    Operator->>Select: Клик по "Europe/Minsk (GMT+3)"
    Select-->>UI: emit("update:modelValue", "Europe/Minsk")
    UI->>UI: updateClock() (пересчет живых часов)
    UI->>API: PUT /api/v1/settings/user-preferences { timezone: "Europe/Minsk" }
    API->>Core: update_user_preferences_handler()
    Core->>DB: KvStore::set("user:{id}:preferences")
    DB-->>Core: Успешно
    Core-->>API: 200 OK

    Note over Operator, DB: 3. Смена формата отображения времени
    Operator->>UI: Выбор формата времени (например, "12h_sec")
    UI->>UI: updateClock() (мгновенное переключение на 12-часовой формат с AM/PM)
    UI->>API: PUT /api/v1/settings/user-preferences { time_format: "12h_sec" }
    API->>Core: update_user_preferences_handler()
    Core->>DB: KvStore::set("user:{id}:preferences")
    DB-->>Core: Успешно
    Core-->>API: 200 OK
```

---

## 9. Блок-схема: Жизненный цикл, оптимизация и сквозное отображение аватаров пользователей

```mermaid
graph TD
    %% Загрузка фото
    UploadStart(["Клик 'Загрузить фото'"]) --> ChooseFile["Выбор файла (.png, .jpg, .webp)"]
    ChooseFile --> ValidateFormat{"Формат image/*?"}
    ValidateFormat -- "Нет" --> ShowFmtErr["Ошибка: Неверный формат файла"]
    ValidateFormat -- "Да" --> ValidateSize{"Размер <= 2 MB?"}
    ValidateSize -- "Нет" --> ShowSizeErr["Ошибка: Превышен лимит размера"]
    ValidateSize -- "Да" --> CanvasCrop["HTML5 Canvas Processing:<br/>1. Центрирование и обрезка 1:1<br/>2. Масштабирование до 256x256 px<br/>3. Экспорт в JPEG DataURL (0.85 quality)"]
    CanvasCrop --> UpdateLocalUI["Мгновенное обновление UI:<br/>avatar.value = dataUrl<br/>authStore.avatar = dataUrl"]
    UpdateLocalUI --> PutPref["PUT /api/v1/settings/user-preferences<br/>{ avatar: dataUrl }"]
    PutPref --> ServerMerge["Backend Axum handler:<br/>1. Чтение текущих preferences из KV<br/>2. Слияние (merge) JSON-патча<br/>3. Запись в SQLite KvStore: user:{id}:preferences"]
    ServerMerge --> ShowSuccess["Статус: Фото профиля успешно обновлено"]

    %% Навигация и отображение
    NavEvent(["Переход на любую страницу приложения"]) --> CheckAuth["App.vue / AppHeader.vue"]
    CheckAuth --> FetchStore["authStore.fetchUser() / loadPreferences()"]
    FetchStore --> LoadAvatar{"authStore.avatar задан?"}
    %% Сквозные компоненты
    LoadAvatar -- "Да (Data URL)" --> RenderHeaderAvatar["AppHeader.vue:<br/>Отображение аватара rounded-xl + точка статуса"]
    LoadAvatar -- "Да (Data URL)" --> RenderDropdownAvatar["Dropdown профиля:<br/>Отображение аватара rounded-xl"]
    LoadAvatar -- "Да (Data URL)" --> RenderProfileAvatar["UserProfileView.vue:<br/>Отображение аватара rounded-2xl с неоновым контуром"]
    LoadAvatar -- "Да (Data URL)" --> RenderUsersTable["UsersManagementView.vue:<br/>Для строки текущего оператора: аватар rounded-xl"]
    LoadAvatar -- "Нет (null)" --> FallbackInitials["Fallback: Текстовые инициалы getUserInitials<br/>в цветном контейнере роли (rounded-xl)"]

    %% Сброс аватара
    ResetStart(["Клик 'Удалить фото'"]) --> ResetLocal["avatar.value = null<br/>authStore.avatar = null"]
    ResetLocal --> PutEmpty["PUT /api/v1/settings/user-preferences<br/>avatar: пустая строка"]
    PutEmpty --> ServerMerge
```

### Диаграмма последовательности: Загрузка аватара и сквозное отображение

```mermaid
sequenceDiagram
    autonumber
    actor User as Оператор
    participant Profile as UserProfileView.vue
    participant Canvas as HTML5 Canvas
    participant Store as authStore (Pinia)
    participant Header as AppHeader.vue
    participant UsersTable as UsersManagementView.vue
    participant API as ApiClient (/api/v1/settings)
    participant Backend as Axum SettingsHandler
    participant DB as SQLite KV Store

    Note over User, DB: 1. Загрузка, обрезка и сохранение фото профиля
    User->>Profile: Выбор файла изображения (до 2MB)
    Profile->>Canvas: Центрирование (minDim) и resize 256x256
    Canvas-->>Profile: Data URL сжатого JPEG изображения
    Profile->>Store: authStore.avatar = dataUrl
    Profile->>Header: Реактивное обновление аватарки в шапке (rounded-xl)
    Profile->>API: PUT /api/v1/settings/user-preferences (avatar = dataUrl)
    API->>Backend: update_user_preferences_handler()
    Backend->>DB: Чтение user preferences из KvStore
    DB-->>Backend: Текущие настройки пользователя
    Backend->>Backend: Merge: сохранение timezone и theme, обновление avatar
    Backend->>DB: Запись объединенных настроек в KvStore
    DB-->>Backend: OK
    Backend-->>API: 200 OK (UserPreferencesDto)
    API-->>Profile: Уведомление "Фото обновлено"

    Note over User, DB: 2. Переход на страницу "Управление пользователями"
    User->>UsersTable: Переход в раздел /settings/users
    UsersTable->>Store: Чтение authStore.avatar для текущей учетной записи
    UsersTable->>UsersTable: Рендеринг таблицы: для текущего юзера выводится аватарка rounded-xl
```

### Тест-кейсы для верификации аватаров
#### ТК-Аватар-1: Загрузка и персистентность аватара
* **Given**: Пользователь авторизован и находится в `/profile`.
* **When**: Пользователь загружает изображение размером до 2MB.
* **Then**: Картинка сжимается до 256x256 px, отображается в карточке профиля, шапке `AppHeader.vue` и сохраняется на бэкенде в KV-хранилище без затирания часового пояса и темы.

#### ТК-Аватар-2: Сохранение аватара при навигации
* **Given**: Аватар успешно загружен и сохранен.
* **When**: Пользователь переходит на страницу `/settings/users` или обновляет страницу.
* **Then**: В верхнем баре (`AppHeader`), выпадающем меню и строке текущего оператора в таблице пользователей отображается миниатюра аватара в стиле `rounded-xl`.

#### ТК-Аватар-3: Сброс аватара к инициалам
* **Given**: Пользователь имеет установленный аватар.
* **When**: Пользователь нажимает «Удалить фото».
* **Then**: Аватар очищается в Pinia store и на сервере, а во всех компонентах плавно возвращается отображение текстовых инициалов в цветных контейнерах ролей.

---

## 10. Блок-схема и диаграмма последовательности: Двухфакторная аутентификация (2FA / TOTP и Резервные коды)

### 10.1. Блок-схема: Архитектура сквозного потока 2FA на Frontend

```mermaid
graph TD
    %% Шаг авторизации
    subgraph LoginFlow ["1. Экран авторизации (LoginView.vue)"]
        LoginInput["Ввод логина и пароля"] --> SubmitAuth["POST /api/v1/auth/login"]
        SubmitAuth --> CheckChallenge{"requires_2fa == true?"}
        CheckChallenge -- "Нет (2FA выключен)" --> AuthSuccess["Успешный вход -> Редирект на /dashboard"]
        CheckChallenge -- "Да (2FA включен)" --> SwitchTo2FAUI["store.tempToken = temp_token<br/>UI переключается в режим 2FA Challenge"]
        SwitchTo2FAUI --> CodeMode{"Выбор метода ввода"}
        CodeMode -- "TOTP" --> Input6Digit["Поле ввода 6-значного кода<br/>(Автофокус, моноширинный шрифт)"]
        CodeMode -- "Backup Code" --> InputBackup["Поле ввода резервного кода<br/>(Формат: XXXX-XXXX)"]
        Input6Digit --> VerifySubmit["POST /api/v1/auth/2fa/verify-login<br/>{ temp_token, code, is_backup_code: false }"]
        InputBackup --> VerifyBackupSubmit["POST /api/v1/auth/2fa/verify-login<br/>{ temp_token, code, is_backup_code: true }"]
        VerifySubmit --> CheckVerifyResult{"Результат проверки"}
        VerifyBackupSubmit --> CheckVerifyResult
        CheckVerifyResult -- "Ошибка (401)" --> Show2FAError["Отображение ошибки в форме"]
        CheckVerifyResult -- "Успех (200)" --> SetTokens["Сохранение постоянного токена<br/>в localStorage / sessionStorage"] --> AuthSuccess
    end

    %% Настройка в профиле
    subgraph ProfileFlow ["2. Настройка 2FA в профиле (UserProfileView.vue)"]
        ClickSetup["Клик 'Настроить 2FA'"] --> ReqSetup["POST /api/v1/auth/2fa/setup"]
        ReqSetup --> OpenModal["Открытие мастера настройки BaseModal"]
        OpenModal --> Step1["Шаг 1: QR-код + Кнопка копирования Base32"]
        Step1 --> Step2["Шаг 2: Ввод 6-значного кода подтверждения"]
        Step2 --> ReqEnable["POST /api/v1/auth/2fa/enable<br/>{ secret, code, backup_codes }"]
        ReqEnable --> CheckEnable{"Код валиден?"}
        CheckEnable -- "Нет" --> ShowStepErr["Ошибка: Неверный код 2FA"] --> Step2
        CheckEnable -- "Да" --> Step3["Шаг 3: Сетка из 8 резервных кодов<br/>• Кнопка 'Скопировать'<br/>• Кнопка 'Скачать .txt'<br/>• Чекбокс подтверждения"]
        Step3 --> FinishSetup["Клик 'Завершить настройку'<br/>authStore.user.is_totp_enabled = true<br/>Бейдж 'Активен'"]
    end

    %% Отключение 2FA
    subgraph DisableFlow ["3. Отключение 2FA"]
        ClickDisable["Клик 'Отключить 2FA'"] --> CheckPolicy{"force_2fa активна?"}
        CheckPolicy -- "Да" --> BlockUI["Запрещено политикой безопасности"]
        CheckPolicy -- "Нет" --> DisableModal["Модальное окно с вводом пароля"]
        DisableModal --> ReqDisable["POST /api/v1/auth/2fa/disable<br/>{ password: '...' }"]
        ReqDisable --> DisableSuccess["2FA отключен<br/>is_totp_enabled = false"]
    end
```

### 10.2. Диаграмма последовательности: Полный жизненный цикл 2FA

```mermaid
sequenceDiagram
    autonumber
    actor User as Оператор
    participant LoginUI as LoginView.vue
    participant ProfileUI as UserProfileView.vue
    participant Store as authStore (Pinia)
    participant API as ApiClient (/api/v1/auth)
    participant Backend as Axum AuthHandler
    participant Totp as TOTP Engine (RFC 6238)
    participant DB as SQLite DB

    %% Сценарий 1: Настройка и включение 2FA
    Note over User, DB: Сценарий 1: Настройка и активация 2FA в профиле
    User->>ProfileUI: Клик "Настроить 2FA"
    ProfileUI->>API: POST /api/v1/auth/2fa/setup
    API->>Backend: setup_2fa_handler()
    Backend->>Totp: generate_totp_secret() + generate_backup_codes(8)
    Totp-->>Backend: Base32 Secret, QR DataURL PNG, Backup Codes
    Backend-->>API: 200 OK { secret, qr_code_url, otpauth_url, backup_codes }
    API-->>ProfileUI: Отображение Шага 1 (QR-код и Base32 ключ)
    User->>ProfileUI: Сканирование QR в Authenticator -> Шаг 2 -> Ввод 6-значного кода
    ProfileUI->>API: POST /api/v1/auth/2fa/enable { secret, code, backup_codes }
    API->>Backend: enable_2fa_handler()
    Backend->>Totp: verify_totp_code(secret, code)
    Totp-->>Backend: Код валиден (true)
    Backend->>Totp: hash_and_serialize_backup_codes(backup_codes)
    Totp-->>Backend: JSON-массив Argon2id хэшей
    Backend->>DB: UPDATE users SET is_totp_enabled=1, totp_secret=..., totp_backup_codes=...
    DB-->>Backend: OK
    Backend-->>API: 200 OK { success: true }
    API-->>ProfileUI: Переход на Шаг 3 (Отображение 8 кодов, копирование, скачивание)
    User->>ProfileUI: Скачивание .txt + Чекбокс подтверждения + "Завершить"
    ProfileUI->>Store: user.is_totp_enabled = true

    %% Сценарий 2: Авторизация с 2FA Challenge
    Note over User, DB: Сценарий 2: Вход с двухфакторной аутентификацией (TOTP)
    User->>LoginUI: Ввод operatorId и accessCode
    LoginUI->>Store: login(operatorId, accessCode)
    Store->>API: POST /api/v1/auth/login
    API->>Backend: login_handler() -> Проверка пароля -> is_totp_enabled == true
    Backend->>Backend: Выпуск temp_token (роль: 2fa_pending, TTL: 300 сек)
    Backend-->>API: 200 OK { requires_2fa: true, temp_token: "..." }
    API-->>Store: store.tempToken = temp_token, store.requires2fa = true
    Store-->>LoginUI: is2faStep = true (Переключение интерфейса на ввод 2FA)
    User->>LoginUI: Ввод 6 цифр из приложения
    LoginUI->>Store: verify2faLogin(code)
    Store->>API: POST /api/v1/auth/2fa/verify-login { temp_token, code }
    API->>Backend: verify_2fa_login_handler()
    Backend->>Backend: Валидация temp_token (claims: 2fa_pending)
    Backend->>DB: Чтение totp_secret пользователя
    DB-->>Backend: totp_secret
    Backend->>Totp: verify_totp_code(secret, code)
    Totp-->>Backend: Код верен (true)
    Backend->>Backend: Выпуск полноценного JWT токена
    Backend-->>API: 200 OK { token, user }
    API-->>Store: api.setToken(token), user = user, requires2fa = false
    Store-->>LoginUI: Успешный вход -> Редирект на /dashboard

    %% Сценарий 3: Вход по одноразовому Backup коду
    Note over User, DB: Сценарий 3: Вход по резервному коду при утере телефона
    User->>LoginUI: Клик "Войти с помощью резервного кода" -> Ввод XXXX-XXXX
    LoginUI->>Store: verify2faLogin(code, isBackupCode=true)
    Store->>API: POST /api/v1/auth/2fa/verify-login { temp_token, code, is_backup_code: true }
    API->>Backend: verify_2fa_login_handler()
    Backend->>DB: Чтение totp_backup_codes (JSON Argon2id хэшей)
    DB-->>Backend: Хэши кодов
    Backend->>Totp: verify_and_consume_backup_code(hashes, code)
    Totp-->>Backend: Хэш найден, возвращен обновленный JSON без использованного кода
    Backend->>DB: UPDATE users SET totp_backup_codes = updated_json
    DB-->>Backend: OK
    Backend-->>API: 200 OK { token, user }
    API-->>Store: Сохранение токена
    Store-->>LoginUI: Успешный вход -> Редирект на /dashboard
```

### 10.3. Тест-кейсы для верификации 2FA на Frontend

#### ТК-2FA-1: Двухэтапный вход (2FA Challenge)
* **Given**: Для оператора включена 2FA (`is_totp_enabled = true`).
* **When**: Оператор вводит логин и пароль на `/login`.
* **Then**: Форма переключается на экран ввода 2FA, выводится поле для 6-значного кода с автофокусом и кнопка переключения на резервный код. Рабочий токен до подтверждения 2FA не выдается.

#### ТК-2FA-2: Вход по одноразовому резервному коду
* **Given**: Оператор находится на шаге 2FA Challenge.
* **When**: Оператор выбирает «Войти с помощью резервного кода» и вводит валидный сохраненный код `ABCD-EFGH`.
* **Then**: Сервер подтверждает вход, выпускает постоянный токен, удаляет данный код из доступных (повторный ввод этого же кода приводит к ошибке 401).

#### ТК-2FA-3: Полный цикл активации 2FA в профиле
* **Given**: Пользователь авторизован и находится в `/profile`, 2FA отключен.
* **When**: Пользователь нажимает «Настроить 2FA», сканирует QR-код, вводит 6-значный код из приложения, скачивает файл `aethercore-2fa-backup-codes-*.txt` и подтверждает чекбокс.
* **Then**: Карточка 2FA меняет статус на зеленый бейдж «Активен», появляются кнопки «Перевыпустить коды» и «Отключить 2FA».

#### ТК-2FA-4: Отключение 2FA
* **Given**: 2FA активен, системная политика `force_2fa` выключена.
* **When**: Пользователь нажимает «Отключить 2FA», вводит текущий пароль в модальном окне и подтверждает действие.
* **Then**: Статус 2FA в профиле и на сервере сбрасывается в «Отключен», секрет и бэкап-коды удаляются из БД.

### 10.4. Механизм гибкого управления и принудительного MFA (MFA Scope & Per-User Override)

В системе реализован трехуровневый сквозной механизм управления требованиями двухфакторной аутентификации:

```mermaid
graph TD
    subgraph L1["1. Уровень глобальных политик (Access & Identity)"]
        SCOPE["Выбор MFA Scope<br/>(disabled / admins_only / all)"]
        PARS["Параметры безопасности<br/>- Доверие устройств (0-90 дн)<br/>- Льготный период (0-30 дн)<br/>- Бэкап-коды (8-16 шт)"]
    end

    subgraph L2["2. Уровень пользователя (Users Management)"]
        USER_FLAG{"User force_2fa<br/>(Персональный флаг)"}
        USER_ENFORCE["Enforced (Обязательно)"]
        USER_EXEMPT["Exempt (Исключение)"]
        USER_AUTO["Auto / Default (По политике)"]
    end

    subgraph L3["3. Клиентский роутинг и шлюз безопасности (Frontend)"]
        STORE["authStore.is2faRequiredForCurrentUser"]
        GUARD{"Router Navigation Guard<br/>(isEnforced2fa && !user.is_totp_enabled)"}
        BLOCKED["Блокирующий редирект на<br/>/settings/profile?setup_2fa=true"]
        MODAL["Модальное окно TOTP мастера<br/>(Запрет закрытия и обхода)"]
        LOGOUT_BTN["Кнопка 'Выйти из системы'"]
        ACCESS_GRANTED["Доступ открыт к Dashboard и модулям"]
    end

    SCOPE --> USER_AUTO
    USER_FLAG -->|Some true| USER_ENFORCE
    USER_FLAG -->|Some false| USER_EXEMPT
    USER_FLAG -->|None| USER_AUTO

    USER_ENFORCE -->|Требовать 2FA| STORE
    USER_EXEMPT -->|Освобожден от 2FA| STORE
    USER_AUTO -->|Сверка с MFA Scope| STORE

    STORE --> GUARD
    GUARD -->|Да, 2FA не настроен| BLOCKED
    BLOCKED --> MODAL
    MODAL --> LOGOUT_BTN
    GUARD -->|Нет, 2FA настроен / не требуется| ACCESS_GRANTED
```

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Администратор
    actor Operator as Оператор
    participant UI as Vue Frontend (Router / Guard)
    participant Store as Pinia (authStore)
    participant API as AetherCore REST API
    participant DB as SQLite DB

    %% Сценарий 1: Настройка глобальной политики MFA Scope
    Note over Admin, DB: 1. Настройка области действия MFA (MFA Scope)
    Admin->>UI: Переход в раздел "Доступ и авторизация"
    UI->>API: GET /api/v1/settings/security
    API-->>UI: SecurityPoliciesDto { mfa_scope: "admins_only", ... }
    Admin->>UI: Выбор режима "Только администраторы" и сохранение
    UI->>API: PUT /api/v1/settings/security { mfa_scope: "admins_only", force_2fa: true }
    API->>DB: Сохранение security_policies в kv_store
    DB-->>API: OK
    API-->>UI: 200 OK

    %% Сценарий 2: Персональное переопределение требования для оператора
    Note over Admin, DB: 2. Персональное переопределение для конкретного пользователя
    Admin->>UI: Раздел "Управление пользователями" -> Редактирование оператора
    Admin->>UI: Селектор 2FA -> "Обязательно требовать 2FA" (Enforced)
    UI->>API: PUT /api/v1/users/:id { force_2fa: true }
    API->>DB: UPDATE users SET force_2fa = 1 WHERE id = :id
    DB-->>API: OK
    API-->>UI: 200 OK

    %% Сценарий 3: Блокирующий вход и Navigation Guard
    Note over Operator, DB: 3. Вход пользователя с обязательным 2FA
    Operator->>UI: Вход по логину и паролю
    UI->>Store: login(username, password)
    Store->>API: POST /api/v1/auth/login
    API-->>Store: 200 OK { token, user: { is_totp_enabled: false, force_2fa: true } }
    Store->>Store: is2faRequiredForCurrentUser = true
    UI->>UI: Router Navigation Guard проверяет /dashboard
    UI-->>Operator: Блокирующий редирект на /settings/profile?setup_2fa=true
    UI-->>Operator: Открытие модального окна настройки TOTP (ESC и клик вне окна заблокированы)
    Operator->>UI: Завершение привязки 2FA (QR -> OTP -> Backup Codes)
    UI->>API: POST /api/v1/auth/2fa/enable
    API-->>UI: 200 OK { success: true }
    UI->>Store: user.is_totp_enabled = true
    UI-->>Operator: Разблокировка навигации и переход на /dashboard
```

### 10.5. Расширенные тест-кейсы для верификации (Given-When-Then)

#### ТК-MFA-1: Обязательный MFA для администраторов (`admins_only`)
* **Given**: В политиках безопасности выставлен `mfa_scope = "admins_only"`. Учетная запись `admin` (роль Admin) не имеет настроенного 2FA (`is_totp_enabled = false`, `force_2fa = null`).
* **When**: Пользователь `admin` входит в систему и пытается перейти на любой рабочий экран (`/dashboard`, `/modules`, `/users`).
* **Then**: Глобальный Navigation Guard роутера блокирует переход, перенаправляет на `/settings/profile?setup_2fa=true` и открывает блокирующее модальное окно привязки аутентификатора. В карточке профиля кнопка «Отключить 2FA» неактивна с подсказкой о политике безопасности.

#### ТК-MFA-2: Персональное исключение оператора (`force_2fa = false`)
* **Given**: В системе действует строгий режим `mfa_scope = "all"`, но конкретному служебному оператору администратором выставлено исключение `force_2fa = false` (Exempt).
* **When**: Оператор авторизуется в веб-интерфейсе.
* **Then**: Роутер не блокирует доступ к дашборду, в таблице пользователей у оператора отображается серый бейдж «Исключение», а в профиле статус 2FA отображается как «Опционально».

#### ТК-MFA-3: Персональное принуждение оператора (`force_2fa = true`)
* **Given**: Глобальный режим `mfa_scope = "disabled"` (2FA доброволен для всех). Для оператора администратором установлен персональный флаг `force_2fa = true` (Enforced).
* **When**: Оператор входит в аккаунт без привязанного 2FA.
* **Then**: `authStore.is2faRequiredForCurrentUser` возвращает `true`, оператор немедленно направляется на обязательную первичную привязку 2FA до получения доступа к рабочим функциям.

#### ТК-MFA-4: Динамический размер пула резервных кодов (8–16 шт)
* **Given**: Администратор в `AccessIdentityView.vue` настроил параметр `mfa_backup_codes_count = 12`.
* **When**: Любой пользователь запускает настройку 2FA или перевыпуск резервных кодов в профиле.
* **Then**: Сервер генерирует ровно 12 одноразовых кодов восстановления, и в интерфейсе модального окна и скачиваемом `.txt` файле отображаются все 12 кодов.





