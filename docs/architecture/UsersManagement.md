# Архитектура: UsersManagementView.vue

## 1. Обзор архитектуры компонента

Компонент `UsersManagementView.vue` отвечает за полный жизненный цикл управления пользователями (операторами) платформы AetherCore:
* Отображение списка пользователей, их ролей, статусов активности и онлайн-присутствия.
* Динамическое получение системных политик безопасности (`security_policies`) и вычисление требований к паролям.
* Валидация и создание новых пользователей (`POST /api/v1/users`) с обработкой ошибок бэкенда прямо в модальном окне.
* Редактирование ролей и сброс паролей (`PUT /api/v1/users/:id`).
* Блокировка / разблокировка учетных записей (`PUT /api/v1/users/:id`).
* Одиночное и массовое удаление пользователей (`DELETE /api/v1/users/:id`).

---

## 2. Блок-схема и диаграмма последовательности (Mermaid)

### 2.1. Сквозной поток данных компонента

```mermaid
flowchart TD
    Start([Открытие UsersManagementView]) --> Mount[onMounted: loadUsers]
    Mount --> ParallelFetch["Promise.allSettled:\n1. usersApi.list()\n2. settingsApi.getSecurityPolicies()"]
    
    ParallelFetch --> SetState["Сохранение в operators & securityPolicies"]
    SetState --> CalcHints["Вычисление dynamic passwordHintText на основе SecurityPolicies"]
    
    CalcHints --> UserAction{"Действие Администратора"}
    
    UserAction -->|Клик 'Добавить оператора'| OpenAddModal["openAddModal: сброс формы и formError = null\nОтображение требований к паролю"]
    OpenAddModal --> SubmitAdd["handleCreateUser (submit)"]
    SubmitAdd --> ValidateForm{"Валидация формы?"}
    ValidateForm -->|Пустой логин| ShowLocalErr["formError = 'Имя пользователя обязательно'"]
    ValidateForm -->|Корректно| PostApi["usersApi.create(CreateUserDto)"]
    
    PostApi --> ApiResponse{"Ответ бэкенда"}
    ApiResponse -->|200 OK / 201 Created| SuccessAdd["Добавление в operators.value\nСброс формы\nЗакрытие модального окна"]
    ApiResponse -->|400 / 409 / 422 Ошибка| ErrorAdd["formError = err.message (текст ошибки бэкенда)\nМодальное окно остается открытым\nФейковые пользователи НЕ создаются"]
    
    UserAction -->|Редактирование / Сброс пароля| OpenEditModal["handleOpenEdit: установка формы, editFormError = null"]
    OpenEditModal --> SubmitEdit["handleSaveEdit -> usersApi.update"]
    SubmitEdit --> EditResponse{"Ответ бэкенда"}
    EditResponse -->|200 OK| SuccessEdit["Обновление в operators.value\nЗакрытие модалки"]
    EditResponse -->|Ошибка| ErrorEdit["editFormError = err.message\nМодалка остается открытой"]
    
    UserAction -->|Блокировка / Разблокировка| LockAction["confirmToggleLock -> usersApi.update(id, is_active)"]
    LockAction -->|Успех| UpdateLockState["selectedUserForAction.is_active = newActiveState"]
    
    UserAction -->|Удаление| DeleteAction["confirmDeleteUser -> usersApi.delete(id)"]
    DeleteAction -->|Успех| RemoveFromList["Удаление из operators.value и selectedUserIds"]
```

---

