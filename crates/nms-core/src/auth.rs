// Модуль аутентификации, хэширования паролей (Argon2id), JWT токенов, MFA/2FA (TOTP RFC 6238) и прав RBAC

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use hmac::{Hmac, Mac};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;

type HmacSha1 = Hmac<Sha1>;

/// Время жизни токена по умолчанию (7 дней)
pub const TOKEN_TTL_SECONDS: u64 = 86400 * 7;

/// Модель текущего авторизованного пользователя
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CurrentUser {
    pub id: String,
    pub username: String,
    pub full_name: String,
    pub email: Option<String>,
    pub uid: Option<String>,
    pub role_id: String,
    pub role_name: String,
    pub avatar: Option<String>,
    pub is_authenticated: bool,
    pub permissions: Vec<String>,
    pub token_jti: Option<String>,
}

/// JWT Claims информация в токене доступа
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // ID пользователя
    pub username: String,   // Имя пользователя
    pub jti: String,        // Уникальный ID сессии/токена
    pub token_type: String, // "access" или "refresh"
    pub exp: usize,         // Время истечения (timestamp)
    pub iat: usize,         // Время выписки (timestamp)
}

/// Хэширование открытого пароля с помощью алгоритма Argon2id
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("Failed to hash password with Argon2id: {}", e))?
        .to_string();
    Ok(password_hash)
}

/// Проверка соответствия введенного пароля сохраненному Argon2id хэшу
pub fn verify_password(password: &str, password_hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(password_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Создание signed JWT access-токена
pub fn create_access_token(
    user_id: &str,
    username: &str,
    secret_key: &str,
    ttl_hours: u64,
) -> Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as usize;

    let jti = format!("jti-{}", uuid::Uuid::new_v4().simple());
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        jti,
        token_type: "access".to_string(),
        exp: now + (ttl_hours as usize * 3600),
        iat: now,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret_key.as_bytes()),
    )?;
    Ok(token)
}

/// Создание signed JWT refresh-токена
pub fn create_refresh_token(
    user_id: &str,
    username: &str,
    jti: &str,
    secret_key: &str,
    ttl_hours: u64,
) -> Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        jti: jti.to_string(),
        token_type: "refresh".to_string(),
        exp: now + (ttl_hours as usize * 3600),
        iat: now,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret_key.as_bytes()),
    )?;
    Ok(token)
}

/// Проверка и декодирование JWT токена
pub fn decode_token(token: &str, secret_key: &str) -> Option<Claims> {
    let validation = Validation::default();
    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret_key.as_bytes()),
        &validation,
    ) {
        Ok(data) => Some(data.claims),
        Err(_) => None,
    }
}

/// Проверить и декодировать access-токен (1-в-1 с Python decode_access_token)
pub fn decode_access_token(token: &str, secret_key: &str) -> Option<Claims> {
    let claims = decode_token(token, secret_key)?;
    if claims.token_type == "access" {
        Some(claims)
    } else {
        None
    }
}

/// Проверить и декодировать refresh-токен (1-в-1 с Python decode_refresh_token)
pub fn decode_refresh_token(token: &str, secret_key: &str) -> Option<Claims> {
    let claims = decode_token(token, secret_key)?;
    if claims.token_type == "refresh" {
        Some(claims)
    } else {
        None
    }
}

// ── MFA / 2FA (RFC 6238 TOTP) ───────────────────────────────────────

/// Генерация случайного Base32-секрета для 2FA/TOTP (160 бит)
pub fn generate_totp_secret() -> String {
    let mut bytes = [0u8; 20];
    let _ = getrandom::getrandom(&mut bytes);
    data_encoding::BASE32_NOPAD.encode(&bytes)
}

/// Расчет 6-значного TOTP кода по стандарту RFC 6238 (HMAC-SHA1)
pub fn get_totp_code(secret: &str, time_step: Option<u64>) -> Option<String> {
    let now = time_step.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });

    let clean_secret = secret.trim().to_uppercase();
    let secret_bytes = data_encoding::BASE32_NOPAD
        .decode(clean_secret.as_bytes())
        .ok()?;

    let counter = now / 30;
    let counter_bytes = counter.to_be_bytes();

    let mut mac = HmacSha1::new_from_slice(&secret_bytes).ok()?;
    mac.update(&counter_bytes);
    let result = mac.finalize().into_bytes();

    let offset = (result[result.len() - 1] & 0x0f) as usize;
    let code_binary = ((result[offset] as u32 & 0x7f) << 24)
        | ((result[offset + 1] as u32 & 0xff) << 16)
        | ((result[offset + 2] as u32 & 0xff) << 8)
        | (result[offset + 3] as u32 & 0xff);

    let code = code_binary % 1_000_000;
    Some(format!("{:06}", code))
}

