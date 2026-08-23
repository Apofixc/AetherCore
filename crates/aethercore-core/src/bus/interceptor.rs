//! # Конвейер перехватчиков событий (Middleware / Interceptor Pipeline)
//!
//! Предоставляет точки расширения жизненного цикла событий перед отправкой подписчикам
//! (аудит, маскирование секретов, трассировка, валидация).

use aethercore_common::error::{AppError, Result};
use aethercore_common::models::events::EventMessage;
use async_trait::async_trait;
use std::sync::Arc;

/// Результат обработки события перехватчиком перед публикацией
#[derive(Debug, PartialEq, Eq)]
pub enum InterceptorAction {
    /// Продолжить стандартную обработку и доставку события
    Continue,
    /// Молча отбросить событие (не доставлять подписчикам и не писать в хранилище)
    DropSilently,
    /// Отклонить публикацию с возвратом ошибки вызывающему коду
    Reject(AppError),
}

/// Трейт перехватчика событий шины
#[async_trait]
pub trait EventInterceptor: Send + Sync {
    /// Вызывается перед маршрутизацией события (позволяет модифицировать или отклонить)
    async fn pre_publish(&self, event: &mut EventMessage) -> Result<InterceptorAction>;

    /// Вызывается после успешной отправки события подписчикам
    async fn post_publish(&self, _event: &EventMessage) {}
}

/// Конвейер зарегистрированных перехватчиков
#[derive(Default, Clone)]
pub struct InterceptorPipeline {
    interceptors: Vec<Arc<dyn EventInterceptor>>,
}

impl std::fmt::Debug for InterceptorPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InterceptorPipeline")
            .field("count", &self.interceptors.len())
            .finish()
    }
}

impl InterceptorPipeline {
    /// Создать пустой конвейер
    pub fn new() -> Self {
        Self {
            interceptors: Vec::new(),
        }
    }

    /// Зарегистрировать перехватчик в конвейере
    pub fn register(&mut self, interceptor: Arc<dyn EventInterceptor>) {
        self.interceptors.push(interceptor);
    }

    /// Выполнить фазу pre_publish по цепочке
    pub async fn execute_pre(&self, event: &mut EventMessage) -> Result<InterceptorAction> {
        for interceptor in &self.interceptors {
            let action = interceptor.pre_publish(event).await?;
            if action != InterceptorAction::Continue {
                return Ok(action);
            }
        }
        Ok(InterceptorAction::Continue)
    }

    /// Выполнить фазу post_publish по цепочке
    pub async fn execute_post(&self, event: &EventMessage) {
        for interceptor in &self.interceptors {
            interceptor.post_publish(event).await;
        }
    }
}

/// Встроенный перехватчик для маскирования конфиденциальных полей в JSON payload
pub struct MaskingInterceptor {
    masked_keys: Vec<&'static str>,
}

impl Default for MaskingInterceptor {
    fn default() -> Self {
        Self {
            masked_keys: vec!["password", "secret", "token", "api_key", "private_key"],
        }
    }
}

#[async_trait]
impl EventInterceptor for MaskingInterceptor {
    async fn pre_publish(&self, event: &mut EventMessage) -> Result<InterceptorAction> {
        if let serde_json::Value::Object(map) = &mut event.payload {
            for key in &self.masked_keys {
                if map.contains_key(*key) {
                    map.insert((*key).to_string(), serde_json::json!("***"));
                }
            }
        }
        Ok(InterceptorAction::Continue)
    }
}
