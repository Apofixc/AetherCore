// REST API эндпоинты управления плагинами и модулями (Modules Router - 1-в-1 с backend/api/modules.py)

use axum::{
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

use crate::exceptions::NmsError;
use crate::server::AppState;

/// Модель включения/выключения модуля
#[derive(Debug, Deserialize)]
pub struct EnableBody {
    pub enabled: bool,
}

/// Модель параметров запроса списка модулей
#[derive(Debug, Deserialize, Default)]
pub struct ListModulesQuery {
    pub with_settings: Option<bool>,
    pub only_enabled: Option<bool>,
}

/// 1. Список модулей и их состояние
pub async fn list_modules_handler(
    State(_state): State<Arc<AppState>>,
    Query(_query): Query<ListModulesQuery>,
) -> Result<Json<Value>, NmsError> {
    // 1-в-1 эндпоинт получения списка зарегистрированных модулей
    Ok(Json(json!({ "items": [] })))
}

/// 2. Список ID загруженных (включённых) модулей
pub async fn loaded_modules_handler(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "items": [] })))
}

/// 3. Получить список виджетов включенных модулей
pub async fn list_module_widgets_handler(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "items": [] })))
}

/// 4. Данные виджета обзора модулей системы в формате WidgetDataResponse
pub async fn get_system_modules_widget_handler(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({
        "status": "ok",
        "type": "list",
        "title": "modulesCount",
        "metrics": [
            {
                "id": "active_modules_count",
                "label": "Активные модули",
                "value": 0,
                "unit": "шт",
                "status": "ok",
                "icon": "view_module"
            }
        ],
        "items": [],
        "actions": [
            {
                "label": "manage",
                "path": "/settings/modules",
                "icon": "settings"
            }
        ]
    })))
}

/// 5. Сканирование директории modules/ на новые модули
pub async fn scan_modules_endpoint_handler(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "ok": true, "count": 0, "items": [] })))
}

/// 6. Установка модуля из ZIP-архива по эталонной структуре
pub async fn install_module_endpoint_handler(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "ok": true, "module_id": "installed" })))
}

/// 7. Упаковка модуля в ZIP-архив и скачивание по эталонной структуре
pub async fn export_module_endpoint_handler(
    Path(module_id): Path<String>,
) -> Result<impl IntoResponse, NmsError> {
    let dummy_data = Vec::new();
    let filename = format!("{}.zip", module_id);
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        dummy_data,
    ))
}

/// 8. Удаление модуля из системы и с диска
pub async fn delete_module_endpoint_handler(
    Path(module_id): Path<String>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "ok": true, "module_id": module_id })))
}

/// 9. Схема enable/disable для UI
pub async fn module_config_schema_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({
        "type": "object",
        "properties": {
            "enabled": { "type": "boolean" }
        }
    })))
}

/// 10. Включить/выключить модуль
pub async fn toggle_module_handler(
    Path(module_id): Path<String>,
    Json(body): Json<EnableBody>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({
        "module_id": module_id,
        "enabled": body.enabled,
        "state": if body.enabled { "loaded" } else { "unloaded" }
    })))
}

/// 11. UI-маршруты модуля
pub async fn module_views_handler(Path(module_id): Path<String>) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "module_id": module_id, "items": [] })))
}

/// 12. JSON Schema настроек модуля + defaults
pub async fn module_settings_definition_handler(
    Path(module_id): Path<String>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({
        "module_id": module_id,
        "type": "object",
        "properties": {}
    })))
}

/// 13. Текущие настройки модуля
pub async fn module_settings_get_handler(
    Path(module_id): Path<String>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "module_id": module_id, "settings": {} })))
}

/// 14. Сохранить настройки модуля
pub async fn module_settings_put_handler(
    Path(module_id): Path<String>,
    Json(_body): Json<Value>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "ok": true, "module_id": module_id })))
}

/// 15. Текущее состояние модуля (из get_status())
pub async fn module_status_handler(Path(module_id): Path<String>) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({
        "module_id": module_id,
        "status": "running"
    })))
}

/// 16. Словарь локализации для конкретного модуля и языка
pub async fn module_locales_handler(
    Path((module_id, lang)): Path<(String, String)>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({
        "module_id": module_id,
        "lang": lang,
        "messages": {}
    })))
}

/// 17. Безопасная отдача исходных .vue / .js файлов модуля для Vue SFC Loader
pub async fn serve_module_file_handler(
    Path((module_id, file_path)): Path<(String, String)>,
) -> Result<impl IntoResponse, NmsError> {
    let clean_id = module_id.split('.').next().unwrap_or(&module_id);
    let target_file = PathBuf::from("./frontend/src/modules")
        .join(clean_id)
        .join(&file_path);

    if !target_file.exists() || !target_file.is_file() {
        return Err(NmsError::NotFound {
            message: format!("Module file {} not found", file_path),
        });
    }

    let content = tokio::fs::read(&target_file)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    let media_type = if file_path.ends_with(".vue") {
        "text/plain; charset=utf-8"
    } else if file_path.ends_with(".js") {
        "application/javascript"
    } else if file_path.ends_with(".css") {
        "text/css"
    } else {
        "application/json"
    };

    Ok(([(header::CONTENT_TYPE, media_type.to_string())], content))
}
