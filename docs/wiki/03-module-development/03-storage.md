# Модуль базы данных и системного аудита (Storage & Audit)

Документ описывает подсистему работы с базой данных **SQLx** (SQLite/PostgreSQL), инициализацию первично необходимых данных (роли, права, пользователь root), управление системными настройками и сервис журналирования аудита безопасности в `nms-core`.

---

## 🏛️ Компоненты модуля

### 1. Подключение, схема БД и сидирование (`db.rs`)
- **Инициализация пула соединений SQLx**: `init_db_pool(db_path)`.
- **Настройка PRAGMA SQLite**: `PRAGMA journal_mode=WAL;`, `busy_timeout=30000;`, `synchronous=NORMAL;`, `foreign_keys=ON;`.
- **Автоматическое создание 11 системных таблиц**:
  - `roles` — роли пользователей.
  - `permissions` — категории и наименования разрешений.
  - `role_permissions` — матрица связей ролей и прав доступа (с `ON DELETE CASCADE`).
  - `users` — учетные записи (пароли Argon2id, MFA, статусы блокировок).
  - `audit_logs` — журнал событий безопасности.
  - `system_settings` — хранилище системных настроек (Key-Value).
  - `active_sessions` — активные JWT-сессии пользователей.
  - `remote_log_sources` — источники удаленных логов.
  - `system_events_journal` — журнал событий для повтора/восстановления WebSocket.
  - `notifications` — персистентные уведомления системы.
  - `notification_preferences` — пользовательские настройки и заглушки уведомлений.
- **Инициализация начальных данных (`seed_initial_data`)**:
  - Системные роли (`1`: Superuser, `2`: Admin, `3`: Operator, `4`: Viewer, `role-admin`).
  - 12 системных прав доступа (`system.all`, `system.admin`, `users.view`, `users.manage`, `roles.view`, `roles.manage`, `settings.view`, `settings.edit`, `modules.view`, `modules.manage`, `audit.view`, `audit.export`).
  - Создание главного администратора `root` (`usr-root-01`, пароль по умолчанию `admin` с хешированием Argon2id).
- **Управление системными настройками**:
  - `get_system_setting(pool, key)` — чтение значения настройки.
  - `set_system_setting(pool, key, value)` — сохранение или обновление (UPSERT) настройки.

---

### 2. Журнал аудита (`audit.rs`)
- **Запись событий**: Метод `log_audit_event(pool, user_id, username, action, resource, details, ip_address)` фиксирует действия операторов.
- **Ротация логов**: Метод `rotate_audit_logs(pool, max_days, max_records)` автоматически очищает старые записи (старше `max_days` дней или при превышении `max_records`).
- **Пагинация и забор записей**:
  - `get_audit_logs(pool, limit, offset)` — получении списка событий (от новых к старым).
  - `count_audit_logs(pool)` — получение общего количества записей аудита.

---

## 💡 Пример использования в Rust

```rust
use nms_core::{
    count_audit_logs, get_audit_logs, get_system_setting, init_db_pool,
    log_audit_event, rotate_audit_logs, set_system_setting,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Инициализация БД (автоматически создаст таблицы, роли, права и root)
    let pool = init_db_pool(&PathBuf::from("./data/nms.db")).await?;

    // 2. Запись и чтение системной настройки
    set_system_setting(&pool, "site_title", "NMS Enterprise Portal").await?;
    if let Some(title) = get_system_setting(&pool, "site_title").await? {
        println!("Site title: {}", title);
    }

    // 3. Запись нового события аудита
    log_audit_event(
        &pool,
        Some("usr-root-01"),
        "root",
        "settings.update",
        "system",
        Some("Updated log level to DEBUG"),
        Some("127.0.0.1"),
    ).await?;

    // 4. Получение количества и пагинированного списка записей
    let total = count_audit_logs(&pool).await?;
    let logs = get_audit_logs(&pool, 10, 0).await?;
    println!("Total logs: {}, fetched: {}", total, logs.len());

    // 5. Ротация журнала аудита
    let deleted = rotate_audit_logs(&pool, 90, 100_000).await?;
    println!("Cleaned {} audit entries", deleted);

    Ok(())
}
```

