# Архитектура: Политики безопасности, Квоты и Ролевая модель (Security & RBAC)

## 1. Блок-схема и Диаграммы взаимодействия (Mermaid)

### А. Блок-схема иерархии ролей и валидации операций над пользователями (Graph TD)

```mermaid
graph TD
    Start(["Запрос: Create / Update / Delete User"]) --> CheckAuth{"Пользователь авторизован?"}
    CheckAuth -- "Нет" --> Err401["401 Unauthorized"]
    CheckAuth -- "Да" --> CheckManagePerm{"Есть право users.manage или Superuser?"}
    CheckManagePerm -- "Нет" --> Err403Perm["403 Forbidden: Требуется users.manage"]
    CheckManagePerm -- "Да" --> CalcLevels["Расчет рангов: Superuser: 4, Admin: 3, Operator: 2, Viewer: 1"]
    CalcLevels --> CheckOpType{"Тип операции"}
    %% Создание пользователя
    CheckOpType -- "Create User" --> CheckCreateLevel{"target_level <= caller_level?"}
    CheckCreateLevel -- "Нет" --> Err403Escalate["403 Forbidden: Запрещено создавать пользователей выше своего ранга"]
    CheckCreateLevel -- "Да" --> CheckSuperQuota{"Создается Superuser?"}
    CheckSuperQuota -- "Да: Лимит 4" --> CheckCount4{"Текущее число Superusers < 4?"}
    CheckCount4 -- "Нет" --> Err400Quota["400 Bad Request: Превышен лимит 4 суперпользователя"]
    CheckCount4 -- "Да" --> AllowOp["Выполнить операцию и зафиксировать в Audit Log"]
    CheckSuperQuota -- "Нет" --> AllowOp
    %% Редактирование пользователя
    CheckOpType -- "Update User" --> CheckTargetLevel{"target_user_level <= caller_level или Self-update?"}
    CheckTargetLevel -- "Нет" --> Err403EditHigher["403 Forbidden: Запрещено изменять пользователя выше своего ранга"]
    CheckTargetLevel -- "Да" --> CheckNewRoleLevel{"new_role_level <= caller_level?"}
    CheckNewRoleLevel -- "Нет" --> Err403AssignHigher["403 Forbidden: Запрещено повышать до ранга выше своего"]
    CheckNewRoleLevel -- "Да" --> CheckLockRoot{"Попытка блокировки Superuser?"}
    CheckLockRoot -- "Да" --> Err400Lock["400 Bad Request: Запрещено блокировать суперпользователей"]
    CheckLockRoot -- "Нет" --> CheckDemoteLast{"Понижение последнего Superuser?"}
    CheckDemoteLast -- "Да" --> Err400Demote["400 Bad Request: Нельзя понизить единственного Superuser"]
    CheckDemoteLast -- "Нет" --> AllowOp
    %% Удаление пользователя
    CheckOpType -- "Delete User" --> CheckSelfDel{"Удаление самого себя?"}
    CheckSelfDel -- "Да" --> Err400Self["400 Bad Request: Нельзя удалить свой аккаунт"]
    CheckSelfDel -- "Нет" --> CheckDeleteLevel{"caller is Superuser ИЛИ target_level < caller_level?"}
    CheckDeleteLevel -- "Нет" --> Err403DeleteHigher["403 Forbidden: Запрещено удалять пользователя равного или высшего ранга"]
    CheckDeleteLevel -- "Да" --> CheckDeleteLastSuper{"Удаление последнего Superuser?"}
    CheckDeleteLastSuper -- "Да" --> Err400Last["400 Bad Request: Нельзя удалить последнего суперпользователя"]
    CheckDeleteLastSuper -- "Нет" --> AllowOp
```

### Б. Диаграмма последовательности: Авторизация и раздельный онбординг (Sequence Diagram)

```mermaid
sequenceDiagram
    autonumber
    actor User as Пользователь
    participant UI as LoginView.vue
    participant API as authApi (/api/v1/auth)
    participant Core as UserService & Argon2id
    participant DB as SQLite DB

    User->>UI: Ввод логина и пароля
    UI->>API: POST /api/v1/auth/login
    API->>Core: authenticate(username, password)
    Core->>DB: Проверка активности (is_active) и хэша
    DB-->>Core: User { is_username_locked, must_change_password, roles, permissions }
    Core-->>API: Ok(User + JWT Claims с ролями и правами)
    API-->>UI: 200 OK { token, user }

    alt !is_username_locked && username != "root" && must_change_password
        UI->>UI: Открытие 2-шагового мастера (Шаг 1: Логин -> Шаг 2: Пароль)
        User->>UI: Шаг 1: ввод нового логина -> "Далее"
        User->>UI: Шаг 2: ввод постоянного пароля -> "Сохранить и войти"
        UI->>API: PUT /api/v1/users/:id { username, password, is_username_locked: true, must_change_password: false }
        API->>Core: update_user (смена логина, Argon2id, фиксация)
        Core->>DB: UPDATE users SET username = ..., password_hash = ..., is_username_locked = 1, must_change_password = 0
        DB-->>Core: 200 OK
        Core-->>API: UserUpdated
        API-->>UI: 200 OK -> Переход на /dashboard
    else !is_username_locked && username != "root" && !must_change_password
        UI->>UI: Открытие 1-шагового окна смены логина
        User->>UI: Ввод нового логина -> "Сохранить и войти"
        UI->>API: PUT /api/v1/users/:id { username, is_username_locked: true }
        API->>Core: update_user (смена логина и фиксация)
        Core->>DB: UPDATE users SET username = ..., is_username_locked = 1
        DB-->>Core: 200 OK -> Переход на /dashboard
    else is_username_locked && must_change_password
        UI->>UI: Открытие окна обязательной смены пароля
        User->>UI: Ввод нового пароля -> "Сохранить пароль"
        UI->>API: PUT /api/v1/users/:id { password, must_change_password: false }
        API->>Core: update_user (Argon2id, сброс must_change_password)
        Core->>DB: UPDATE users SET password_hash = ..., must_change_password = 0
        DB-->>Core: 200 OK -> Переход на /dashboard
    else Обычный вход без онбординга
        UI->>UI: Прямой переход на /dashboard
    end
```