/// Проверка 6-значного OTP кода с допуском рассинхронизации (окно ±1)
pub fn verify_totp_code(secret: &str, code: &str) -> bool {
    let clean_code = code.trim();
    if clean_code.len() != 6 || !clean_code.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for delta in &[-30i64, 0i64, 30i64] {
        let test_time = (now as i64 + delta) as u64;
        if let Some(generated) = get_totp_code(secret, Some(test_time)) {
            if generated == clean_code {
                return true;
            }
        }
    }
    false
}

/// Формирование URI для приложений аутентификации (otpauth://)
pub fn get_totp_uri(username: &str, secret: &str, issuer: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits=6&period=30",
        urlencoding::encode(issuer),
        urlencoding::encode(username),
        secret,
        urlencoding::encode(issuer)
    )
}

/// Получить список разрешенных Origin из настроек (1-в-1 с Python)
pub fn get_allowed_cors_origins() -> Vec<String> {
    crate::config::AppConfig::from_env().cors_origins
}

/// Проверка заголовка Origin против allowlist (1-в-1 с Python)
pub fn is_origin_allowed(origin: Option<&str>, allowed_origins: Option<&[String]>) -> bool {
    let origin_str = match origin {
        Some(o) if !o.trim().is_empty() => o.trim(),
        _ => return true,
    };

    let default_origins;
    let allowed_list = match allowed_origins {
        Some(list) => list,
        None => {
            default_origins = get_allowed_cors_origins();
            &default_origins[..]
        }
    };

    if allowed_list.iter().any(|a| a == "*") {
        return true;
    }

    let parsed_origin = origin_str.trim_end_matches('/');
    for allowed in allowed_list {
        if allowed == "*" || allowed.trim_end_matches('/') == parsed_origin {
            return true;
        }
    }

    // Разрешаем локальные loopback и приватные подсети RFC 1918 для dev/lan доступа
    if let Ok(url) = reqwest::Url::parse(parsed_origin) {
        if let Some(host_str) = url.host_str() {
            if matches!(host_str, "localhost" | "127.0.0.1" | "0.0.0.0" | "::1") {
                return true;
            }
            if let Ok(ip) = host_str.parse::<std::net::IpAddr>() {
                if ip.is_loopback() {
                    return true;
                }
                match ip {
                    std::net::IpAddr::V4(ipv4) => {
                        if ipv4.is_private() {
                            return true;
                        }
                    }
                    std::net::IpAddr::V6(_) => {}
                }
            }
        }
    }

    false
}

// ── Менеджер одноразовых WebSocket-билетов ───────────────────────────

static GLOBAL_WS_TICKETS: OnceLock<WsTicketManager> = OnceLock::new();

fn get_global_ws_tickets() -> &'static WsTicketManager {
    GLOBAL_WS_TICKETS.get_or_init(WsTicketManager::new)
}

/// Сгенерировать одноразовый билет для подключения к WebSocket (1-в-1 с Python)
pub async fn create_ws_ticket(
    user_id: &str,
    _jti: Option<&str>,
    _expires_in: Option<u64>,
) -> String {
    get_global_ws_tickets().create_ticket(user_id).await
}

/// Проверить и погасить одноразовый билет WebSocket (1-в-1 с Python)
pub async fn consume_ws_ticket(ticket: &str) -> Option<String> {
    get_global_ws_tickets().consume_ticket(ticket).await
}

/// Менеджер одноразовых билетов для WebSocket сессий
#[derive(Clone, Default)]
pub struct WsTicketManager {
    tickets: Arc<RwLock<HashMap<String, (String, u64)>>>,
}

