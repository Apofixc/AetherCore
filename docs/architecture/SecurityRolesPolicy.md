# Архитектура: Политики безопасности, Квоты и Ролевая модель (Security & RBAC)

## 1. Блок-схема и Диаграммы взаимодействия (Mermaid)

### А. Блок-схема валидации операций над пользователями и ролями (Graph TD)

```mermaid
graph TD
    Start([Запрос на операцию: Create / Update / Delete]) --> CheckAuth{Пользователь авторизован?}
    CheckAuth -- Нет --> Err401[401 Unauthorized]
    CheckAuth -- Да --> CheckRoleOp{Тип операции}

    %% Создание / Повышение до Superuser
    CheckRoleOp -- Назначение роли Superuser --> CheckCallerSuper{Вызывающий оператор - Superuser?}
    CheckCallerSuper -- Нет --> Err403Super[403 Forbidden: Только Superuser может назначать эту роль]
    CheckCallerSuper -- Да --> CheckCount4{Текущее число Superusers < 4?}
    CheckCount4 -- Нет --> Err400Quota[400 Bad Request: Превышен лимит 4 суперпользователя]
    CheckCount4 -- Да --> AllowOp[Выполнить операцию и зафиксировать в Audit Log]

    %% Блокировка
    CheckRoleOp -- Блокировка is_active = false --> CheckTargetSuper{Целевой пользователь Superuser?}
    CheckTargetSuper -- Да --> Err400Lock[400 Bad Request: Запрещено блокировать суперпользователей]
    CheckTargetSuper -- Нет --> AllowOp

    %% Удаление / Понижение Superuser
    CheckRoleOp -- Удаление или Понижение Superuser --> CheckCount1{Число Superusers > 1?}
    CheckCount1 -- Нет --> Err400Last[400 Bad Request: Нельзя удалить или понизить последнего суперпользователя]
    CheckCount1 -- Да --> CheckCallerPerm{Вызывающий Superuser?}
    CheckCallerPerm -- Нет --> Err403Target[403 Forbidden: Запрещено изменять суперпользователя]
    CheckCallerPerm -- Да --> AllowOp

    %% Обычные операции
    CheckRoleOp -- Обычный сброс пароля / редактирование --> CheckTargetAdmin{Цель Superuser, а вызывающий нет?}
    CheckTargetAdmin -- Да --> Err403Target
    CheckTargetAdmin -- Нет --> AllowOp
```

### Б. Диаграмма последовательности: Вход с обязательной сменой пароля (Sequence Diagram)

```mermaid
sequenceDiagram
    autonumber
    actor User as Пользователь / Оператор
    participant UI as LoginView.vue
    participant API as authApi (/api/v1/auth)
    participant Core as UserService & Argon2id
    participant DB as SQLite DB

    User->>UI: Ввод логина и временного пароля
    UI->>API: POST /api/v1/auth/login
    API->>Core: authenticate(username, password)
    Core->>DB: Проверка активности (is_active) и хэша
    DB-->>Core: User { must_change_password: true, ... }
    Core-->>API: Ok(User + JWT Token)
    API-->>UI: 200 OK { token, user: { must_change_password: true } }

    alt must_change_password == true
        UI->>UI: Блокировка перехода на Dashboard
        UI->>User: Показ модального окна "Обязательная смена пароля"
        User->>UI: Ввод нового сложного пароля
        UI->>API: PUT /api/v1/users/:id { password: "НовыйПароль" }
        API->>Core: update_user (хэширование Argon2id + must_change_password = false)
        Core->>DB: UPDATE users SET password_hash = ..., must_change_password = 0
        DB-->>Core: 200 OK
        Core-->>API: UserUpdated
        API-->>UI: 200 OK
        UI->>UI: Снятие блокировки и переход на /dashboard
    else must_change_password == false
        UI->>UI: Прямой переход на /dashboard
    end
```

### В. Блок-схема политики: Авторизация веб-интерфейса (web_ui_auth)

