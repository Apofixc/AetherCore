# Модуль системы уведомлений (Notifications Engine)

Документ описывает подсистему генерации, персистентного хранения, группировки, фильтрации, эскалации и экспорта алертов и уведомлений в `nms-core`.

---

## 🏛️ Компоненты модуля `notify.rs`

- **Уровни критичности `NotificationSeverity`**: `Info`, `Success`, `Warning`, `Error` (сопоставляются с уровнями приоритета 1, 2, 3).
- **Модель сообщения `NotificationMessage`**:
  - `id`: Уникальный идентификатор уведомления в SQLite.
  - `module_id`: Идентификатор модуля-источника (по умолчанию `"core"`).
  - `user_id`: Идентификатор адресата.
  - `title`, `body`: Заголовок (авто-обрезка до 255 символов) и текст (авто-обрезка до 4000 символов).
  - `severity`, `category`: Критичность и категория (`system`, `security`, `module`, `user`).
  - `group_count`, `title_template`: Счетчик повторений при группировке за последние 60 секунд и шаблон заголовка (например, `Interface eth0 flap #{count}`).
  - `acknowledged_at`, `acknowledged_by`: Время и автор квитирования проработки алерта.
  - `escalated_at`: Штамп эскалации неквитированной критической ошибки.
  - `push_eligible`, `sound_eligible`, `sound_signal`: Динамические флаги пригодности к отправке push/звукового оповещения с учётом тихих часов.

---

## ⚙️ Основной функционал `NotificationEngine`

### 1. Отправка и Дедупликация (`notify` / `send_notification`)
- Проверяет глобальные настройки заглушения адресата (`muted_until`), порог критичности `min_severity` и подписки `subscribed_modules`.
- Если за последние 60 секунд было создано аналогичное непрочитанное уведомление, увеличивается `group_count`, а заголовок обновляется по `title_template`.
- При успешном создании публикуется событие `core.notifications.created` в шину `EventBus`.

### 2. Предпочтения пользователей (`get_notification_preferences` / `set_notification_preferences`)
- Хранение параметров в таблице SQLite `notification_preferences`.
- Поддержка тихих часов (`quiet_hours`) с проверкой расписания (`is_quiet_hours`) с перекрытием для критических ошибок (`Error`).

### 3. Прочтение и Квитирование
- `count_unread_notifications`: Подсчет непрочитанных с in-memory кэшированием.
- `mark_as_read`, `mark_as_unread`, `mark_all_as_read`.
- `acknowledge_notification`, `acknowledge_all_notifications`: Квитирование проработанных аварий.

### 4. Эскалация аварий (`process_alert_escalations`)
- Фоновый процесс проверяет неквитированные критические ошибки (`severity = Error`) старше $N$ минут.
- Устанавливает timestamp `escalated_at` и генерирует высокоприоритетное событие `core.notifications.escalated` в `EventBus`.

### 5. Очистка и Retention (`prune_notifications`)
- Удаляет уведомления старше $N$ дней (по умолчанию 30 дней), защищая от удаления непрочитанные/неквитированные аварии уровня `Error`.

### 6. Фильтрация, Пагинация и Экспорт
- `get_user_notifications`: Пагинация (`limit`, `offset`) и фильтрация по `unread_only`, `severity`, `category`, `search`.
- `export_user_notifications`: Экспорт в форматах CSV и JSON.

---

## 💡 Пример использования в Rust

```rust
use nms_core::{
    init_db_pool, EventBus, NotificationEngine, NotificationFilter, NotificationSeverity, NotifyParams,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let event_bus = EventBus::new(1024);
    let pool = init_db_pool(&PathBuf::from("/var/lib/nms/db.sqlite")).await?;

    // Инициализация движка уведомлений с подключением SQLite
    let engine = NotificationEngine::new_with_db(event_bus, pool);

    // 1. Отправка уведомления с группировкой по шаблону
    let notif = engine
        .notify(NotifyParams {
            user_id: "admin".to_string(),
            title: "Превышение нагрузки CPU #{count}".to_string(),
            body: "Загрузка процессора 98% на узле router-01".to_string(),
            severity: NotificationSeverity::Warning,
            category: "system".to_string(),
            module_id: "monitoring".to_string(),
            title_template: Some("Превышение нагрузки CPU #{count}".to_string()),
            ..Default::default()
        })
        .await?;

    if let Some(msg) = notif {
        println!("Sent notification ID: {}, title: {}", msg.id, msg.title);
    }

    // 2. Получение списка непрочитанных
    let list = engine
        .get_user_notifications(
            "admin",
            &NotificationFilter {
                unread_only: Some(true),
                ..Default::default()
            },
        )
        .await?;

    println!("Unread count: {}", list.unread_count);
    Ok(())
}
```
