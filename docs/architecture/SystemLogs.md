# Архитектура: SystemAdminView.vue & Логирование

## Диаграмма взаимодействия (Mermaid)

```mermaid
sequenceDiagram
    autonumber
    actor Admin as Администратор
    participant View as SystemAdminView.vue
    participant API as systemApi & eventsApi
    participant Server as Axum System & Events Handler
    participant Logger as LoggerService & EventBus

    %% Инициализация информации о системе
    View->>API: systemApi.getInfo()
    API->>Server: GET /api/v1/system/info
    Server-->>API: 200 OK { version, uptime_seconds, dev_mode, safe_mode }
    API-->>View: Отображение карточки статуса ядра

    %% Получение лог-провайдеров и логов
    View->>API: systemApi.getProviders()
    API->>Server: GET /api/v1/system/logs/providers
    Server-->>API: 200 OK Vec<LogProvider>
    API-->>View: Заполнение выпадающего списка провайдеров

    View->>API: systemApi.getLogs({ provider, level, search, limit })
    API->>Server: GET /api/v1/system/logs
    Server->>Logger: LoggerService::get_logs
    Logger-->>Server: LogQueryResult { lines, total_lines }
    Server-->>API: 200 OK
    API-->>View: Рендер строк логов в интерактивную консоль

    %% Скачивание файла логов
    Admin->>View: Клик "Скачать лог"
    View->>API: systemApi.downloadLog(provider)
    API->>Server: GET /api/v1/system/logs/download?provider=...
    Server-->>View: 200 OK (Content-Disposition: attachment)
    View-->>Admin: Сохранение файла лога на диск
```

## Тест-кейсы для верификации

### ТК-1: Загрузка системной информации и времени непрерывной работы
* **Given**: Страница `/settings/system` открыта.
* **When**: Компонент монтируется.
* **Then**: Вызывается `systemApi.getInfo()`, отображается версия ядра, режим работы и форматированный uptime.

### ТК-2: Интерактивный поиск и фильтрация логов
* **Given**: Выбран уровень логирования `WARN` или `ERROR` и введена поисковая строка.
* **When**: Пользователь применяет фильтр или срабатывает интервал автообновления.
* **Then**: Отправляется `GET /api/v1/system/logs` с параметрами `level` и `search`, консоль обновляется отфильтрованными записями.