```mermaid
graph TD
    Start([Пользователь открывает страницу / отправляет запрос]) --> CheckConfig{web_ui_auth включена?}
    
    %% Режим без авторизации
    CheckConfig -- Нет: web_ui_auth = false --> CheckTokenNoAuth{Есть JWT токен?}
    CheckTokenNoAuth -- Нет --> AutoSuper[Автоматический вход с правами Superuser<br/>Anonymous Admin]
    CheckTokenNoAuth -- Да --> VerifyTokenNoAuth[Валидация токена]
    VerifyTokenNoAuth --> AutoSuper
    AutoSuper --> AllowAccess[Разрешить доступ к Dashboard / API]

    %% Режим с обязательной авторизацией
    CheckConfig -- Да: web_ui_auth = true --> CheckTokenAuth{Есть валидный JWT токен?}
    CheckTokenAuth -- Да --> CheckRoute{Маршрут /login?}
    CheckRoute -- Да --> GoDash[Перенаправление на /dashboard]
    CheckRoute -- Нет --> AllowAccess
    CheckTokenAuth -- Нет --> RouteLogin{Публичный маршрут /login?}
    RouteLogin -- Да --> ShowLoginForm[Отобразить форму входа]
    RouteLogin -- Нет --> RedirectLogin[401 / Редирект на /login]

    %% Событие изменения политики
    ChangePolicy([Администратор переключает тумблер web_ui_auth]) --> IsEnabling{Опция включается?}
    IsEnabling -- Да: false -> true --> DropSession[Сброс текущей сессии authStore.logout]
    DropSession --> RedirectLogin
    IsEnabling -- Нет: true -> false --> EnableAnon[Снятие требования входа и переход на Dashboard]
```

---

## 2. Тест-кейсы для верификации (Given-When-Then)

### ТК-1: Запрет создания 5-го суперпользователя
* **Given**: В системе уже зарегистрировано 4 пользователя с ролью `superuser`.
* **When**: Действующий суперпользователь пытается создать 5-го суперпользователя.
* **Then**: Бэкенд возвращает ошибку `400 Bad Request` с сообщением о превышении лимита (максимум 4). В UI опция `Superuser` заблокирована.

### ТК-2: Запрет удаления и понижения единственного суперпользователя
* **Given**: В системе остался ровно 1 активный суперпользователь (`root`).
* **When**: Выполняется запрос на удаление `DELETE /api/v1/users/:id` или попытка сменить его роль на `admin`.
* **Then**: Операция отклоняется с ошибкой `400 Bad Request` ("Нельзя удалить/понизить последнего суперпользователя").

### ТК-3: Запрет блокировки суперпользователей
* **Given**: В таблице операторов выбран пользователь с ролью `superuser`.
* **When**: Выполняется попытка отправить `is_active: false`.
* **Then**: Кнопка блокировки в интерфейсе неактивна (`disabled`), а прямой API-запрос возвращает ошибку `400 Bad Request`.

### ТК-4: Защита от эскалации прав обычным администратором
* **Given**: Авторизован пользователь с ролью `admin` (не `superuser`).
* **When**: Администратор пытается отредактировать пользователя с ролью `superuser` или назначить кому-либо роль `superuser`.
* **Then**: Запрос отклоняется с кодом `403 Forbidden`.

### ТК-5: Раздельная первичная настройка аккаунта и обязательная смена пароля
* **Given**: Администратор создал пользователя с незафиксированным логином (`is_username_locked: false`).
* **When**: Пользователь успешно аутентифицируется с временными учетными данными.
* **Then**: Доступность шагов онбординга определяется строго независимыми флагами:
  1. **Смена логина (`is_username_locked: false`)**: Доступна однократная персонализация логина на Шаге 1. После первого успешного сохранения выставляется `is_username_locked: true`, и дальнейшие попытки сменить логин блокируются на уровне ядра.
  2. **Смена пароля (`must_change_password: true`)**: Требуется обязательная установка постоянного пароля с проверкой сложности.
  3. **Независимость**: Если `must_change_password: false`, пользователь настраивает только логин и сразу входит в систему. Если `must_change_password: true` для старого пользователя (`is_username_locked: true`), окно смены логина не показывается — открывается только форма смены пароля. Для `root` онбординг всегда отключен.

### ТК-6: Переключение политики «Авторизация веб-интерфейса» (web_ui_auth)
* **Given**: Авторизация веб-интерфейса отключена (`web_ui_auth: false`), работа ведется в режиме анонимного суперпользователя.
* **When**: Администратор включает тумблер `web_ui_auth: true` и нажимает «Применить изменения».
* **Then**: Сессия немедленно сбрасывается (`authStore.logout()`), пользователя выкидывает на страницу входа `/login`, а бэкенд начинает строго требовать JWT токен для всех защищенных API-маршрутов.

### ТК-7: Ограничение прав администратора при редактировании пользователей
* **Given**: Администратор открывает модальное окно редактирования пользователя.
* **When**: Отображается форма редактирования.
* **Then**: Администратору доступны исключительно управление ролью (`role`) и процедура сброса пароля (`password`). Поля логина, ФИО и email защищены от редактирования администратором.
