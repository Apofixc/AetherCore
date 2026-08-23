//! # Системные эндпоинты ядра (`/api/v1/system`)
//!
//! Предоставляет HTTP эндпоинты для:
//! - Получения статуса и версии ядра (`GET /api/v1/system/info`).
//! - Экспорта словарей локализации (`GET /api/v1/system/i18n/{locale}`).
//! - Чтения журнала аудита (`GET /api/v1/system/audit`).
//! - Получения списка лог-провайдеров (`GET /api/v1/system/logs/providers`).
//! - Поиска и фильтрации системных логов (`GET /api/v1/system/logs`).
//! - Скачивания файла системного журнала (`GET /api/v1/system/logs/download`).

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use aethercore_common::error::{AppError, ErrorResponse};
use aethercore_common::i18n::{global, Locale};
use aethercore_core::auth::check_permission;
use aethercore_core::db::DbStorageStats;
use aethercore_core::services::{
    AuditArchiveInfo, AuditLogRecord, BackupInfo, LogLevel, LogProvider, LogQueryResult,
    RestoreResult,
};
use axum::extract::Multipart;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Создать вложенный роутер системных эндпоинтов `/system`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/info", get(system_info_handler))
        .nest("/scheduler", super::scheduler::router())
        .route("/i18n/{locale}", get(i18n_export_handler))
        .route("/audit", get(audit_logs_handler).delete(clear_audit_logs_handler))
        .route("/audit/count", get(audit_count_handler))
        .route("/audit/rotate", post(rotate_audit_logs_handler))
        .route("/audit/import", post(import_audit_logs_handler))
        .route("/audit/archives", get(list_audit_archives_handler))
        .route("/audit/archives/{filename}", get(download_audit_archive_handler))
        .route("/logs/providers", get(log_providers_handler))
        .route("/logs", get(logs_query_handler))
        .route("/logs/download", get(logs_download_handler))
        .route("/db/stats", get(db_stats_handler))
        .route("/backup/list", get(list_backups_handler))
        .route("/backup/create", post(create_backup_handler))
        .route("/backup/download/{filename}", get(download_backup_handler))
        .route("/backup/restore", post(restore_backup_handler))
        .route("/backup/upload-restore", post(upload_restore_backup_handler))
        .route("/backup/{filename}", axum::routing::delete(delete_backup_handler))
}

/// Ответ REST API с метаинформацией о запущенном экземпляре ядра платформы
#[derive(Debug, Serialize)]
pub struct SystemInfoResponse {
    /// Имя платформы
    pub name: &'static str,
    /// Версия платформы
    pub version: &'static str,
    /// Время непрерывной работы в секундах
    pub uptime_seconds: u64,
    /// Флаг режима разработки (`dev_mode`)
    pub dev_mode: bool,
    /// Флаг аварийного безопасного режима (`safe_mode`)
    pub safe_mode: bool,
}

/// GET /api/v1/system/info
///
/// Получить информацию о версии ядра, времени непрерывной работы (uptime) и активных режимах работы (`dev_mode`, `safe_mode`).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
///
/// # Возвращаемое значение
/// Структура [`SystemInfoResponse`].
async fn system_info_handler(State(state): State<AppState>) -> Json<SystemInfoResponse> {
    let uptime = state.start_time.elapsed().as_secs();
    Json(SystemInfoResponse {
        name: "AetherCore Platform",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
        dev_mode: state.config.server.dev_mode,
        safe_mode: state.config.server.safe_mode,
    })
}

/// GET /api/v1/system/i18n/:locale
///
/// Экспортировать полный словарь локализации для указанного языка (например, `"ru"` или `"en"`).
///
/// # Аргументы
/// * `locale_str` — Строковый код языка (`"ru"`, `"en"`).
///
/// # Возвращаемое значение
/// JSON-словарь пар `ключ -> шаблон_перевода`.
async fn i18n_export_handler(Path(locale_str): Path<String>) -> Json<HashMap<String, String>> {
    let locale = Locale::from_str_relaxed(&locale_str);
    let dict = global().export_locale(locale);
    Json(dict)
}