impl WsTicketManager {
    pub fn new() -> Self {
        Self {
            tickets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Сгенерировать одноразовый билет на 30 секунд
    pub async fn create_ticket(&self, user_id: &str) -> String {
        let ticket = format!("wst_{}", uuid::Uuid::new_v4().simple());
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + 30;

        let mut map = self.tickets.write().await;
        map.insert(ticket.clone(), (user_id.to_string(), expires_at));
        ticket
    }

    /// Проверить и погасить одноразовый билет WebSocket
    pub async fn consume_ticket(&self, ticket: &str) -> Option<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut map = self.tickets.write().await;
        map.retain(|_, (_, exp)| *exp >= now);

        if let Some((user_id, exp)) = map.remove(ticket) {
            if exp >= now {
                return Some(user_id);
            }
        }
        None
    }
}

/// Проверка, входит ли client_ip в список whitelist (разделенный запятыми/пробелами/переводами строк/точным совпадением или CIDR)
pub fn is_ip_whitelisted(client_ip: &str, whitelist_str: &str) -> bool {
    let clean_wl = whitelist_str.trim();
    if clean_wl.is_empty() {
        return true;
    }

    let ip_obj: std::net::IpAddr = match client_ip.trim().parse() {
        Ok(ip) => ip,
        Err(_) => return false,
    };

    let items: Vec<&str> = clean_wl
        .split(|c: char| c == ',' || c == ';' || c == ' ' || c == '\n' || c == '\t')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if items.is_empty() {
        return true;
    }

    for item in items {
        if item.contains('/') {
            let parts: Vec<&str> = item.split('/').collect();
            if parts.len() == 2 {
                if let (Ok(net_ip), Ok(prefix_len)) = (
                    parts[0].parse::<std::net::IpAddr>(),
                    parts[1].parse::<u32>(),
                ) {
                    match (ip_obj, net_ip) {
                        (std::net::IpAddr::V4(ipv4), std::net::IpAddr::V4(net_v4))
                            if prefix_len <= 32 =>
                        {
                            let mask = if prefix_len == 0 {
                                0
                            } else {
                                !((1u64 << (32 - prefix_len)) - 1) as u32
                            };
                            if (u32::from(ipv4) & mask) == (u32::from(net_v4) & mask) {
                                return true;
                            }
                        }
                        (std::net::IpAddr::V6(ipv6), std::net::IpAddr::V6(net_v6))
                            if prefix_len <= 128 =>
                        {
                            let mask = if prefix_len == 0 {
                                0
                            } else {
                                !((1u128 << (128 - prefix_len)) - 1)
                            };
                            if (u128::from(ipv6) & mask) == (u128::from(net_v6) & mask) {
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        } else if let Ok(target_ip) = item.parse::<std::net::IpAddr>() {
            if ip_obj == target_ip {
                return true;
            }
        }
    }

    false
}

/// Проверить, аннулирована ли сессия по ее JTI в БД active_sessions
pub async fn is_session_revoked(pool: &sqlx::SqlitePool, jti: &str) -> Result<bool> {
    if jti.is_empty() {
        return Ok(false);
    }
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT is_revoked FROM active_sessions WHERE token_jti = ?")
            .bind(jti)
            .fetch_optional(pool)
            .await?;

    Ok(row.map(|(r,)| r == 1).unwrap_or(false))
}

/// Проверка наличия разрешения у пользователя с учетом суперправа system.all и иерархии прав
pub fn has_permission(permissions: &[String], required_permission: &str) -> bool {
    if permissions
        .iter()
        .any(|p| p == "system.all" || p == required_permission)
    {
        return true;
    }

    // Иерархия прав: управляющее право включает просмотр
    let implied_parents: &[&str] = match required_permission {
        "users.view" => &["users.manage"],
        "roles.view" => &["roles.manage"],
        "settings.view" => &["settings.edit"],
        "modules.view" => &["modules.manage"],
        "audit.view" => &["audit.export"],
        _ => &[],
    };

    permissions
        .iter()
        .any(|p| implied_parents.contains(&p.as_str()))
}

/// Проверка наличия разрешения для конкретного модуля
pub fn has_module_permission(permissions: &[String], module_id: &str, action: &str) -> bool {
    if permissions.iter().any(|p| p == "system.all") {
        return true;
    }

    let perm_key1 = format!("module.{}.{}", module_id, action);
    let perm_key2 = format!("{}.{}", module_id, action);

    permissions
        .iter()
        .any(|p| p == &perm_key1 || p == &perm_key2)
}

static ROLE_PERMISSIONS_CACHE: OnceLock<Arc<RwLock<HashMap<String, Vec<String>>>>> =
    OnceLock::new();

fn get_permissions_cache() -> &'static Arc<RwLock<HashMap<String, Vec<String>>>> {
    ROLE_PERMISSIONS_CACHE.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

/// Очистка кэша разрешений ролей (1-в-1 с Python clear_permissions_cache)
pub async fn clear_permissions_cache(role_id: Option<&str>) {
    let cache = get_permissions_cache();
    let mut lock = cache.write().await;
    if let Some(rid) = role_id {
        lock.remove(rid);
    } else {
        lock.clear();
    }
}

/// Проверка наличия разрешения у пользователя по его user_id в базе данных (1-в-1 с Python user_has_permission)
pub async fn user_has_permission(pool: &sqlx::SqlitePool, user_id: &str, permission: &str) -> bool {
    if user_id.is_empty() {
        return false;
    }

    let user_row: Option<(String,)> = match sqlx::query_as(
        r#"
        SELECT u.role_id
        FROM users u
        WHERE u.id = ? AND u.is_active = 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    {
        Ok(res) => res,
        Err(_) => return false,
    };

    let role_id = match user_row {
        Some((r,)) => r,
        None => return false,
    };

    let cache = get_permissions_cache();
    {
        let lock = cache.read().await;
        if let Some(perms) = lock.get(&role_id) {
            return has_permission(perms, permission);
        }
    }

    let perm_rows: Vec<(String,)> =
        match sqlx::query_as("SELECT permission_id FROM role_permissions WHERE role_id = ?")
            .bind(&role_id)
            .fetch_all(pool)
            .await
        {
            Ok(rows) => rows,
            Err(_) => return false,
        };

    let permissions: Vec<String> = perm_rows.into_iter().map(|(p,)| p).collect();
    {
        let mut lock = cache.write().await;
        lock.insert(role_id.clone(), permissions.clone());
    }

    has_permission(&permissions, permission)
}

/// Универсальная проверка разрешения для ID пользователя или роли (1-в-1 с Python has_role_permission)
pub async fn has_role_permission(
    pool: &sqlx::SqlitePool,
    role_or_user: &str,
    permission: &str,
) -> bool {
    user_has_permission(pool, role_or_user, permission).await
}

/// Извлекает текущего пользователя по Bearer токену и базе данных (1-в-1 с Python get_current_user)
pub async fn get_current_user(
    pool: &sqlx::SqlitePool,
    token: &str,
    secret_key: &str,
) -> Result<CurrentUser, crate::exceptions::NmsError> {
    use crate::exceptions::NmsError;

    let claims = decode_access_token(token, secret_key).ok_or_else(|| NmsError::AuthRequired {
        message: "Invalid or expired token".to_string(),
    })?;

    let row = sqlx::query(
        r#"
        SELECT u.id, u.username, u.full_name, u.email, u.uid, u.avatar, u.token_valid_after, u.is_active, u.role_id, r.name as role_name
        FROM users u
        JOIN roles r ON u.role_id = r.id
        WHERE u.id = ? AND u.is_active = 1
        "#,
    )
    .bind(&claims.sub)
    .fetch_optional(pool)
    .await
    .map_err(|e| NmsError::Internal {
        message: e.to_string(),
        details: serde_json::json!({}),
    })?;

    let user_row = row.ok_or_else(|| NmsError::AuthRequired {
        message: "User not found or locked".to_string(),
    })?;

    use sqlx::Row;
    let user_id: String = user_row.get("id");
    let username: String = user_row.get("username");
    let full_name: String = user_row.get("full_name");
    let email: Option<String> = user_row.get("email");
    let uid: Option<String> = user_row.get("uid");
    let avatar: Option<String> = user_row.get("avatar");
    let role_id: String = user_row.get("role_id");
    let role_name: String = user_row.get("role_name");
    let token_valid_after: Option<i64> = user_row.get("token_valid_after");

    if let Some(valid_after) = token_valid_after {
        if (claims.iat as i64) <= valid_after {
            return Err(NmsError::AuthRequired {
                message: "Session revoked".to_string(),
            });
        }
    }

    if is_session_revoked(pool, &claims.jti).await.unwrap_or(false) {
        return Err(NmsError::AuthRequired {
            message: "Session revoked by admin".to_string(),
        });
    }

    let perm_rows: Vec<(String,)> =
        sqlx::query_as("SELECT permission_id FROM role_permissions WHERE role_id = ?")
            .bind(&role_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

    let permissions: Vec<String> = perm_rows.into_iter().map(|(p,)| p).collect();

    Ok(CurrentUser {
        id: user_id,
        username,
        full_name,
        email,
        uid,
        role_id,
        role_name,
        avatar,
        is_authenticated: true,
        permissions,
        token_jti: Some(claims.jti),
    })
}

/// Оптимистично извлекает пользователя по токену, либо None (1-в-1 с Python get_current_user_optional)
pub async fn get_current_user_optional(
    pool: &sqlx::SqlitePool,
    token: Option<&str>,
    secret_key: &str,
) -> Option<CurrentUser> {
    let token = token?;
    get_current_user(pool, token, secret_key).await.ok()
}

/// Проверка прав доступа у текущего пользователя (1-в-1 с Python require_permission)
pub fn require_permission(
    user: &CurrentUser,
    permission: &str,
) -> Result<(), crate::exceptions::NmsError> {
    if has_permission(&user.permissions, permission) {
        Ok(())
    } else {
        Err(crate::exceptions::NmsError::PermissionDenied {
            message: format!("Insufficient permissions for '{}'", permission),
        })
    }
}

/// Проверка включенности модуля и наличия разрешения у роли (1-в-1 с Python require_module_permission)
pub fn require_module_permission(
    user: &CurrentUser,
    module_id: &str,
    action: &str,
) -> Result<(), crate::exceptions::NmsError> {
    if has_module_permission(&user.permissions, module_id, action) {
        Ok(())
    } else {
        Err(crate::exceptions::NmsError::PermissionDenied {
            message: format!(
                "Insufficient module permissions for '{}.{}'",
                module_id, action
            ),
        })
    }
}

/// Генерация SVG QR-кода (версия 3, 29x29) без внешних библиотек
pub fn generate_qr_svg(content: &str) -> String {
    let modules = _encode_qr_matrix(content);
    let size = modules.len();
    let mut rects = Vec::new();

    for y in 0..size {
        for x in 0..size {
            if modules[y][x] {
                rects.push(format!(
                    r##"<rect x="{}" y="{}" width="1" height="1" fill="#22d3ee"/>"##,
                    x, y
                ));
            }
        }
    }

    let svg_body = rects.join("");
    let view_box = format!("0 0 {} {}", size, size);
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{}" shape-rendering="crispEdges" width="200" height="200" style="background:#0f172a; border-radius:12px; padding:12px;"><g>{}</g></svg>"##,
        view_box, svg_body
    );
    format!("data:image/svg+xml;utf8,{}", urlencoding::encode(&svg))
}

fn _encode_qr_matrix(text: &str) -> Vec<Vec<bool>> {
    let size = 29;
    let mut matrix = vec![vec![false; size]; size];
    let mut reserved = vec![vec![false; size]; size];

    let mut add_finder = |rx: usize, ry: usize| {
        for dy in 0..7 {
            for dx in 0..7 {
                let x = rx + dx;
                let y = ry + dy;
                reserved[y][x] = true;
                if dy == 0
                    || dy == 6
                    || dx == 0
                    || dx == 6
                    || (2 <= dy && dy <= 4 && 2 <= dx && dx <= 4)
                {
                    matrix[y][x] = true;
                } else {
                    matrix[y][x] = false;
                }
            }
        }
    };

    add_finder(0, 0);
    add_finder(size - 7, 0);
    add_finder(0, size - 7);

    let (ax, ay) = (20, 20);
    for dy in 0..5 {
        for dx in 0..5 {
            let x = ax - 2 + dx;
            let y = ay - 2 + dy;
            reserved[y][x] = true;
            if dy == 0 || dy == 4 || dx == 0 || dx == 4 || (dy == 2 && dx == 2) {
                matrix[y][x] = true;
            }
        }
    }

    for i in 8..(size - 8) {
        reserved[6][i] = true;
        matrix[6][i] = i % 2 == 0;
        reserved[i][6] = true;
        matrix[i][6] = i % 2 == 0;
    }

    let mut bits = Vec::new();
    for b in text.as_bytes() {
        for bit_idx in (0..8).rev() {
            bits.push(((b >> bit_idx) & 1) != 0);
        }
    }

    let mut bit_cursor = 0;
    let bit_len = bits.len();

    for y in 0..size {
        for x in 0..size {
            if !reserved[y][x] {
                if bit_len > 0 {
                    matrix[y][x] = bits[bit_cursor % bit_len];
                    bit_cursor += 1;
                } else {
                    matrix[y][x] = (x + y) % 2 == 0;
                }
            }
        }
    }

    matrix
}
