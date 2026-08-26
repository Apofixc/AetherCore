//! # Сессия и управление состоянием WebSocket-клиента
//!
//! Инкапсулирует данные авторизации, генератор монотонных номеров `seq`,
//! формат сериализации, динамические фильтры потока, Rate-limiting и валидацию прав доступа (RBAC).

use aethercore_common::models::events::{EventMessage, EventPriority, EventType};
use aethercore_common::models::user::JwtClaims;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
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
    /// Время подключения в секундах (UNIX timestamp)
    connected_at: u64,
    /// Настройки фильтрации событий
    min_priority: Arc<RwLock<Option<EventPriority>>>,
    event_types_filter: Arc<RwLock<Option<Vec<EventType>>>>,
    source_filter: Arc<RwLock<Option<String>>>,
    /// Rate-limiting: секунда текущего окна и счетчик команд
    current_sec: AtomicU64,
    commands_this_sec: AtomicU32,
}

impl WsSession {
    /// Создать новую сессию подключения
    pub fn new(
        initial_claims: Option<JwtClaims>,
        initial_format: crate::ws::types::WsCodecFormat,
        client_ip: String,
    ) -> Self {
        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            seq_counter: AtomicU64::new(1),
            claims: Arc::new(RwLock::new(initial_claims)),
            format: Arc::new(RwLock::new(initial_format)),
            client_ip,
            connected_at: now_sec,
            min_priority: Arc::new(RwLock::new(None)),
            event_types_filter: Arc::new(RwLock::new(None)),
            source_filter: Arc::new(RwLock::new(None)),
            current_sec: AtomicU64::new(now_sec),
            commands_this_sec: AtomicU32::new(0),
        }
    }

    /// Получить следующий монотонный порядковый номер события
    pub fn next_seq(&self) -> u64 {
        self.seq_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Проверить лимит входящих команд (Rate Limiting)
    pub fn check_rate_limit(&self, max_per_sec: u32) -> bool {
        if max_per_sec == 0 {
            return true;
        }

        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let old_sec = self.current_sec.swap(now_sec, Ordering::Relaxed);
        if old_sec != now_sec {
            self.commands_this_sec.store(1, Ordering::Relaxed);
            true
        } else {
            let count = self.commands_this_sec.fetch_add(1, Ordering::Relaxed);
            count < max_per_sec
        }
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

    /// Установить фильтры потока событий на клиенте
    pub async fn set_filters(
        &self,
        min_prio: Option<EventPriority>,
        types: Option<Vec<EventType>>,
        source: Option<String>,
    ) {
        *self.min_priority.write().await = min_prio;
        *self.event_types_filter.write().await = types;
        *self.source_filter.write().await = source;
    }

    /// IP-адрес клиента
    pub fn client_ip(&self) -> &str {
        &self.client_ip
    }

    /// Время непрерывного подключения в секундах
    pub fn uptime_secs(&self) -> u64 {
        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now_sec.saturating_sub(self.connected_at)
    }

    /// Проверить, должно ли событие быть отправлено клиенту с учетом RBAC и пользовательских фильтров
    pub async fn should_deliver_event(&self, event: &EventMessage) -> bool {
        // 1. Проверка RBAC прав доступа к топику
        if !self.can_read_topic(&event.topic).await {
            return false;
        }

        // 2. Фильтр минимального приоритета
        if let Some(min_prio) = *self.min_priority.read().await {
            if event.priority < min_prio {
                return false;
            }
        }

        // 3. Фильтр типов событий
        if let Some(ref types) = *self.event_types_filter.read().await {
            if !types.is_empty() && !types.contains(&event.event_type) {
                return false;
            }
        }

        // 4. Фильтр по префиксу источника
        if let Some(ref src_filter) = *self.source_filter.read().await {
            if !event.source.starts_with(src_filter) {
                return false;
            }
        }

        true
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