/// Параметры постраничной пагинации и поиска журнала аудита
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    /// Максимальное число возвращаемых записей (по умолчанию 50, макс 500)
    pub limit: Option<u32>,
    /// Смещение для постраничной пагинации
    pub offset: Option<u64>,
    /// Поисковый запрос
    pub search: Option<String>,
    /// Идентификатор последнего прочитанного события (для обратной совместимости)
    pub after_id: Option<i64>,
}

/// Ответ REST API с общим количеством записей журнала аудита
#[derive(Debug, Serialize)]
pub struct AuditCountResponse {
    /// Общее количество записей
    pub total: i64,
}

/// GET /api/v1/system/audit/count
///
/// Получить общее количество записей журнала аудита безопасности с учетом фильтра.
async fn audit_count_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditCountResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let total = state
        .audit_service
        .count_logs(query.search.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(AuditCountResponse { total }))
}

/// GET /api/v1/system/audit
///
/// Получить записи журнала аудита безопасности и действий пользователей.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `_auth` — Авторизованный пользователь [`AuthUser`].
/// * `query` — Параметры пагинации [`AuditQuery`].
///
/// # Возвращаемое значение
/// Список записей аудита [`AuditLogRecord`] и заголовок `X-Total-Count`.
async fn audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(query): Query<AuditQuery>,
) -> Result<(HeaderMap, Json<Vec<AuditLogRecord>>), (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let limit = query.limit.unwrap_or(50);
    let total = state
        .audit_service
        .count_logs(query.search.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let logs = state
        .audit_service
        .list_logs(limit, query.offset, query.search.as_deref())
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-total-count",
        total.to_string().parse().unwrap_or(axum::http::HeaderValue::from_static("0")),
    );

    Ok((headers, Json(logs)))
}

/// Ответ REST API на очистку журнала аудита
#[derive(Debug, Serialize)]
pub struct ClearAuditResponse {
    /// Флаг успешного выполнения
    pub success: bool,
    /// Количество удаленных записей
    pub deleted_count: u64,
}

/// DELETE /api/v1/system/audit
///
/// Очистить журнал аудита безопасности (доступно суперпользователю или с правами access.manage).
async fn clear_audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    headers: HeaderMap,
    AuthUser(claims): AuthUser,
) -> Result<Json<ClearAuditResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let client_ip = crate::middleware::extract_client_ip(&headers);

    let deleted_count = state
        .audit_service
        .clear_logs()
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    // Фиксируем факт очистки журнала аудита
    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "audit.clear",
            "system/audit",
            "success",
            Some(&format!("Cleared {} audit records", deleted_count)),
            Some(&client_ip),
        )
        .await;

    Ok(Json(ClearAuditResponse {
        success: true,
        deleted_count,
    }))
}

fn default_true() -> bool {
    true
}

/// Параметры запроса на ротацию журнала аудита
#[derive(Debug, Deserialize)]
pub struct RotateAuditRequest {
    /// Срок давности в днях (по умолчанию 90)
    pub days: Option<u32>,
    /// Сохранить ли архивный JSON-файл перед удалением
    #[serde(default = "default_true")]
    pub archive: bool,
}

/// Ответ на запрос ротации журнала аудита
#[derive(Debug, Serialize)]
pub struct RotateAuditResponse {
    /// Флаг успеха
    pub success: bool,
    /// Количество удаленных записей
    pub deleted_count: u64,
    /// Имя созданного архивного файла
    pub archive_filename: Option<String>,
}

