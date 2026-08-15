# 🔔 WIT Интерфейс `nms:core/notify`

## 1. Назначение и роль в ядре

Интерфейс `nms:core/notify` предоставляет плагинам возможность отправлять критические алерты и уведомления администраторам платформы через настроенные каналы (Telegram, Email, Webhook).

---

## 2. Полный код WIT спецификации

```wit
interface notify {
    enum alert-severity {
        info,
        warning,
        critical,
    }

    /// Отправить системный алерт
    send-alert: func(severity: alert-severity, title: string, message: string) -> result<_, string>;
}
```

---

## 3. Пример использования на Rust (Гостевой код плагина)

```rust
use crate::bindings::nms::core::notify::{self, AlertSeverity};

pub fn check_temperature(temp: f32) {
    if temp > 80.0 {
        let _ = notify::send_alert(
            AlertSeverity::Critical,
            "Перегрев оборудования!",
            &format!("Текущая температура датчика достигла {} °C", temp),
        );
    }
}
```
