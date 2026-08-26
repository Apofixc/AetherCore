//! # Сессия и управление состоянием WebSocket-клиента
//!
//! Инкапсулирует данные авторизации, генератор монотонных номеров `seq`,
//! формат сериализации и валидацию прав доступа (RBAC) к топикам шины.

use aethercore_common::models::user::JwtClaims;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Сессионное состояние активного WebSocket-подключения
pub struct WsSession {
    /// Монотонный инкрементный счетчик исходящих сообщений для предотвращения Out-of-Order
    seq_counter: AtomicU64,
    /// Утвержденные клеймы JWT авторизованного пользователя
    claims: Arc<RwLock<Option<JwtClaims>>>,
    /// Выбранный кодек данных (JSON или MessagePack)
    format: Arc<RwLock<crate::ws::types::WsCodecFormat>>,
    /// IP-адрес клиента
    client_ip: String,
}

impl WsSession {
    /// Создать новую сессию подключения
    pub fn new(
        initial_claims: Option<JwtClaims>,
        initial_format: crate::ws::types::WsCodecFormat,
        client_ip: String,
    ) -> Self {
        Self {
            seq_counter: AtomicU64::new(1),
            claims: Arc::new(RwLock::new(initial_claims)),
            format: Arc::new(RwLock::new(initial_format)),
            client_ip,
        }
    }

    /// Получить следующий монотонный порядковый номер события
    pub fn next_seq(&self) -> u64 {
        self.seq_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Проверить, авторизован ли сокет в данный момент
    pub async fn is_authenticated(&self) -> bool {
        self.claims.read().await.is_some()
    }

    /// Получить снимок текущих прав пользователя
    pub async fn get_claims(&self) -> Option<JwtClaims> {
        self.claims.read().await.clone()
    }

    /// Установить новые клеймы пользователя (после успешной In-Band авторизации)
    pub async fn set_claims(&self, claims: JwtClaims) {
        let mut guard = self.claims.write().await;
        *guard = Some(claims);
    }

    /// Сбросить авторизацию (при Logout)
    pub async fn clear_claims(&self) {
        let mut guard = self.claims.write().await;
        *guard = None;
    }

    /// Получить текущий кодек
    pub async fn get_format(&self) -> crate::ws::types::WsCodecFormat {
        *self.format.read().await
    }

    /// Установить формат кодека
    pub async fn set_format(&self, format: crate::ws::types::WsCodecFormat) {
        let mut guard = self.format.write().await;
        *guard = format;
    }

    /// IP-адрес клиента
    pub fn client_ip(&self) -> &str {
        &self.client_ip
    }

    /// Проверить право пользователя на чтение/подписку на топик (RBAC Topic Guard)
    pub async fn can_read_topic(&self, topic: &str) -> bool {
        let guard = self.claims.read().await;
        let claims = match guard.as_ref() {
            Some(c) => c,
            None => return false,
        };

        if claims.is_superuser {
            return true;
        }

        // Защита системных приватных топиков ядра
        if topic.starts_with("system.auth") || topic.starts_with("core.security") {
            return claims.permissions.iter().any(|p| p == "system.manage" || p == "system.view");
        }

        // Топики пользователей
        if topic.starts_with("users.") {
            return claims.permissions.iter().any(|p| p == "users.view" || p == "users.manage");
        }

        // Топики модулей/плагинов
        if topic.starts_with("plugin.") || topic.starts_with("modules.") {
            return claims.permissions.iter().any(|p| p == "modules.view" || p == "modules.manage" || p == "events.view");
        }

        // По умолчанию для чтения открытых событий требуется events.view
        claims.permissions.iter().any(|p| p == "events.view" || p == "system.view")
    }

    /// Проверить право пользователя на публикацию в топик (RBAC Publish Guard)
    pub async fn can_write_topic(&self, topic: &str) -> bool {
        let guard = self.claims.read().await;
        let claims = match guard.as_ref() {
            Some(c) => c,
            None => return false,
        };

        if claims.is_superuser {
            return true;
        }

        // В системные топики ядра могут писать только администраторы
        if topic.starts_with("system.") || topic.starts_with("core.") {
            return claims.permissions.iter().any(|p| p == "system.manage");
        }

        // В топики пользователей могут писать пользователи с правом users.manage
        if topic.starts_with("users.") {
            return claims.permissions.iter().any(|p| p == "users.manage");
        }

        // В топики плагинов могут публиковать события пользователи с modules.manage или events.view
        if topic.starts_with("plugin.") {
            return claims.permissions.iter().any(|p| p == "modules.manage" || p == "events.view");
        }

        // По умолчанию для публикации требуется events.view
        claims.permissions.iter().any(|p| p == "events.view")
    }
}
