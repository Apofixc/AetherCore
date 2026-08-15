# Модуль планировщика задач (Task Scheduler)

Документ описывает подсистему асинхронного планирования и фонового выполнения задач плагинов и ядра в `nms-core`.

---

## 🏛️ Компоненты планировщика `scheduler.rs`

Планировщик `SchedulerManager` работает на базе Tokio Runtime и обеспечивает безопасное параллельное выполнение 3 типов фоновых задач:

1. **Периодические задачи (`every`)**:
   - Выполнение асинхронной функции каждые `seconds` секунд.
   - Изолированное выполнение с автоматическим подсчетом попыток (`runs_count`), ошибок (`error_count`) и сохранением `last_error` в `JobInfo`.

2. **Задачи по расписанию (`cron`)**:
   - Выполнение по 5-элементному выражению cron (`min hour dom month dow`).
   - Поддержка стандартизированных макросов: `@hourly`, `@daily`, `@midnight`, `@weekly`, `@monthly`.
   - Автоматическая коррекция 5-полевых выражений и интеграция с `tokio-cron-scheduler`.

3. **Однократные задачи (`once`)**:
   - Выполнение задачи один раз с задержкой в `delay_seconds` секунд.
   - Автоматическая пометка `is_running = false` после выполнения.

4. **Управление и отмена задач**:
   - `cancel_job(job_id)` — индивидуальная отмена задачи по ID.
   - `cancel_module_jobs(module_id)` — массовая отмена всех активных задач указанного модуля плагина.
   - `get_jobs(filter_module_id)` — получение списка текущих задач с полной статистикой выполнения.
   - `stop()` — остановка всего планировщика и отмена всех зарегистрированных задач.
   - `is_running()` — проверка активности планировщика.

---

## 📋 Структура метаданных задачи `JobInfo`

При запросе `get_jobs()` возвращается вектор объектов `JobInfo`:

```rust
pub struct JobInfo {
    pub job_id: String,           // Уникальный ID задачи ("job_xxxxxxxx")
    pub job_type: String,         // Тип задачи: "every", "cron", "once"
    pub name: String,             // Название задачи
    pub module_id: Option<String>,// ID модуля/плагина (если задан)
    pub seconds: Option<f64>,     // Интервал в секундах (для every)
    pub cron_expr: Option<String>,// Cron-выражение (для cron)
    pub delay: Option<f64>,       // Задержка в секундах (для once)
    pub runs_count: u64,          // Количество успешных запусков
    pub error_count: u64,         // Количество ошибок при выполнении
    pub last_run: Option<u64>,    // UNIX timestamp последнего запуска
    pub last_error: Option<String>,// Сообщение последней ошибки (если была)
    pub is_running: bool,         // Статус активности задачи
}
```

---

## 💡 Пример использования в Rust

```rust
use nms_core::SchedulerManager;
use anyhow::Result;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    let scheduler = SchedulerManager::new();

    // 1. Задача каждые 10 секунд
    let job1 = scheduler.every(10.0, Some("ping".to_string()), "icmp_ping", || async {
        println!("Running ping check...");
        Ok(())
    }).await?;

    // 2. Задача по макросу @hourly
    let job2 = scheduler.cron("@hourly", Some("reports".to_string()), "hourly_report", || async {
        println!("Generating hourly report...");
        Ok(())
    }).await?;

    // 3. Однократная задача через 5 секунд
    let job3 = scheduler.once(5.0, Some("cleanup".to_string()), "temp_cleanup", || async {
        println!("Cleaning up temporary files...");
        Ok(())
    }).await?;

    // 4. Получение списка всех активных задач модуля "ping"
    let jobs = scheduler.get_jobs(Some("ping")).await;
    println!("Active ping jobs: {}", jobs.len());

    // 5. Отмена задач модуля "ping"
    scheduler.cancel_module_jobs("ping").await;

    // 6. Остановка всех задач и завершение работы планировщика
    scheduler.stop().await;

    Ok(())
}
```
