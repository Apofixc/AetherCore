// Асинхронный планировщик фоновых задач ядра NMS и плагинов
// Поддерживает задачи типов: every (периодические), cron (по расписанию) и once (однократные)

use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tracing::{error, info};

/// Метаданные и статус запланированной задачи
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    pub job_id: String,
    pub job_type: String, // "every", "cron", "once"
    pub name: String,
    pub module_id: Option<String>,
    pub seconds: Option<f64>,
    pub cron_expr: Option<String>,
    pub delay: Option<f64>,
    pub runs_count: u64,
    pub error_count: u64,
    pub last_run: Option<u64>,
    pub last_error: Option<String>,
    pub is_running: bool,
}

/// Алиас для обратной 1-в-1 совместимости с Python ScheduledJob
pub type ScheduledJob = JobInfo;

/// Информация об отдельной задаче в планировщике
struct TaskJob {
    info: JobInfo,
    handle: Option<JoinHandle<()>>,
}

/// Распарсить отдельное поле cron-выражения (*, */N, N, N-M, N-M/S, N,M) в множество допустимых значений
fn parse_cron_field(field_str: &str, min_val: u32, max_val: u32) -> Result<HashSet<u32>> {
    let mut result = HashSet::new();
    for sub_raw in field_str.split(',') {
        let sub = sub_raw.trim();
        if sub.is_empty() {
            anyhow::bail!("Empty item in cron field '{}'", field_str);
        }

        let mut step: u32 = 1;
        let main_part = if let Some((left, right)) = sub.split_once('/') {
            step = right
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid step in cron field '{}'", field_str))?;
            if step == 0 {
                anyhow::bail!("Step must be positive in cron field '{}'", field_str);
            }
            left
        } else {
            sub
        };

        if main_part == "*" {
            for v in (min_val..=max_val).step_by(step as usize) {
                result.insert(v);
            }
        } else if let Some((start_str, end_str)) = main_part.split_once('-') {
            let start = start_str
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid range in cron field '{}'", sub))?;
            let end = end_str
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid range in cron field '{}'", sub))?;
            if start < min_val || start > end || end > max_val {
                anyhow::bail!("Range '{}' out of bounds [{}-{}]", sub, min_val, max_val);
            }
            for v in (start..=end).step_by(step as usize) {
                result.insert(v);
            }
        } else {
            let val = main_part
                .parse::<u32>()
                .map_err(|_| anyhow!("Invalid value in cron field '{}'", sub))?;
            if val < min_val || val > max_val {
                anyhow::bail!(
                    "Value '{}' out of bounds [{}-{}] in cron field '{}'",
                    val,
                    min_val,
                    max_val,
                    field_str
                );
            }
            if step > 1 {
                for v in (val..=max_val).step_by(step as usize) {
                    result.insert(v);
                }
            } else {
                result.insert(val);
            }
        }
    }
    Ok(result)
}

