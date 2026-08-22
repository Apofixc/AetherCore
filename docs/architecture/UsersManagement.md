# Архитектура: UsersManagementView.vue

## Диаграмма взаимодействия (Mermaid)

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Администратор
    participant View as UsersManagementView.vue
    participant API as usersApi (/api/v1/users)
    participant Server as Axum UserService
    participant SQLite as SQLite Database

    %% Загрузка списка
    Admin->>View: Открытие раздела пользователей
    View->>API: usersApi.list()
    API->>Server: GET /api/v1/users
    Server->>SQLite: SELECT * FROM users
    SQLite-->>Server: Rows
    Server-->>API: 200 OK Vec<UserResponseDto>
    API-->>View: operators.value = list

    %% Создание пользователя
    Admin->>View: Заполнение формы и клик "Создать"
    View->>API: usersApi.create(CreateUserDto)
    API->>Server: POST /api/v1/users
    Server->>SQLite: Argon2id hash + INSERT user
    SQLite-->>Server: Created Row
    Server-->>API: 201 Created UserResponseDto
    API-->>View: Добавление в список + закрытие модалки

    %% Редактирование роли / Сброс пароля
    Admin->>View: Изменение роли или ввод пароля для сброса
    View->>API: usersApi.update(id, { roles, password?, must_change_password? })
    API->>Server: PUT /api/v1/users/:id
    Server->>SQLite: UPDATE users SET ...
    SQLite-->>Server: Updated Row
    Server-->>API: 200 OK UserResponseDto
    API-->>View: Обновление роли в таблице + закрытие модалки

    %% Блокировка / Разблокировка
    Admin->>View: Клик по кнопке блокировки
    View->>API: usersApi.update(id, { is_active: !current })
    API->>Server: PUT /api/v1/users/:id
    Server->>SQLite: UPDATE users SET is_active = ...
    SQLite-->>Server: Updated Row
    Server-->>API: 200 OK UserResponseDto
    API-->>View: Переключение статуса в таблице

    %% Удаление
    Admin->>View: Подтверждение удаления
    View->>API: usersApi.delete(id)
    API->>Server: DELETE /api/v1/users/:id
    Server->>SQLite: DELETE FROM users WHERE id = :id
    SQLite-->>Server: Deleted
    Server-->>API: 200 OK
    API-->>View: Удаление из локального состояния таблицы
```

## Тест-кейсы для верификации

### ТК-1: Загрузка списка пользователей
* **Given**: Пользователь авторизован с ролью администратора (`users.view`).
* **When**: Открывается вкладка управления операторами `/settings/users`.
* **Then**: Вызывается `usersApi.list()`, таблица отображает полученных пользователей с бейджами ролей и статуса.

### ТК-2: Создание нового пользователя с валидацией
* **Given**: Открыто модальное окно создания пользователя.
* **When**: Заполнены логин, имя, email, пароль и выбрана роль `operator`, нажата кнопка "Создать".
* **Then**: Отправляется запрос `POST /api/v1/users`. При успехе созданный пользователь немедленно появляется в таблице, модальное окно закрывается.

### ТК-3: Удаление пользователя
* **Given**: В таблице выбран пользователь (не защищенный root/admin).
* **When**: Нажата кнопка удаления и подтверждено действие в модальном окне.
* **Then**: Отправляется `DELETE /api/v1/users/{id}`, пользователь исчезает из таблицы.