### В. Блок-схема политики: Авторизация веб-интерфейса (web_ui_auth)

```mermaid
graph TD
    Start(["Пользователь открывает страницу / отправляет запрос"]) --> CheckConfig{"web_ui_auth включена?"}
    %% Режим без авторизации
    CheckConfig -- "Нет: web_ui_auth = false" --> CheckTokenNoAuth{"Есть JWT токен?"}
    CheckTokenNoAuth -- "Нет" --> AutoSuper["Автоматический вход с правами Superuser (Anonymous Admin)"]
    CheckTokenNoAuth -- "Да" --> VerifyTokenNoAuth["Валидация токена"]
    VerifyTokenNoAuth --> AutoSuper
    AutoSuper --> AllowAccess["Разрешить доступ к Dashboard / API"]

    %% Режим с обязательной авторизацией
    CheckConfig -- "Да: web_ui_auth = true" --> CheckTokenAuth{"Есть валидный JWT токен?"}
    CheckTokenAuth -- "Да" --> CheckRoute{"Маршрут /login?"}
    CheckRoute -- "Да" --> GoDash["Перенаправление на /dashboard"]
    CheckRoute -- "Нет" --> AllowAccess
    CheckTokenAuth -- "Нет" --> RouteLogin{"Публичный маршрут /login?"}
    RouteLogin -- "Да" --> ShowLoginForm["Отобразить форму входа"]
    RouteLogin -- "Нет" --> RedirectLogin["401 / Редирект на /login"]

    %% Событие изменения политики
    ChangePolicy(["Администратор переключает тумблер web_ui_auth"]) --> IsEnabling{"Опция включается?"}
    IsEnabling -- "Да: false -> true" --> DropSession["Сброс текущей сессии authStore.logout"]
    DropSession --> RedirectLogin
    IsEnabling -- "Нет: true -> false" --> EnableAnon["Снятие требования входа и переход на Dashboard"]
```

### Г. Блок-схема разграничения матрицы прав доступа (Permissions Matrix)

```mermaid
graph TD
    MatrixReq(["Запрос: PUT /api/v1/settings/permissions"]) --> CheckMatrixAuth{"Авторизован?"}
    CheckMatrixAuth -- "Нет" --> Err401M["401 Unauthorized"]
    CheckMatrixAuth -- "Да" --> CheckRolesPerm{"Есть право access.manage или Superuser?"}
    CheckRolesPerm -- "Нет" --> Err403PermM["403 Forbidden: Требуется access.manage"]
    CheckRolesPerm -- "Да" --> CheckRoleRank{"Ранг вызывающего"}

    CheckRoleRank -- "Superuser: Ранг 4" --> SaveMatrix["Сохранить матрицу прав в таблице 'role_permissions' и Audit Log"]
    CheckRoleRank -- "Admin: Ранг 3" --> CheckAdminModifies{"Модифицируются только права Operator и Viewer?"}
    CheckAdminModifies -- "Да" --> SaveMatrix
    CheckAdminModifies -- "Нет" --> Err403Admin["403 Forbidden: Администратор не может менять права Admin / Superuser"]
    CheckRoleRank -- "Operator / Viewer: Ранг <= 2" --> Err403Low["403 Forbidden: Меньший ранг не имеет доступа на запись матрицы"]
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

### ТК-8: Иерархия ролей и разграничение доступа в матрице RBAC
* **Given**: Авторизован пользователь с ролью меньшего статуса (например, `operator` уровень 2 или `viewer` уровень 1).
* **When**: Пользователь пытается создать/отредактировать учетную запись со статусом выше своего или изменить матрицу прав ролей.
* **Then**:
  1. В интерфейсе создания/редактирования в выпадающих списках отображаются только роли с уровнем $\le$ текущего пользователя.
  2. В таблице кнопка «Добавить пользователя» и действия (редактирование, блокировка, удаление) отключены (`disabled`) при отсутствии права `users.manage`.
  3. В матрице прав (`/settings/access-identity`) колонки высших ролей заблокированы для редактирования, а кнопка «Применить изменения» отключена для пользователей без права `access.manage`.
  4. На уровне API ядро возвращает `403 Forbidden` при любых попытках создания пользователя выше своего ранга (`target_level > caller_level`) или модификации матрицы прав не-администратором/не-суперпользователем.