/// Вычислить следующий datetime срабатывания 5-полевого cron-выражения (min hour dom month dow)
pub fn get_next_cron_time(
    cron_expr: &str,
    base_time: Option<DateTime<Utc>>,
) -> Result<DateTime<Utc>> {
    let expr_trimmed = cron_expr.trim().to_lowercase();
    let expanded = match expr_trimmed.as_str() {
        "@hourly" => "0 * * * *",
        "@daily" | "@midnight" => "0 0 * * *",
        "@weekly" => "0 0 * * 0",
        "@monthly" => "0 0 1 * *",
        _ => cron_expr.trim(),
    };

    let parts: Vec<&str> = expanded.split_whitespace().collect();
    if parts.len() != 5 {
        anyhow::bail!(
            "Invalid cron expression '{}'. Expected 5 fields.",
            cron_expr
        );
    }

    let min_set = parse_cron_field(parts[0], 0, 59)?;
    let hour_set = parse_cron_field(parts[1], 0, 23)?;
    let dom_set = parse_cron_field(parts[2], 1, 31)?;
    let month_set = parse_cron_field(parts[3], 1, 12)?;
    let mut dow_set = parse_cron_field(parts[4], 0, 7)?;

    let dom_restricted = parts[2] != "*";
    let dow_restricted = parts[4] != "*";

    if dow_set.contains(&7) {
        dow_set.insert(0);
    }
    if dow_set.contains(&0) {
        dow_set.insert(7);
    }

    let start = base_time.unwrap_or_else(Utc::now);
    let mut dt = start
        .with_second(0)
        .and_then(|t| t.with_nanosecond(0))
        .ok_or_else(|| anyhow!("Invalid base_time"))?
        + chrono::Duration::minutes(1);

    for _ in 0..525600 {
        if !month_set.contains(&dt.month()) {
            let next_year = if dt.month() == 12 {
                dt.year() + 1
            } else {
                dt.year()
            };
            let next_month = if dt.month() == 12 { 1 } else { dt.month() + 1 };
            dt = Utc
                .with_ymd_and_hms(next_year, next_month, 1, 0, 0, 0)
                .single()
                .ok_or_else(|| anyhow!("Invalid datetime step"))?;
            continue;
        }

        let cron_dow = (dt.weekday().num_days_from_monday() + 1) % 7;
        let iso_dow = dt.weekday().number_from_monday();

        let dom_match = dom_set.contains(&dt.day());
        let dow_match = dow_set.contains(&cron_dow) || dow_set.contains(&iso_dow);
        let day_match = if dom_restricted && dow_restricted {
            dom_match || dow_match
        } else {
            dom_match && dow_match
        };

        if !day_match {
            let next_day = dt + chrono::Duration::days(1);
            dt = Utc
                .with_ymd_and_hms(next_day.year(), next_day.month(), next_day.day(), 0, 0, 0)
                .single()
                .ok_or_else(|| anyhow!("Invalid datetime step"))?;
            continue;
        }

        if !hour_set.contains(&dt.hour()) {
            let next_hour = dt + chrono::Duration::hours(1);
            dt = Utc
                .with_ymd_and_hms(
                    next_hour.year(),
                    next_hour.month(),
                    next_hour.day(),
                    next_hour.hour(),
                    0,
                    0,
                )
                .single()
                .ok_or_else(|| anyhow!("Invalid datetime step"))?;
            continue;
        }

        let current_min = dt.minute();
        let matching_min = min_set.iter().copied().filter(|&m| m >= current_min).min();
        if let Some(m) = matching_min {
            return dt
                .with_minute(m)
                .ok_or_else(|| anyhow!("Invalid minute replacement"));
        }

        let next_hour = dt + chrono::Duration::hours(1);
        dt = Utc
            .with_ymd_and_hms(
                next_hour.year(),
                next_hour.month(),
                next_hour.day(),
                next_hour.hour(),
                0,
                0,
            )
            .single()
            .ok_or_else(|| anyhow!("Invalid datetime step"))?;
    }

    anyhow::bail!(
        "Cron expression '{}' never matches within one year.",
        cron_expr
    );
}

/// Преобразование стандартных макросов cron (@hourly, @daily и т.д.)
fn expand_cron_macro(expr: &str) -> String {
    match expr.trim().to_lowercase().as_str() {
        "@hourly" => "0 0 * * * *".to_string(),
        "@daily" | "@midnight" => "0 0 0 * * *".to_string(),
        "@weekly" => "0 0 0 * * 0".to_string(),
        "@monthly" => "0 0 0 1 * *".to_string(),
        other => {
            let parts: Vec<&str> = other.split_whitespace().collect();
            if parts.len() == 5 {
                format!("0 {}", other)
            } else {
                other.to_string()
            }
        }
    }
}

/// Асинхронный планировщик задач NMS на базе Tokio runtime
#[derive(Clone, Default)]
pub struct SchedulerManager {
    jobs: Arc<RwLock<HashMap<String, TaskJob>>>,
    is_running: Arc<RwLock<bool>>,
}

/// Алиас типа для 1-в-1 соответствия с Python AsyncScheduler
pub type AsyncScheduler = SchedulerManager;

