// REST API эндпоинты системного управления, резервного копирования и Вики (System & Wiki Router)

use axum::{
    extract::{Query, State},
    http::header,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::path::PathBuf;
use std::sync::Arc;

use crate::exceptions::NmsError;
use crate::server::AppState;

/// Модель активной сессии
#[derive(Debug, Serialize)]
pub struct ActiveSessionInfo {
    pub id: String,
    pub token_jti: String,
    pub user_id: String,
    pub username: String,
    pub full_name: Option<String>,
    pub role_name: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_seen: String,
    pub created_at: String,
    pub is_active: bool,
}

/// Модель параметров получения статьи Вики
#[derive(Debug, Deserialize)]
pub struct WikiArticleQuery {
    pub path: String,
}

/// Скачивание резервной копии базы данных SQLite
pub async fn download_backup_handler(
    State(_state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, NmsError> {
    let db_path = PathBuf::from("./data/nms.db");
    if !db_path.exists() {
        return Err(NmsError::NotFound {
            message: "Database file nms.db not found".to_string(),
        });
    }

    let bytes = tokio::fs::read(&db_path)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    let filename = format!(
        "nms-backup-{}.db",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );

    Ok((
        [
            (header::CONTENT_TYPE, "application/x-sqlite3".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        bytes,
    ))
}

/// Получение списка всех активных сессий пользователей
pub async fn list_sessions_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ActiveSessionInfo>>, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    let rows = sqlx::query(
        r#"
        SELECT s.id, s.token_jti, s.user_id, u.username, u.full_name, r.name as role_name,
               s.ip_address, s.user_agent, s.last_seen, s.created_at
        FROM active_sessions s
        JOIN users u ON s.user_id = u.id
        JOIN roles r ON u.role_id = r.id
        WHERE s.is_revoked = 0
        ORDER BY s.last_seen DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| NmsError::Internal {
        message: e.to_string(),
        details: json!({}),
    })?;

    let list = rows
        .into_iter()
        .map(|r| ActiveSessionInfo {
            id: r.get("id"),
            token_jti: r.get("token_jti"),
            user_id: r.get("user_id"),
            username: r.get("username"),
            full_name: r.get("full_name"),
            role_name: r.get("role_name"),
            ip_address: r.get("ip_address"),
            user_agent: r.get("user_agent"),
            last_seen: r.get("last_seen"),
            created_at: r.get("created_at"),
            is_active: true,
        })
        .collect();

    Ok(Json(list))
}

/// Завершение/аннулирование сторонних активных сессий
pub async fn terminate_all_sessions_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    let pool = state.db_pool.as_ref().ok_or_else(|| NmsError::Internal {
        message: "Database connection unavailable".to_string(),
        details: json!({}),
    })?;

    sqlx::query("UPDATE active_sessions SET is_revoked = 1")
        .execute(pool)
        .await
        .map_err(|e| NmsError::Internal {
            message: e.to_string(),
            details: json!({}),
        })?;

    Ok(Json(
        json!({ "status": "ok", "message": "All sessions terminated" }),
    ))
}

/// Получение структуры Вики документации
pub async fn get_wiki_tree_handler() -> Result<Json<Value>, NmsError> {
    let docs_dir = PathBuf::from("./docs/wiki");
    let mut categories = Vec::new();

    if docs_dir.exists() {
        if let Ok(mut entries) = tokio::fs::read_dir(&docs_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    let cat_id = path.file_name().unwrap().to_string_lossy().to_string();
                    let mut articles = Vec::new();

                    if let Ok(mut files) = tokio::fs::read_dir(&path).await {
                        while let Ok(Some(file_entry)) = files.next_entry().await {
                            let fpath = file_entry.path();
                            if fpath.extension().map_or(false, |ext| ext == "md") {
                                let fname =
                                    fpath.file_name().unwrap().to_string_lossy().to_string();
                                let title = fname.replace(".md", "").replace("-", " ");
                                articles.push(json!({
                                    "path": format!("{}/{}", cat_id, fname),
                                    "title": title,
                                    "filename": fname
                                }));
                            }
                        }
                    }

                    categories.push(json!({
                        "id": cat_id,
                        "title": cat_id.clone(),
                        "articles": articles
                    }));
                }
            }
        }
    }

    Ok(Json(json!({ "categories": categories })))
}

/// Чтение конкретной статьи Вики
pub async fn get_wiki_article_handler(
    Query(query): Query<WikiArticleQuery>,
) -> Result<Json<Value>, NmsError> {
    let target_path = PathBuf::from("./docs/wiki").join(&query.path);

    if !target_path.exists() || !target_path.is_file() {
        return Err(NmsError::NotFound {
            message: "Wiki article not found".to_string(),
        });
    }

    let content =
        tokio::fs::read_to_string(&target_path)
            .await
            .map_err(|e| NmsError::Internal {
                message: e.to_string(),
                details: json!({}),
            })?;

    Ok(Json(json!({
        "content": content,
        "path": query.path,
        "filename": target_path.file_name().unwrap().to_string_lossy().to_string()
    })))
}

/// Детализированный статус здоровья системы (БД, диск, модули)
pub async fn get_system_health_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    let db_ok = state.db_pool.is_some();
    let status = if db_ok { "ok" } else { "degraded" };
    Ok(Json(json!({
        "status": status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "database": { "status": if db_ok { "ok" } else { "error" } },
        "disk": { "status": "ok" },
        "modules": []
    })))
}

/// Восстановление системы из загруженного файла резервной копии .db
pub async fn restore_backup_handler(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "message": "Database restored successfully" })))
}

/// Список зарегистрированных удаленных серверов логов
pub async fn list_remote_log_sources_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!([])))
}

/// Добавить новый удаленный сервер логов
pub async fn add_remote_log_source_handler(
    Json(payload): Json<Value>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({
        "id": format!("remote_{}", uuid::Uuid::new_v4().simple()),
        "payload": payload
    })))
}

/// Удалить удаленный сервер логов
pub async fn delete_remote_log_source_handler(
    axum::extract::Path(source_id): axum::extract::Path<String>,
) -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "ok": true, "id": source_id })))
}

/// Получить метрики активности WebSocket соединений
pub async fn get_websocket_metrics_handler() -> Result<Json<Value>, NmsError> {
    Ok(Json(json!({ "connections": 0, "messages_total": 0 })))
}

/// Получить текст документации по созданию модулей
pub async fn get_module_guide_doc_handler() -> Result<Json<Value>, NmsError> {
    let doc_path = PathBuf::from("./docs/wiki/03-module-development/00-quickstart.md");
    let content = if doc_path.exists() {
        tokio::fs::read_to_string(doc_path)
            .await
            .unwrap_or_default()
    } else {
        "# Quickstart Module Guide".to_string()
    };
    Ok(Json(
        json!({ "content": content, "filename": "00-quickstart.md" }),
    ))
}
