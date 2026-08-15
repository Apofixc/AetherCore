// Внутрипамятный скользящий ограничик частоты запросов (In-memory Sliding Window Rate Limiter)
// Используется для защиты REST/WS эндпоинтов от перебора паролей и DoS-атак по IP и логину

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Структура скользящего ограничения частоты запросов (Rate Limiter)
#[derive(Clone, Debug, Default)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    /// Создать новый экземпляр RateLimiter
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Проверить превышение лимита запросов для указанного ключа (например IP + username)
    ///
    /// # Параметры:
    /// - `key`: уникальный идентификатор клиента (например "192.168.1.1:admin:login")
    /// - `max_requests`: максимальное допустимое количество запросов за временное окно
    /// - `window_seconds`: длительность временного окна в секундах
    ///
    /// # Возвращает:
    /// - `true` если лимит превышен (запрос следует заблокировать)
    /// - `false` если запрос разрешен
    pub fn is_rate_limited(&self, key: &str, max_requests: usize, window_seconds: u64) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(window_seconds);

        let mut map = match self.requests.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let timestamps = map.entry(key.to_string()).or_default();

        // Очистка устаревших временных отметок, выходящих за временное окно
        timestamps.retain(|&t| now.duration_since(t) < window);

        if timestamps.len() >= max_requests {
            return true;
        }

        timestamps.push(now);
        false
    }

    /// Сбросить/очистить все сохраненные метки запросов
    pub fn clear(&self) {
        let mut map = match self.requests.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        map.clear();
    }
}
