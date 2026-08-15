# 11. Локализация бэкенда и модулей (i18n)

Модуль `i18n` в `nms-core` предоставляет механизм локализации системных сообщений ядра и плагинов на стороне бэкенда.

---

## 1. Определение языка запроса

Функция `get_lang` определяет код языка (`ru` или `en`) на основе параметров HTTP-запроса:

```rust
use nms_core::get_lang;

let lang = get_lang(
    query_params.get("lang").map(|s| s.as_str()),
    headers.get("accept-language").and_then(|h| h.to_str().ok()),
);
```

1. Если передан GET-параметр `?lang=ru` или `?lang=en`, применяется он.
2. В противном случае анализируется заголовок `Accept-Language` (поиск подстроки `ru`).
3. По умолчанию используется английский язык (`en`).

---

## 2. Перевод сообщений (`I18nEngine`)

Реестр `I18nEngine` позволяет переводить ключи и подставлять параметры:

```rust
use nms_core::I18nEngine;

let i18n = I18nEngine::new();

// Простой перевод
let msg = i18n.tr("ru", "auth_required", None, None);
// Результат: "Необходима авторизация"

// Перевод с именованными параметрами {param}
let params = [("deleted", "10")];
let log_msg = i18n.tr("ru", "audit_logs_rotated", None, Some(&params));
// Результат: "Удалено 10 устаревших записей аудита"
```

---

## 3. Локализация модулей (`locales/*.json`)

Каждый модуль/плагин может содержать собственную папку `locales/` с JSON-файлами переводов:

```
my-module/
├── manifest.yaml
└── locales/
    ├── ru.json
    └── en.json
```

Содержимое `ru.json`:
```json
{
  "device_offline_alert": "Устройство {device_name} недоступно!"
}
```

Автоматическая загрузка локалей модуля в ядро:
```rust
i18n.load_module_locales("/path/to/my-module")?;
```