### 2.2. Диаграмма последовательности взаимодействия компонентов и API

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Администратор (root)
    participant View as UsersManagementView.vue
    participant SettingsAPI as settingsApi (/api/v1/settings/security)
    participant UsersAPI as usersApi (/api/v1/users)
    participant Server as AetherCore Axum Backend
    participant SQLite as SQLite Database (users, kv_store)

    %% 1. Инициализация и загрузка
    rect rgb(240, 245, 255)
    note right of Admin: 1. Инициализация экрана и политик
    Admin->>View: Переход на страницу пользователей
    par Загрузка пользователей
        View->>UsersAPI: usersApi.list()
        UsersAPI->>Server: GET /api/v1/users
        Server->>SQLite: SELECT * FROM users LEFT JOIN user_roles
        SQLite-->>Server: Rows
        Server-->>UsersAPI: 200 OK Vec<UserResponseDto>
        UsersAPI-->>View: operators.value = list
    and Загрузка политик безопасности
        View->>SettingsAPI: settingsApi.getSecurityPolicies()
        SettingsAPI->>Server: GET /api/v1/settings/security
        Server->>SQLite: SELECT value FROM kv_store WHERE key = 'security_policies'
        SQLite-->>Server: SecurityPolicies JSON
        Server-->>SettingsAPI: 200 OK SecurityPoliciesDto
        SettingsAPI-->>View: securityPolicies.value = policies
    end
    View->>View: Динамический расчет passwordHintText (длина, регистр, цифры, спецсимволы)
    end

    %% 2. Создание нового пользователя
    rect rgb(240, 255, 240)
    note right of Admin: 2. Создание нового пользователя
    Admin->>View: Клик "Добавить оператора", ввод данных
    View->>View: Отображение динамической подсказки требований к паролю
    Admin->>View: Нажатие кнопки "Создать"
    View->>UsersAPI: usersApi.create(CreateUserDto)
    UsersAPI->>Server: POST /api/v1/users
    alt Пароль или логин не прошел валидацию
        Server-->>UsersAPI: 422 Unprocessable Entity (VALIDATION_ERROR)
        UsersAPI-->>View: throw Error("Password length must be at least N...")
        View->>View: formError.value = err.message (Модалка открыта, ошибка видна)
    else Успешное создание
        Server->>SQLite: Hash Argon2id + INSERT INTO users + INSERT INTO user_roles
        SQLite-->>Server: User Created (UUID)
        Server-->>UsersAPI: 200 OK UserResponseDto
        UsersAPI-->>View: created User
        View->>View: operators.value.unshift(created)\nshowAddModal.value = false
    end
    end

    %% 3. Редактирование роли и сброс пароля
    rect rgb(255, 250, 240)
    note right of Admin: 3. Редактирование роли и сброс пароля
    Admin->>View: Выбор оператора -> "Редактировать"
    Admin->>View: Смена роли / ввод нового пароля -> "Сохранить"
    View->>UsersAPI: usersApi.update(id, UpdateUserDto)
    UsersAPI->>Server: PUT /api/v1/users/{id}
    alt Ошибка обновления
        Server-->>UsersAPI: 400 Bad Request / 403 Forbidden
        UsersAPI-->>View: throw Error(...)
        View->>View: editFormError.value = err.message
    else Успешное обновление
        Server->>SQLite: UPDATE users, user_roles
        SQLite-->>Server: Updated
        Server-->>UsersAPI: 200 OK UserResponseDto
        UsersAPI-->>View: updated User
        View->>View: Обновление элемента в operators.value\nshowEditModal.value = false
    end
    end

    %% 4. Блокировка и удаление
    rect rgb(255, 240, 245)
    note right of Admin: 4. Блокировка и удаление
    Admin->>View: Подтверждение удаления оператора
    View->>UsersAPI: usersApi.delete(id)
    UsersAPI->>Server: DELETE /api/v1/users/{id}
    Server->>SQLite: DELETE FROM users WHERE id = :id AND username != 'root'
    SQLite-->>Server: Deleted
    Server-->>UsersAPI: 200 OK
    UsersAPI-->>View: Success
    View->>View: operators.value = operators.filter(u => u.id !== id)
    end
```

---

## 3. Тест-кейсы для верификации (Given-When-Then)

### ТК-1: Загрузка пользователей и динамических политик безопасности
* **Given**: Пользователь авторизован как администратор (`root`).
* **When**: Открывается раздел управления пользователями `/settings/users`.
* **Then**: Выполняются параллельные запросы `usersApi.list()` и `settingsApi.getSecurityPolicies()`. Таблица наполняется списком пользователей, а `passwordHintText` рассчитывает текст требований на основе актуальных значений `min_password_length`, `require_uppercase`, `require_digits`, `require_special`.

---

### ТК-2: Попытка создания пользователя с простым паролем (Отображение валидации)
* **Given**: Открыто модальное окно добавления пользователя. В политиках безопасности заданы: `min_password_length: 8`, `require_digits: true`, `require_special: true`.
* **When**: Администратор заполняет `username: "test_op"` и `password: "123"` и нажимает кнопку "Создать".
* **Then**:
  1. Отправляется запрос `POST /api/v1/users`.
  2. Бэкенд возвращает статус `422 Unprocessable Entity` с текстом ошибки валидации.
  3. Модальное окно **не закрывается**.
  4. Внутри модального окна в красном блоке отображается текст ошибки `formError`.
  5. В таблице пользователей **не появляются** временные локальные записи.

---

### ТК-3: Успешное создание пользователя с сохранением в БД
* **Given**: Открыто модальное окно добавления пользователя.
* **When**: Администратор вводит валидный логин (`operator_1`) и пароль, удовлетворяющий политике (`Operator123!`), нажимает "Создать".
* **Then**:
  1. Запрос `POST /api/v1/users` завершается со статусом `200 OK`.
  2. Модальное окно закрывается, форма очищается.
  3. Новый пользователь сразу появляется во главе таблицы.
  4. При переходе на страницу дашборда и возврате обратно на страницу пользователей созданный пользователь **присутствует в списке**.

---

### ТК-4: Редактирование роли и сброс пароля
* **Given**: В таблице выбран существующий оператор.
* **When**: Администратор открывает окно редактирования, меняет роль с `viewer` на `operator`, вводит новый пароль и нажимает "Сохранить".
* **Then**: Вызывается `usersApi.update(id, ...)`. При успехе роль оператора в строке таблицы мгновенно обновляется, модальное окно закрывается. При ошибке валидации модальное окно остается открытым с текстом ошибки.

---

### ТК-5: Защита системного администратора (root)
* **Given**: В списке отображается пользователь `root`.
* **When**: Администратор просматривает действия для пользователя `root`.
* **Then**: Кнопки удаления и блокировки недоступны (скрыты/деактивированы), смена роли суперпользователя заблокирована.

