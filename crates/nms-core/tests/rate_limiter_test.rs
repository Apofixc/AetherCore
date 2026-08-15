use nms_core::RateLimiter;
use std::thread::sleep;
use std::time::Duration;

#[test]
fn test_rate_limiter_sliding_window() {
    let limiter = RateLimiter::new();
    let key = "127.0.0.1:login";

    // Допускается ровно 3 запроса в течение 1 секунды
    assert!(!limiter.is_rate_limited(key, 3, 1));
    assert!(!limiter.is_rate_limited(key, 3, 1));
    assert!(!limiter.is_rate_limited(key, 3, 1));

    // 4-й запрос превышает лимит
    assert!(limiter.is_rate_limited(key, 3, 1));

    // Пауза для истечения временного окна в 1 секунду
    sleep(Duration::from_millis(1100));

    // Теперь запрос снова должен проходить
    assert!(!limiter.is_rate_limited(key, 3, 1));
}

#[test]
fn test_rate_limiter_clear() {
    let limiter = RateLimiter::new();
    let key = "user:admin";

    assert!(!limiter.is_rate_limited(key, 1, 60));
    assert!(limiter.is_rate_limited(key, 1, 60));

    limiter.clear();

    assert!(!limiter.is_rate_limited(key, 1, 60));
}
