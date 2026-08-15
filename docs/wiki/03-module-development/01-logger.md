# Модуль системного логирования (Logger & Log Providers)

Документ описывает устройство подсистемы неблокирующего логирования и управления локальными/удаленными лог-провайдерами в Rust-ядре `nms-core`.

---

## 🏛️ Компоненты модуля

### 1. Трассировка `logger.rs`
- Функция `init_logging(log_file_path)` инициализирует подсистему `tracing`.
- **Консольный вывод**: `stdout` формата `date | level | target | message`.
- **Файловый вывод**: **Неблокирующая запускная запись в `backend.log`** на базе `tracing_appender::non_blocking` с возвратом `LoggingGuard` (аналог Python QueueHandler / QueueListener).
- **Уровень логирования**: Динамически считывается из переменной окружения `NMS_LOG_LEVEL` (с запасным проверкой `RUST_LOG` и дефолтом `info,nms_core=info,nms=info`).
- **Безопасная переинициализация**: При повторном вызове `init_logging` в рамках одного процесса (например, при интеграционных тестах или старте субмодулей) глобальный `tracing_subscriber` пропускает повторную регистрацию без генерации ошибок.
- **Хранение `LoggingGuard`**: Объект `LoggingGuard` должен сохраняться в локальной переменной `main()` (`let _log_guard = ...`), чтобы фоновый поток записи логов не закрывался до завершения приложения.

### 2. Провайдеры логов `log_providers.rs`
- **Трейт `LogProvider`**: Асинхронный контракт `#[async_trait]` для любых источников логов (`get_logs`, `download_log`, `is_available`, `get_info`).
- **`LocalFileLogProvider`**: Чтение и фильтрация локальных лог-файлов на сервере. Безопасно обрабатывает бинарные/битые UTF-8 данные с заменяющими символами (lossy UTF-8).
- **`RemoteHTTPLogProvider`**: Получение и скачивание логов с удаленных серверов NMS по HTTP API с Bearer-токенами.
- **Фильтрация и парсинг**:
  - Фильтрация по уровням (`ALL`, `TRACE`, `DEBUG`, `INFO`, `WARN`/`WARNING`, `ERROR`/`CRITICAL`/`FATAL`).
  - Поиск по ключевой строке `search` (case-insensitive).
  - Нативная очистка ANSI escape-кодов (`clean_ansi_codes`).
  - Ограничение размера выдачи (до 2000 строк) и пагинация с конца файла.
  - Метод `download_log()` для скачивания бинарного/текстового дампа файла.
- **`LogProviderRegistry`**: Потокобезопасный реестр `Arc<RwLock<HashMap<String, Arc<dyn LogProvider>>>>` для динамической регистрации локальных и удаленных лог-источников.

---

## 🌐 REST API Эндпоинты

Ядро `nms-core` предоставляет готовые эндпоинты веб-сервера Axum для получения логов:

1. **`GET /api/v1/system/logs`** — Получение списка зарегистрированных провайдеров логов.
   - *Ответ*: Массив объектов `LogProviderInfo` (`id`, `name`, `category`, `exists`, `size_bytes`).
2. **`GET /api/v1/system/logs/{provider_id}`** — Чтение отфильтрованного содержимого лога.
   - *Query-параметры*: `lines` (default 200), `level` (default "ALL"), `search` (опционально).
   - *Ответ*: Объект `LogDataResult` (`id`, `name`, `content`, `total_lines`, `matched_lines`).
3. **`GET /api/v1/system/logs/{provider_id}/download`** — Скачивание лог-файла целиком.
   - *Ответ*: Бинарный/текстовый поток с заголовком `Content-Disposition: attachment`.

---

## 💡 Пример использования в Rust

```rust
use nms_core::init_logging;
use nms_core::log_providers::{LocalFileLogProvider, LogProviderRegistry, RemoteHTTPLogProvider};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 0. Инициализация неблокирующего логирования в файл и stdout
    let _log_guard = init_logging(Some(PathBuf::from("backend.log")))?;

    let registry = LogProviderRegistry::new();

    // 1. Регистрация локального лог-файла
    let local_provider = Arc::new(LocalFileLogProvider::new(
        "backend.log",
        "backend.log",
        PathBuf::from("./backend.log"),
    ));
    registry.register(local_provider).await;

    // 2. Регистрация удаленного HTTP-источника
    let remote_provider = Arc::new(RemoteHTTPLogProvider::new(
        "remote-node-01",
        "Удаленный узел 01",
        "https://192.168.1.50/api/v1/system/logs/backend.log",
        Some("secret-api-token".to_string()),
    ));
    registry.register(remote_provider).await;

    // 3. Чтение и фильтрация логов
    if let Some(provider) = registry.get("backend.log").await {
        let result = provider.get_logs(100, "ERROR", "database").await?;
        println!("Matched lines: {}", result.matched_lines);
    }

    Ok(())
}
```

---

## 🧪 Покрытие автотестами
- **`logger_test.rs`**: Тестирование подсистемы неблокирующего логирования `logger.rs` (создание директории, инициализация файла лога, работу макросов `tracing::info!` и корректную переинициализацию).
- **`log_providers_test.rs`**: Тестирование модуля лог-провайдеров `log_providers.rs` (очистка ANSI-кодов, сопоставление уровней логов, реестр провайдеров, локальное чтение файла и обработка не-UTF8 символов).


