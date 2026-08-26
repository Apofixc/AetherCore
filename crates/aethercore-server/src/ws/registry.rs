//! # Реестр активных WebSocket-соединений шлюза
//!
//! Отслеживает активные подключения, собирает телеметрию (IP, uptime, username, топики)
//! и обеспечивает централизованный учет для мониторинга и безопасности.

use crate::ws::session::WsSession;
use crate::ws::types::{WsConnectionInfo, WsServerMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Запись об активном подключении в реестре
pub struct WsConnectionRecord {
    /// Сессия сокета с правами доступа и IP
    pub session: Arc<WsSession>,
    /// Канал отправки сообщений клиенту
    pub sender: mpsc::Sender<WsServerMessage>,
    /// Список активных тем подписки
    pub topics: Arc<RwLock<Vec<String>>>,
}

/// Реестр активных WebSocket соединений
#[derive(Clone, Default)]
pub struct WsConnectionRegistry {
    connections: Arc<RwLock<HashMap<u64, WsConnectionRecord>>>,
}

impl WsConnectionRegistry {
    /// Создать новый реестр
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Зарегистрировать новое активное соединение
    pub async fn register(
        &self,
        sub_id: u64,
        session: Arc<WsSession>,
        sender: mpsc::Sender<WsServerMessage>,
        initial_topics: Vec<String>,
    ) -> Arc<RwLock<Vec<String>>> {
        let topics = Arc::new(RwLock::new(initial_topics));
        let record = WsConnectionRecord {
            session,
            sender,
            topics: topics.clone(),
        };
        let mut guard = self.connections.write().await;
        guard.insert(sub_id, record);
        topics
    }

    /// Удалить соединение из реестра при отключении
    pub async fn unregister(&self, sub_id: u64) {
        let mut guard = self.connections.write().await;
        guard.remove(&sub_id);
    }

    /// Текущее количество активных WebSocket-подключений
    pub async fn count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Получить список информации обо всех активных соединениях
    pub async fn list(&self) -> Vec<WsConnectionInfo> {
        let guard = self.connections.read().await;
        let mut result = Vec::with_capacity(guard.len());

        for (sub_id, record) in guard.iter() {
            let claims = record.session.get_claims().await;
            let topics = record.topics.read().await.clone();
            let format = format!("{:?}", record.session.get_format().await);

            result.push(WsConnectionInfo {
                sub_id: *sub_id,
                user_id: claims.as_ref().map(|c| c.sub),
                username: claims.map(|c| c.username).unwrap_or_else(|| "anonymous".to_string()),
                client_ip: record.session.client_ip().to_string(),
                uptime_secs: record.session.uptime_secs(),
                format,
                topics,
            });
        }

        result
    }
}
