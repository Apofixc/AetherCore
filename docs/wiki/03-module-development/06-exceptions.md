# 06. Обработка ошибок и исключений (Exceptions)

В архитектуре `nms-webui-v2` обработка ошибок стандартизирована через тип `NmsError` ядра `nms-core`.

---

## 1. Структура ответа с ошибкой (JSON Schema)

Каждая HTTP-ошибка возвращается клиенту в единообразном JSON-формате:

```json
{
  "error": {
    "code": "MODULE_NOT_FOUND",
    "message": "Module 'ping-collector' not found",
    "details": {
      "module_id": "ping-collector"
    }
  }
}
```

* **`code`**: Уникальный машиночитаемый идентификатор ошибки (например, `AUTH_REQUIRED`, `INSUFFICIENT_PERMISSIONS`, `VALIDATION_ERROR`).
* **`message`**: Человекочитаемое описание ошибки на английском языке (для интерфейса используется локализация по коду/ключу).
* **`details`**: Дополнительные структурные данные (JSON Object) с параметрами ошибки (например, имя недопустимого поля или ID модуля).

---

## 2. Основные типы ошибок (`NmsError`)

| Вариант `NmsError` | HTTP Status | Код `code` | Назначение |
| :--- | :--- | :--- | :--- |
| `AuthRequired` | 401 Unauthorized | `AUTH_REQUIRED` | Ошибка авторизации или истёкший токен |
| `PermissionDenied` | 403 Forbidden | `INSUFFICIENT_PERMISSIONS` | Недостаточно прав для выполнения действия |
| `ModuleDisabled` | 403 Forbidden | `MODULE_DISABLED` | Запрошенный модуль отключен администратором |
| `NotFound` | 404 Not Found | `NOT_FOUND` | Ресурс или объект не найден |
| `ModuleNotFound` | 404 Not Found | `MODULE_NOT_FOUND` | Модуль не зарегистрирован в системе |
| `Validation` | 400 Bad Request | `VALIDATION_ERROR` | Ошибка валидации входных данных запроса |
| `ModuleValidationError` | 400 Bad Request | `MODULE_VALIDATION_ERROR` | Невалидный manifest.yaml или файлы плагина |
| `Internal` | 500 Internal Error | `INTERNAL_ERROR` | Внутренняя ошибка бэкенда |
| `Custom` | Настраиваемый | Настраиваемый | Произвольная ошибка с указанием HTTP-кода |

---

## 3. Использование в Axum обработчиках

Благодаря реализации трейта `IntoResponse` для `NmsError`, функции-обработчики Axum могут возвращать `Result<T, NmsError>`:

```rust
use axum::{extract::Path, Json};
use nms_core::NmsError;

pub async fn get_module_handler(Path(id): Path<String>) -> Result<Json<ModuleInfo>, NmsError> {
    if id == "disabled" {
        return Err(NmsError::ModuleDisabled { module_id: id });
    }
    
    let module = find_module(&id).ok_or_else(|| NmsError::ModuleNotFound { module_id: id })?;
    Ok(Json(module))
}
```