/// POST /api/v1/system/audit/rotate
///
/// Выполнить ротацию журнала аудита (удаление записей старше N дней с опциональной архивацией).
async fn rotate_audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    headers: HeaderMap,
    AuthUser(claims): AuthUser,
    Json(payload): Json<RotateAuditRequest>,
) -> Result<Json<RotateAuditResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let client_ip = crate::middleware::extract_client_ip(&headers);
    let retention_days = payload.days.unwrap_or(90);
    let archive_dir = std::path::PathBuf::from("data/archives");

    let (deleted_count, archive_filename) = state
        .audit_service
        .archive_and_prune(retention_days, payload.archive, &archive_dir)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let details = format!(
        "Rotated audit logs older than {} days (deleted: {}, archive: {:?})",
        retention_days, deleted_count, archive_filename
    );

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "audit.rotate",
            "system/audit",
            "success",
            Some(&details),
            Some(&client_ip),
        )
        .await;

    Ok(Json(RotateAuditResponse {
        success: true,
        deleted_count,
        archive_filename,
    }))
}

/// Запрос на импорт записей аудита из архива
#[derive(Debug, Deserialize)]
pub struct ImportAuditRequest {
    /// Список записей аудита для вставки
    pub records: Vec<AuditLogRecord>,
}

/// Ответ на запрос импорта записей аудита
#[derive(Debug, Serialize)]
pub struct ImportAuditResponse {
    /// Флаг успеха
    pub success: bool,
    /// Количество успешно импортированных записей
    pub imported_count: usize,
}

/// POST /api/v1/system/audit/import
///
/// Импортировать записи аудита из файла архива в базу данных.
async fn import_audit_logs_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    headers: HeaderMap,
    AuthUser(claims): AuthUser,
    Json(payload): Json<ImportAuditRequest>,
) -> Result<Json<ImportAuditResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let client_ip = crate::middleware::extract_client_ip(&headers);

    let imported_count = state
        .audit_service
        .import_logs(&payload.records)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let details = format!("Imported {} audit records from archive", imported_count);

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "audit.import",
            "system/audit",
            "success",
            Some(&details),
            Some(&client_ip),
        )
        .await;

    Ok(Json(ImportAuditResponse {
        success: true,
        imported_count,
    }))
}

/// GET /api/v1/system/audit/archives
///
/// Получить список всех сохраненных файлов архива журнала аудита.
async fn list_audit_archives_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<AuditArchiveInfo>>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let archive_dir = std::path::PathBuf::from("data/archives");
    let archives = state
        .audit_service
        .list_archives(&archive_dir)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(archives))
}

/// GET /api/v1/system/audit/archives/{filename}
///
/// Скачать архивный JSON-файл журнала аудита.
async fn download_audit_archive_handler(
    Path(filename): Path<String>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "access.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        let err = AppError::bad_request("Invalid filename");
        return Err((StatusCode::BAD_REQUEST, Json(err.to_api_response(locale))));
    }

    let archive_path = std::path::PathBuf::from("data/archives").join(&filename);
    if !archive_path.exists() {
        let err = AppError::not_found(format!("Archive '{}' not found", filename));
        return Err((StatusCode::NOT_FOUND, Json(err.to_api_response(locale))));
    }

    let content = tokio::fs::read(&archive_path).await.map_err(|e| {
        let err = AppError::internal(e.to_string());
        (StatusCode::INTERNAL_SERVER_ERROR, Json(err.to_api_response(locale)))
    })?;

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    headers.insert(
        CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename).parse().unwrap(),
    );

    Ok((headers, content))
}

/// GET /api/v1/system/logs/providers
///
/// Получить список доступных источников логов (ядро, модули, системные файлы).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `_auth` — Авторизованный пользователь [`AuthUser`].
///
/// # Возвращаемое значение
/// Список доступных провайдеров логов [`LogProvider`].
async fn log_providers_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<LogProvider>>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let providers = state.logger_service.list_providers().map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(providers))
}

/// Параметры фильтрации и поиска системных логов
#[derive(Debug, Deserialize)]
pub struct LogQueryParams {
    /// Идентификатор провайдера логов (по умолчанию `"system"`)
    pub provider: Option<String>,
    /// Максимальное число возвращаемых строк
    pub limit: Option<usize>,
    /// Минимальный уровень логирования для фильтрации (`TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`)
    pub level: Option<String>,
    /// Подстрока полнотекстового поиска по содержимому сообщения
    pub search: Option<String>,
}

