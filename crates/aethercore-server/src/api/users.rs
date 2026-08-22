//! # Эндпоинты управления пользователями (`/api/v1/users`)
//!
//! Предоставляет HTTP CRUD API для:
//! - Получения списка всех пользователей (`GET /api/v1/users`).
//! - Создания нового пользователя (`POST /api/v1/users`).
//! - Получения профиля пользователя по ID (`GET /api/v1/users/{id}`).
//! - Обновления данных пользователя (`PUT /api/v1/users/{id}`).
//! - Удаления пользователя (`DELETE /api/v1/users/{id}`).

use crate::middleware::{AuthUser, RequestLocale};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use aethercore_common::error::{AppError, ErrorResponse};
use aethercore_common::models::user::{CreateUserDto, UpdateUserDto, UserResponseDto};
use aethercore_core::auth::check_permission;
use uuid::Uuid;

/// Создать вложенный роутер управления пользователями `/users`
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users_handler))
        .route("/", post(create_user_handler))
        .route("/{id}", get(get_user_handler))
        .route("/{id}", put(update_user_handler))
        .route("/{id}", delete(delete_user_handler))
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

/// GET /api/v1/users
///
/// Получить список всех пользователей системы с их назначенными ролями и агрегированными правами доступа.
///
/// # Требуемые права RBAC
/// * `users.view` (или права суперпользователя).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `claims` — Данные авторизованного пользователя [`AuthUser`].
///
/// # Возвращаемое значение
/// Список DTO пользователей [`UserResponseDto`].
///
/// # Ошибки
/// * [`StatusCode::FORBIDDEN`] — недостаточно прав доступа.
async fn list_users_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
) -> ApiResult<Vec<UserResponseDto>> {
    check_permission(&claims, "users.view").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    let users = state.user_service.list_users().await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(e.to_api_response(locale)),
        )
    })?;

    let dtos = users.into_iter().map(UserResponseDto::from).collect();
    Ok(Json(dtos))
}

/// POST /api/v1/users
///
/// Создать нового пользователя системы с хэшированием пароля Argon2id и привязкой ролей.
///
/// # Требуемые права RBAC
/// * `users.manage` (или права суперпользователя).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `claims` — Данные авторизованного пользователя [`AuthUser`].
/// * `dto` — Тело JSON-запроса с данными создаваемого пользователя [`CreateUserDto`].
///
/// # Возвращаемое значение
/// Профиль созданного пользователя [`UserResponseDto`].
///
/// # Ошибки
/// * [`StatusCode::FORBIDDEN`] — недостаточно прав доступа или попытка создать суперпользователя без прав суперпользователя.
/// * [`StatusCode::BAD_REQUEST`] — невалидный логин или пароль.
/// * [`StatusCode::CONFLICT`] — пользователь с таким логином уже существует.
async fn create_user_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Json(dto): Json<CreateUserDto>,
) -> ApiResult<UserResponseDto> {
    check_permission(&claims, "users.manage").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    let is_creating_superuser = dto.is_superuser == Some(true)
        || dto.roles.as_ref().map_or(false, |r| r.contains(&"superuser".to_string()));

    if is_creating_superuser && !claims.is_superuser {
        return Err((
            StatusCode::FORBIDDEN,
            Json(
                AppError::forbidden("Only superusers can create or assign superuser role")
                    .to_api_response(locale),
            ),
        ));
    }

    let user = state.user_service.create_user(dto).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::BAD_REQUEST),
            Json(e.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "users.create",
            &format!("users/{}", user.id),
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(UserResponseDto::from(user)))
}

/// GET /api/v1/users/{id}
///
/// Получить профиль пользователя по его уникальному идентификатору (UUID).
///
/// # Требуемые права RBAC
/// * `users.view` (или права суперпользователя).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `claims` — Данные авторизованного пользователя [`AuthUser`].
/// * `id` — Идентификатор искомого пользователя [`Uuid`].
///
/// # Возвращаемое значение
/// Профиль пользователя [`UserResponseDto`].
///
/// # Ошибки
/// * [`StatusCode::FORBIDDEN`] — недостаточно прав доступа.
/// * [`StatusCode::NOT_FOUND`] — пользователь не найден.
async fn get_user_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<UserResponseDto> {
    check_permission(&claims, "users.view").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    let user = state.user_service.get_user_by_id(id).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::NOT_FOUND),
            Json(e.to_api_response(locale)),
        )
    })?;

    Ok(Json(UserResponseDto::from(user)))
}