impl SchedulerManager {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(true)),
        }
    }

    /// Запустить планировщик и активировать выполнение задач
    pub async fn start(&self) {
        let mut running = self.is_running.write().await;
        *running = true;
        info!("SchedulerManager started.");
    }

    /// Регистрация периодической задачи (every) каждые N секунд
    pub async fn every<F, Fut>(
        &self,
        seconds: f64,
        module_id: Option<String>,
        name: impl Into<String>,
        job_fn: F,
    ) -> Result<String>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        if seconds <= 0.0 {
            anyhow::bail!("Interval seconds must be greater than 0");
        }

        let job_id = format!("job_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let job_name = name.into();

        let info = JobInfo {
            job_id: job_id.clone(),
            job_type: "every".to_string(),
            name: job_name.clone(),
            module_id: module_id.clone(),
            seconds: Some(seconds),
            cron_expr: None,
            delay: None,
            runs_count: 0,
            error_count: 0,
            last_run: None,
            last_error: None,
            is_running: true,
        };

        let jobs_arc = self.jobs.clone();
        let running_arc = self.is_running.clone();
        let id_clone = job_id.clone();

        let handle = tokio::spawn(async move {
            let interval_dur = Duration::from_secs_f64(seconds);
            loop {
                sleep(interval_dur).await;

                if !*running_arc.read().await {
                    break;
                }

                let now_ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                match job_fn().await {
                    Ok(_) => {
                        let mut map = jobs_arc.write().await;
                        if let Some(j) = map.get_mut(&id_clone) {
                            j.info.runs_count += 1;
                            j.info.last_run = Some(now_ts);
                        }
                    }
                    Err(err) => {
                        let err_str = err.to_string();
                        error!(
                            "Error executing job '{}' (id={}): {}",
                            job_name, id_clone, err_str
                        );
                        let mut map = jobs_arc.write().await;
                        if let Some(j) = map.get_mut(&id_clone) {
                            j.info.error_count += 1;
                            j.info.last_run = Some(now_ts);
                            j.info.last_error = Some(err_str);
                        }
                    }
                }
            }
        });

        let mut map = self.jobs.write().await;
        map.insert(
            job_id.clone(),
            TaskJob {
                info,
                handle: Some(handle),
            },
        );

        info!(
            "Scheduled task '{}' (id={}) every {}s",
            name_ref(&map, &job_id),
            job_id,
            seconds
        );
        Ok(job_id)
    }

    /// Регистрация задачи по расписанию cron (cron)
    pub async fn cron<F, Fut>(
        &self,
        cron_expr: &str,
        module_id: Option<String>,
        name: impl Into<String>,
        job_fn: F,
    ) -> Result<String>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        // Валидация cron выражения на этапе регистрации
        get_next_cron_time(cron_expr, None)?;

        let expanded_expr = expand_cron_macro(cron_expr);
        let job_id = format!("job_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let job_name = name.into();

        let jobs_arc = self.jobs.clone();
        let id_clone = job_id.clone();
        let job_name_clone = job_name.clone();

        let job_fn_arc = Arc::new(job_fn);

        let schedule: tokio_cron_scheduler::Job =
            tokio_cron_scheduler::Job::new_async(expanded_expr.as_str(), move |_, _| {
                let job_fn = job_fn_arc.clone();
                let jobs_arc = jobs_arc.clone();
                let id_clone = id_clone.clone();
                let job_name = job_name_clone.clone();
                Box::pin(async move {
                    let now_ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    match job_fn().await {
                        Ok(_) => {
                            let mut map = jobs_arc.write().await;
                            if let Some(j) = map.get_mut(&id_clone) {
                                j.info.runs_count += 1;
                                j.info.last_run = Some(now_ts);
                            }
                        }
                        Err(err) => {
                            let err_str = err.to_string();
                            error!(
                                "Error executing cron job '{}' (id={}): {}",
                                job_name, id_clone, err_str
                            );
                            let mut map = jobs_arc.write().await;
                            if let Some(j) = map.get_mut(&id_clone) {
                                j.info.error_count += 1;
                                j.info.last_run = Some(now_ts);
                                j.info.last_error = Some(err_str);
                            }
                        }
                    }
                })
            })
            .map_err(|e| anyhow!("Invalid cron expression '{}': {}", cron_expr, e))?;

        let info = JobInfo {
            job_id: job_id.clone(),
            job_type: "cron".to_string(),
            name: job_name,
            module_id,
            seconds: None,
            cron_expr: Some(cron_expr.to_string()),
            delay: None,
            runs_count: 0,
            error_count: 0,
            last_run: None,
            last_error: None,
            is_running: true,
        };

        let sched = tokio_cron_scheduler::JobScheduler::new().await?;
        sched.add(schedule).await?;
        sched.start().await?;

        let handle = tokio::spawn(async move {
            let _sched = sched;
            loop {
                sleep(Duration::from_secs(3600)).await;
            }
        });

        let mut map = self.jobs.write().await;
        map.insert(
            job_id.clone(),
            TaskJob {
                info,
                handle: Some(handle),
            },
        );

        Ok(job_id)
    }

    /// Регистрация однократной задачи (once) с задержкой в секунд
    pub async fn once<F, Fut>(
        &self,
        delay_seconds: f64,
        module_id: Option<String>,
        name: impl Into<String>,
        job_fn: F,
    ) -> Result<String>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let job_id = format!("job_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let job_name = name.into();

        let info = JobInfo {
            job_id: job_id.clone(),
            job_type: "once".to_string(),
            name: job_name.clone(),
            module_id,
            seconds: None,
            cron_expr: None,
            delay: Some(delay_seconds),
            runs_count: 0,
            error_count: 0,
            last_run: None,
            last_error: None,
            is_running: true,
        };

        let jobs_arc = self.jobs.clone();
        let id_clone = job_id.clone();

        let handle = tokio::spawn(async move {
            if delay_seconds > 0.0 {
                sleep(Duration::from_secs_f64(delay_seconds)).await;
            }

            let now_ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            match job_fn().await {
                Ok(_) => {
                    let mut map = jobs_arc.write().await;
                    if let Some(j) = map.get_mut(&id_clone) {
                        j.info.runs_count += 1;
                        j.info.last_run = Some(now_ts);
                        j.info.is_running = false;
                    }
                }
                Err(err) => {
                    let err_str = err.to_string();
                    error!("Error executing once job '{}': {}", job_name, err_str);
                    let mut map = jobs_arc.write().await;
                    if let Some(j) = map.get_mut(&id_clone) {
                        j.info.error_count += 1;
                        j.info.last_run = Some(now_ts);
                        j.info.last_error = Some(err_str);
                        j.info.is_running = false;
                    }
                }
            }
        });

        let mut map = self.jobs.write().await;
        map.insert(
            job_id.clone(),
            TaskJob {
                info,
                handle: Some(handle),
            },
        );

        Ok(job_id)
    }

    /// Отмена конкретной задачи по job_id
    pub async fn cancel_job(&self, job_id: &str) -> bool {
        let mut map = self.jobs.write().await;
        if let Some(mut task_job) = map.remove(job_id) {
            if let Some(handle) = task_job.handle.take() {
                handle.abort();
            }
            info!("Cancelled scheduled job {}", job_id);
            return true;
        }
        false
    }

    /// Массовая отмена всех задач конкретного модуля (module_id)
    pub async fn cancel_module_jobs(&self, module_id: &str) -> usize {
        let mut map = self.jobs.write().await;
        let job_ids_to_cancel: Vec<String> = map
            .iter()
            .filter(|(_, j)| j.info.module_id.as_deref() == Some(module_id))
            .map(|(id, _)| id.clone())
            .collect();

        let count = job_ids_to_cancel.len();
        for id in job_ids_to_cancel {
            if let Some(mut task_job) = map.remove(&id) {
                if let Some(handle) = task_job.handle.take() {
                    handle.abort();
                }
            }
        }

        info!(
            "Cancelled {} scheduled jobs for module '{}'",
            count, module_id
        );
        count
    }

    /// Получение списка всех задач планировщика с фильтром по module_id
    pub async fn get_jobs(&self, filter_module_id: Option<&str>) -> Vec<JobInfo> {
        let map = self.jobs.read().await;
        map.values()
            .filter(|j| match filter_module_id {
                Some(mod_id) => j.info.module_id.as_deref() == Some(mod_id),
                None => true,
            })
            .map(|j| j.info.clone())
            .collect()
    }

    /// Проверка, запущен ли планировщик
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Остановка работы планировщика и отмена всех активных задач
    pub async fn stop(&self) {
        let mut running = self.is_running.write().await;
        *running = false;
        let mut map = self.jobs.write().await;
        for (_, mut task_job) in map.drain() {
            if let Some(handle) = task_job.handle.take() {
                handle.abort();
            }
        }
        info!("SchedulerManager stopped and all tasks cancelled.");
    }
}

fn name_ref<'a>(map: &'a HashMap<String, TaskJob>, id: &str) -> &'a str {
    map.get(id).map(|j| j.info.name.as_str()).unwrap_or("job")
}