/// GET /api/v1/system/logs
///
/// Выполнить поиск и выборку записей логов по источнику, уровню и ключевым словам.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `_auth` — Авторизованный пользователь [`AuthUser`].
/// * `params` — Параметры поиска [`LogQueryParams`].
///
/// # Возвращаемое значение
/// Результаты поиска логов [`LogQueryResult`].
async fn logs_query_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(params): Query<LogQueryParams>,
) -> Result<Json<LogQueryResult>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let provider_id = params.provider.as_deref().unwrap_or("system");
    let limit = params.limit.unwrap_or(200);
    let min_level = params.level.as_deref().and_then(LogLevel::from_str_loose);

    let result = state
        .logger_service
        .get_logs(provider_id, limit, min_level, params.search.as_deref())
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    Ok(Json(result))
}

/// Параметры скачивания файла логов
#[derive(Debug, Deserialize)]
pub struct LogDownloadParams {
    /// Идентификатор источника логов (по умолчанию `"system"`)
    pub provider: Option<String>,
}

/// GET /api/v1/system/logs/download
///
/// Скачать полный файл логов указанного провайдера в виде бинарного потока `text/plain` с заголовком `Content-Disposition: attachment`.
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `_auth` — Авторизованный пользователь [`AuthUser`].
/// * `params` — Параметры скачивания [`LogDownloadParams`].
async fn logs_download_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Query(params): Query<LogDownloadParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }
    let provider_id = params.provider.as_deref().unwrap_or("system");

    let (bytes, filename) = state
        .logger_service
        .download_log(provider_id)
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "text/plain; charset=utf-8".parse().unwrap());
    headers.insert(
        CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    Ok((headers, bytes))
}

// ---------------------------------------------------------------------------
// Эндпоинты телеметрии БД и резервного копирования
// ---------------------------------------------------------------------------

/// Ответ на запрос статистики базы данных и резервных копий
#[derive(Debug, Serialize)]
pub struct DbStatsResponse {
    /// Статистика физического хранилища SQLite
    pub storage: DbStorageStats,
    /// Метаинформация о последней созданной резервной копии
    pub latest_backup: Option<BackupInfo>,
    /// Общее количество доступных бэкапов на сервере
    pub total_backups_count: usize,
}

/// GET /api/v1/system/db/stats
async fn db_stats_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<DbStatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let storage = state.db.get_storage_stats().await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    let backups = state.backup_service.list_backups().await.unwrap_or_default();
    let total_backups_count = backups.len();
    let latest_backup = backups.into_iter().next();

    Ok(Json(DbStatsResponse {
        storage,
        latest_backup,
        total_backups_count,
    }))
}

/// GET /api/v1/system/backup/list
async fn list_backups_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> Result<Json<Vec<BackupInfo>>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.view").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let backups = state.backup_service.list_backups().await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(backups))
}

/// Запрос на создание резервной копии
#[derive(Debug, Deserialize)]
pub struct CreateBackupRequest {
    /// Опциональный пользовательский тег (по умолчанию "manual")
    pub tag: Option<String>,
}

/// POST /api/v1/system/backup/create
async fn create_backup_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(payload): Json<Option<CreateBackupRequest>>,
) -> Result<Json<BackupInfo>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let tag = payload.and_then(|p| p.tag).unwrap_or_else(|| "manual".to_string());
    let info = state.backup_service.create_backup(&tag).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "backup.create",
            &info.filename,
            "SUCCESS",
            Some(&format!(
                "Created SQLite backup: {} ({} bytes, tag: {})",
                info.filename, info.size_bytes, info.tag
            )),
            None,
        )
        .await;

    Ok(Json(info))
}