/// PUT /api/v1/users/{id}
///
/// Обновить параметры пользователя (ФИО, email, пароль, статус активности, флаг суперпользователя, роли).
///
/// # Требуемые права RBAC
/// * `users.manage` (или `self-update` для смены собственного пароля и данных профиля).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `claims` — Данные авторизованного пользователя [`AuthUser`].
/// * `id` — Идентификатор обновляемого пользователя [`Uuid`].
/// * `dto` — Тело JSON-запроса с обновляемыми полями [`UpdateUserDto`].
///
/// # Возвращаемое значение
/// Обновленный профиль пользователя [`UserResponseDto`].
///
/// # Ошибки
/// * [`StatusCode::FORBIDDEN`] — попытка редактирования профиля суперпользователя без прав суперпользователя или несанкционированная эскалация ролей.
/// * [`StatusCode::NOT_FOUND`] — пользователь не найден.
/// * [`StatusCode::BAD_REQUEST`] — ошибка валидации параметров или нарушение квот.
async fn update_user_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateUserDto>,
) -> ApiResult<UserResponseDto> {
    let is_self_update = claims.sub == id;
    if !is_self_update {
        check_permission(&claims, "users.manage").map_err(|e| {
            (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
        })?;
    } else if !claims.is_superuser {
        // Обычный пользователь не может сам себе менять роли, активность или флаг суперпользователя
        if dto.roles.is_some() || dto.is_active.is_some() || dto.is_superuser.is_some() {
            return Err((
                StatusCode::FORBIDDEN,
                Json(
                    AppError::forbidden("Cannot change your own roles, activity or superuser status")
                        .to_api_response(locale),
                ),
            ));
        }
    }

    let target_user = state.user_service.get_user_by_id(id).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::NOT_FOUND),
            Json(e.to_api_response(locale)),
        )
    })?;

    // Только суперпользователь может редактировать профиль другого суперпользователя
    if target_user.is_superuser && claims.sub != target_user.id && !claims.is_superuser {
        return Err((
            StatusCode::FORBIDDEN,
            Json(
                AppError::forbidden("Only superusers can modify other superuser accounts")
                    .to_api_response(locale),
            ),
        ));
    }

    // Только суперпользователь может повысить пользователя до суперпользователя
    let is_promoting_to_superuser = dto.is_superuser == Some(true)
        || dto.roles.as_ref().map_or(false, |r| r.contains(&"superuser".to_string()));

    if !target_user.is_superuser && is_promoting_to_superuser && !claims.is_superuser {
        return Err((
            StatusCode::FORBIDDEN,
            Json(
                AppError::forbidden("Only superusers can promote accounts to superuser")
                    .to_api_response(locale),
            ),
        ));
    }

    // При самостоятельной смене пароля требуем ввод текущего пароля (если это не первый принудительный вход)
    let is_password_changing = dto.password.as_ref().map_or(false, |p| !p.trim().is_empty());
    if is_self_update && is_password_changing && !target_user.must_change_password {
        let has_current = dto
            .current_password
            .as_ref()
            .map_or(false, |p| !p.trim().is_empty());
        if !has_current {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(
                    AppError::validation(
                        "current_password",
                        "Current password is required to change password",
                    )
                    .to_api_response(locale),
                ),
            ));
        }
    }

    let user = state
        .user_service
        .update_user(id, dto)
        .await
        .map_err(|e| {
            (
                StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::BAD_REQUEST),
                Json(e.to_api_response(locale)),
            )
        })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "users.update",
            &format!("users/{}", user.id),
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(UserResponseDto::from(user)))
}

/// DELETE /api/v1/users/{id}
///
/// Удалить учетную запись пользователя из системы. Запрещено удалять собственный аккаунт.
///
/// # Требуемые права RBAC
/// * `users.manage` (для суперпользователей — право суперпользователя).
///
/// # Аргументы
/// * `state` — Разделяемое состояние сервера [`AppState`].
/// * `locale` — Локаль запроса [`RequestLocale`].
/// * `claims` — Данные авторизованного пользователя [`AuthUser`].
/// * `id` — Идентификатор удаляемого пользователя [`Uuid`].
///
/// # Возвращаемое значение
/// JSON-объект `{"success": true}`.
///
/// # Ошибки
/// * [`StatusCode::FORBIDDEN`] — недостаточно прав или попытка удалить суперпользователя не-суперпользователем.
/// * [`StatusCode::BAD_REQUEST`] — попытка удалить свой собственный аккаунт или последнего суперпользователя.
/// * [`StatusCode::NOT_FOUND`] — пользователь не найден.
async fn delete_user_handler(
    State(state): State<AppState>,
    RequestLocale(locale): RequestLocale,
    AuthUser(claims): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<serde_json::Value> {
    check_permission(&claims, "users.manage").map_err(|e| {
        (StatusCode::FORBIDDEN, Json(e.to_api_response(locale)))
    })?;

    // Нельзя удалить самого себя
    if claims.sub == id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                AppError::bad_request("Cannot delete your own user account")
                    .to_api_response(locale),
            ),
        ));
    }

    let target_user = state.user_service.get_user_by_id(id).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::NOT_FOUND),
            Json(e.to_api_response(locale)),
        )
    })?;

    // Только суперпользователь может удалять суперпользователей
    if target_user.is_superuser && !claims.is_superuser {
        return Err((
            StatusCode::FORBIDDEN,
            Json(
                AppError::forbidden("Only superusers can delete superuser accounts")
                    .to_api_response(locale),
            ),
        ));
    }

    state.user_service.delete_user(id).await.map_err(|e| {
        (
            StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::BAD_REQUEST),
            Json(e.to_api_response(locale)),
        )
    })?;

    let _ = state
        .audit_service
        .log(
            Some(&claims.sub.to_string()),
            Some(&claims.username),
            "users.delete",
            &format!("users/{}", id),
            "success",
            None,
            None,
        )
        .await;

    Ok(Json(serde_json::json!({"success": true})))
}
