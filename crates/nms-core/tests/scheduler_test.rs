// Unit-тесты для модуля асинхронного планировщика scheduler.rs

use nms_core::SchedulerManager;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_scheduler_every_job() {
    let scheduler = SchedulerManager::new();
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let job_id = scheduler
        .every(
            0.1,
            Some("test_module".to_string()),
            "test_job",
            move || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .unwrap();

    sleep(Duration::from_millis(350)).await;
    assert!(counter.load(Ordering::SeqCst) >= 2);

    let cancelled = scheduler.cancel_job(&job_id).await;
    assert!(cancelled);
}

#[tokio::test]
async fn test_scheduler_cron_job() {
    let scheduler = SchedulerManager::new();
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    // Тестирование регистрации cron джоба с 5-элементным выражением или макросом @hourly
    let job_id = scheduler
        .cron(
            "0 * * * *",
            Some("test_module".to_string()),
            "cron_test",
            move || {
                let c = counter_clone.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await
        .unwrap();

    let jobs = scheduler.get_jobs(Some("test_module")).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_type, "cron");

    let cancelled = scheduler.cancel_job(&job_id).await;
    assert!(cancelled);
}

#[tokio::test]
async fn test_scheduler_cancel_module_jobs() {
    let scheduler = SchedulerManager::new();

    let _ = scheduler
        .every(0.1, Some("mod_a".to_string()), "job1", || async { Ok(()) })
        .await
        .unwrap();

    let _ = scheduler
        .every(0.1, Some("mod_a".to_string()), "job2", || async { Ok(()) })
        .await
        .unwrap();

    let _ = scheduler
        .every(0.1, Some("mod_b".to_string()), "job3", || async { Ok(()) })
        .await
        .unwrap();

    let cancelled_count = scheduler.cancel_module_jobs("mod_a").await;
    assert_eq!(cancelled_count, 2);

    let jobs = scheduler.get_jobs(None).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].module_id.as_deref(), Some("mod_b"));
}

#[tokio::test]
async fn test_scheduler_once_job() {
    let scheduler = SchedulerManager::new();
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let job_id = scheduler
        .once(0.05, Some("once_mod".to_string()), "once_job", move || {
            let c = counter_clone.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .await
        .unwrap();

    sleep(Duration::from_millis(150)).await;
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    let jobs = scheduler.get_jobs(Some("once_mod")).await;
    assert_eq!(jobs.len(), 1);
    assert!(!jobs[0].is_running);
    assert_eq!(jobs[0].runs_count, 1);

    let cancelled = scheduler.cancel_job(&job_id).await;
    assert!(cancelled);
}

#[tokio::test]
async fn test_scheduler_job_error_tracking() {
    let scheduler = SchedulerManager::new();

    let job_id = scheduler
        .every(0.05, Some("err_mod".to_string()), "err_job", || async {
            Err(anyhow::anyhow!("Simulated task failure"))
        })
        .await
        .unwrap();

    sleep(Duration::from_millis(180)).await;

    let jobs = scheduler.get_jobs(Some("err_mod")).await;
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].error_count >= 2);
    assert_eq!(
        jobs[0].last_error.as_deref(),
        Some("Simulated task failure")
    );

    scheduler.cancel_job(&job_id).await;
}

#[tokio::test]
async fn test_scheduler_stop() {
    let scheduler = SchedulerManager::new();

    let _ = scheduler
        .every(0.1, Some("stop_mod".to_string()), "job1", || async {
            Ok(())
        })
        .await
        .unwrap();

    assert!(scheduler.is_running().await);
    scheduler.stop().await;
    assert!(!scheduler.is_running().await);

    let jobs = scheduler.get_jobs(None).await;
    assert_eq!(jobs.len(), 0);
}

#[test]
fn test_get_next_cron_time_parsing() {
    use chrono::{TimeZone, Utc};
    use nms_core::get_next_cron_time;

    let base = Utc.with_ymd_and_hms(2026, 8, 9, 12, 0, 0).unwrap();

    let next_t = get_next_cron_time("* * * * *", Some(base)).unwrap();
    assert_eq!(next_t, Utc.with_ymd_and_hms(2026, 8, 9, 12, 1, 0).unwrap());

    let next_hourly = get_next_cron_time("@hourly", Some(base)).unwrap();
    assert_eq!(
        next_hourly,
        Utc.with_ymd_and_hms(2026, 8, 9, 13, 0, 0).unwrap()
    );

    let next_step = get_next_cron_time("0-10/2 * * * *", Some(base)).unwrap();
    assert_eq!(
        next_step,
        Utc.with_ymd_and_hms(2026, 8, 9, 12, 2, 0).unwrap()
    );

    assert!(get_next_cron_time("invalid cron", Some(base)).is_err());
    assert!(get_next_cron_time("99 * * * *", Some(base)).is_err());
    assert!(get_next_cron_time("0 0 30 2 *", Some(base)).is_err());
}

#[tokio::test]
async fn test_async_scheduler_alias_and_start() {
    use nms_core::AsyncScheduler;

    let scheduler = AsyncScheduler::new();
    scheduler.start().await;
    assert!(scheduler.is_running().await);
}