/// GET /api/v1/system/backup/download/{filename}
async fn download_backup_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let path = state.backup_service.get_backup_path(&filename).map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::BAD_REQUEST),
            Json(e.to_api_response(locale)),
        )
    })?;

    let bytes = tokio::fs::read(&path).await.map_err(|e| {
        let app_err = AppError::internal(format!("Failed to read backup file: {}", e));
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(app_err.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "backup.download",
            &filename,
            "SUCCESS",
            Some(&format!("Downloaded backup file: {}", filename)),
            None,
        )
        .await;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "application/octet-stream".parse().unwrap());
    headers.insert(
        CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    Ok((headers, bytes))
}

/// Запрос на восстановление базы данных из существующего бэкапа
#[derive(Debug, Deserialize)]
pub struct RestoreBackupRequest {
    /// Имя файла резервной копии на сервере
    pub filename: String,
}

/// POST /api/v1/system/backup/restore
async fn restore_backup_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(payload): Json<RestoreBackupRequest>,
) -> Result<Json<RestoreResult>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let path = state.backup_service.get_backup_path(&payload.filename).map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::BAD_REQUEST),
            Json(e.to_api_response(locale)),
        )
    })?;

    let result = state.backup_service.restore_from_backup_file(&path).await.map_err(|e| {
        let _ = state.audit_service.log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "backup.restore",
            &payload.filename,
            "FAILURE",
            Some(&format!("Restore failed: {}", e)),
            None,
        );
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "backup.restore",
            &payload.filename,
            "SUCCESS",
            Some(&format!(
                "Restored database from {}. Pre-restore safety backup: {:?}",
                payload.filename, result.pre_restore_backup
            )),
            None,
        )
        .await;

    Ok(Json(result))
}

/// POST /api/v1/system/backup/upload-restore
async fn upload_restore_backup_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    mut multipart: Multipart,
) -> Result<Json<RestoreResult>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    let mut file_bytes: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || name == "backup" {
            let fname = field.file_name().unwrap_or("uploaded_backup.db").to_string();
            let data = field.bytes().await.map_err(|e| {
                let app_err = AppError::validation("file", format!("Failed to read uploaded file: {}", e));
                (StatusCode::BAD_REQUEST, Json(app_err.to_api_response(locale)))
            })?;
            file_bytes = Some((fname, data.to_vec()));
            break;
        }
    }

    let (orig_filename, bytes) = file_bytes.ok_or_else(|| {
        let app_err = AppError::validation("file", "No file uploaded in 'file' multipart field");
        (StatusCode::BAD_REQUEST, Json(app_err.to_api_response(locale)))
    })?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let temp_filename = format!("aethercore_backup_{}_upload.db", timestamp);
    let temp_path = state.backup_service.backup_dir().join(&temp_filename);

    let _ = tokio::fs::create_dir_all(state.backup_service.backup_dir()).await;
    tokio::fs::write(&temp_path, &bytes).await.map_err(|e| {
        let app_err = AppError::internal(format!("Failed to save uploaded backup file: {}", e));
        (StatusCode::INTERNAL_SERVER_ERROR, Json(app_err.to_api_response(locale)))
    })?;

    let restore_result = state
        .backup_service
        .restore_from_backup_file(&temp_path)
        .await
        .map_err(|e| {
            let _ = state.audit_service.log(
                Some(&claims.sub.to_string()),
                Some(&claims.username),
                "backup.upload_restore",
                &orig_filename,
                "FAILURE",
                Some(&format!("Upload restore failed: {}", e)),
                None,
            );
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_api_response(locale)),
            )
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "backup.upload_restore",
            &orig_filename,
            "SUCCESS",
            Some(&format!(
                "Restored database from uploaded file {} (saved as {}). Pre-restore backup: {:?}",
                orig_filename, temp_filename, restore_result.pre_restore_backup
            )),
            None,
        )
        .await;

    Ok(Json(restore_result))
}

/// DELETE /api/v1/system/backup/{filename}
async fn delete_backup_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(filename): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    if !claims.is_superuser {
        check_permission(&claims, "system.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    }

    state.backup_service.delete_backup(&filename).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "backup.delete",
            &filename,
            "SUCCESS",
            Some(&format!("Deleted backup file: {}", filename)),
            None,
        )
        .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "deleted": filename
    })))
}
