// Сервис асинхронной рассылки алертов и уведомления пользователей NMS
// Обеспечивает персистентное хранение уведомлений в SQLite, адресную доставку,
// учет предпочтений пользователей (тихие часы, правила модулей, глушение),
// дедупликацию/группировку, квитирование, эскалацию аварий и интеграцию с шиной событий EventBus.

use crate::bus::{EventBus, SystemEvent};
use crate::exceptions::NmsError;
use crate::i18n::I18nEngine;
use anyhow::{anyhow, Result};
use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Row, Sqlite};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

pub const ALLOWED_SEVERITIES: [&str; 4] = ["info", "success", "warning", "error"];
pub const ALLOWED_CATEGORIES: [&str; 4] = ["system", "security", "module", "user"];
pub const NOTIFICATION_RETENTION_DAYS: i64 = 30;
pub const MAX_TITLE_LEN: usize = 255;
pub const MAX_BODY_LEN: usize = 4000;

/// Уровень критичности уведомления
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationSeverity {
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    pub fn level(&self) -> u8 {
        match self {
            Self::Info | Self::Success => 1,
            Self::Warning => 2,
            Self::Error => 3,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "success" => Self::Success,
            "warning" => Self::Warning,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }
}

impl Default for NotificationSeverity {
    fn default() -> Self {
        Self::Info
    }
}

/// Модель сообщения уведомления
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    pub id: i64,
    pub module_id: String,
    pub user_id: String,
    pub title: String,
    pub body: String,
    pub severity: NotificationSeverity,
    pub category: String,
    pub entity_id: Option<String>,
    pub target_url: Option<String>,
    pub group_count: i64,
    pub actions: Option<serde_json::Value>,
    pub acknowledged_at: Option<f64>,
    pub acknowledged_by: Option<String>,
    pub escalated_at: Option<f64>,
    pub title_template: Option<String>,
    pub created_at: f64,
    pub read_at: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_eligible: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_eligible: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_signal: Option<String>,
}

/// Настройки предпочтений уведомлений пользователя
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserNotificationPreferences {
    pub user_id: String,
    pub push_enabled: bool,
    pub sound_enabled: bool,
    pub subscribed_modules: Option<Vec<String>>,
    pub module_rules: serde_json::Value,
    pub sound_signals: serde_json::Value,
    pub muted_until: Option<f64>,
    pub quiet_hours: serde_json::Value,
}

impl Default for UserNotificationPreferences {
    fn default() -> Self {
        Self {
            user_id: String::new(),
            push_enabled: true,
            sound_enabled: true,
            subscribed_modules: None,
            module_rules: serde_json::json!({}),
            sound_signals: serde_json::json!({}),
            muted_until: None,
            quiet_hours: serde_json::json!({}),
        }
    }
}

/// Структура параметров обновления настроек уведомлений
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SetPreferencesInput {
    pub push_enabled: Option<bool>,
    pub sound_enabled: Option<bool>,
    pub subscribed_modules: Option<Vec<String>>,
    pub module_rules: Option<serde_json::Value>,
    pub sound_signals: Option<serde_json::Value>,
    pub muted_until: Option<Option<f64>>,
    pub quiet_hours: Option<serde_json::Value>,
}

/// Параметры создания уведомления
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyParams {
    pub user_id: String,
    pub title: String,
    pub body: String,
    pub severity: NotificationSeverity,
    pub category: String,
    pub entity_id: Option<String>,
    pub module_id: String,
    pub allow_push: bool,
    pub target_url: Option<String>,
    pub actions: Option<serde_json::Value>,
    pub title_template: Option<String>,
}

impl Default for NotifyParams {
    fn default() -> Self {
        Self {
            user_id: String::new(),
            title: String::new(),
            body: String::new(),
            severity: NotificationSeverity::Info,
            category: "system".to_string(),
            entity_id: None,
            module_id: "core".to_string(),
            allow_push: true,
            target_url: None,
            actions: None,
            title_template: None,
        }
    }
}

/// Фильтр для получения списка уведомлений
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NotificationFilter {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub unread_only: Option<bool>,
    pub severity: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
}

/// Результат получения списка уведомлений с пагинацией
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationListResult {
    pub items: Vec<NotificationMessage>,
    pub total: i64,
    pub filtered_total: i64,
    pub unread_count: i64,
    pub limit: i64,
    pub offset: i64,
}

/// Проверить, действуют ли сейчас тихие часы пользователя
pub fn is_quiet_hours(quiet_hours: &serde_json::Value, now_ts: f64) -> bool {
    if !quiet_hours.is_object()
        || !quiet_hours
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        return false;
    }

    let start_str = match quiet_hours.get("start").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };
    let end_str = match quiet_hours.get("end").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return false,
    };

    let now_dt = Local
        .timestamp_opt(now_ts as i64, 0)
        .single()
        .unwrap_or_else(Local::now);
    let wday = now_dt.weekday().num_days_from_monday() as i64; // 0=Mon, 4=Fri, 5=Sat, 6=Sun

    if let Some(days_val) = quiet_hours.get("days") {
        if let Some(days_str) = days_val.as_str() {
            if days_str == "weekdays" && wday >= 5 {
                return false;
            } else if days_str == "weekends" && wday < 5 {
                return false;
            }
        } else if let Some(days_arr) = days_val.as_array() {
            if !days_arr.is_empty() {
                let allowed: Vec<i64> = days_arr.iter().filter_map(|v| v.as_i64()).collect();
                if !allowed.contains(&wday) {
                    return false;
                }
            }
        }
    }

    let parse_hm = |s: &str| -> Option<(u32, u32)> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let h = parts[0].trim().parse::<u32>().ok()?;
            let m = parts[1].trim().parse::<u32>().ok()?;
            Some((h, m))
        } else {
            None
        }
    };

    let (sh, sm) = match parse_hm(start_str) {
        Some(res) => res,
        None => return false,
    };
    let (eh, em) = match parse_hm(end_str) {
        Some(res) => res,
        None => return false,
    };

    let now_minutes = now_dt.hour() * 60 + now_dt.minute();
    let start_min = sh * 60 + sm;
    let end_min = eh * 60 + em;

    if start_min < end_min {
        now_minutes >= start_min && now_minutes < end_min
    } else {
        now_minutes >= start_min || now_minutes < end_min
    }
}

/// Информация о модуле для управления подписками на уведомления
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationModuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Получить список всех поддерживаемых категорий уведомлений
pub fn get_notification_categories() -> Vec<String> {
    let mut categories: Vec<String> = ALLOWED_CATEGORIES.iter().map(|s| s.to_string()).collect();
    categories.sort();
    categories
}

/// Получить список всех зарегистрированных модулей системы для управления подписками
pub fn get_notification_modules() -> Vec<NotificationModuleInfo> {
    vec![NotificationModuleInfo {
        id: "core".to_string(),
        name: "Ядро системы (Core)".to_string(),
        description: "Системные уведомления и важные оповещения ядра".to_string(),
    }]
}

/// Получить текущее время в секундах (UNIX timestamp f64)
fn current_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Менеджер уведомлений ядра NMS
#[derive(Clone)]
pub struct NotificationEngine {
    event_bus: EventBus,
    db_pool: Option<Pool<Sqlite>>,
    unread_cache: Arc<RwLock<HashMap<String, i64>>>,
    i18n: Option<Arc<I18nEngine>>,
}

impl NotificationEngine {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            event_bus,
            db_pool: None,
            unread_cache: Arc::new(RwLock::new(HashMap::new())),
            i18n: None,
        }
    }

    pub fn with_db_pool(mut self, db_pool: Pool<Sqlite>) -> Self {
        self.db_pool = Some(db_pool);
        self
    }

    pub fn with_i18n(mut self, i18n: Arc<I18nEngine>) -> Self {
        self.i18n = Some(i18n);
        self
    }

    pub fn new_with_db(event_bus: EventBus, db_pool: Pool<Sqlite>) -> Self {
        Self {
            event_bus,
            db_pool: Some(db_pool),
            unread_cache: Arc::new(RwLock::new(HashMap::new())),
            i18n: None,
        }
    }

    /// Инвалидировать/очистить кэш непрочитанных уведомлений
    pub fn invalidate_unread_cache(&self, user_id: Option<&str>) {
        if let Ok(mut guard) = self.unread_cache.write() {
            if let Some(uid) = user_id {
                guard.remove(uid.trim());
            } else {
                guard.clear();
            }
        }
    }

    /// Получить настройки уведомлений пользователя
    pub async fn get_notification_preferences(
        &self,
        user_id: &str,
    ) -> Result<UserNotificationPreferences> {
        let user_str = user_id.trim();
        let pool = match &self.db_pool {
            Some(p) => p,
            None => {
                return Ok(UserNotificationPreferences {
                    user_id: user_str.to_string(),
                    ..Default::default()
                })
            }
        };

        let row = sqlx::query(
            "SELECT push_enabled, sound_enabled, subscribed_modules, module_rules, sound_signals, muted_until, quiet_hours FROM notification_preferences WHERE user_id = ?"
        )
        .bind(user_str)
        .fetch_optional(pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => {
                return Ok(UserNotificationPreferences {
                    user_id: user_str.to_string(),
                    ..Default::default()
                })
            }
        };

        let push_enabled: bool = row.try_get("push_enabled").unwrap_or(true);
        let sound_enabled: bool = row.try_get("sound_enabled").unwrap_or(true);

        let subscribed_modules: Option<Vec<String>> = row
            .try_get::<Option<String>, _>("subscribed_modules")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok());

        let module_rules: serde_json::Value = row
            .try_get::<Option<String>, _>("module_rules")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let sound_signals: serde_json::Value = row
            .try_get::<Option<String>, _>("sound_signals")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let raw_muted_until: Option<f64> = row.try_get("muted_until").ok().flatten();
        let now_ts = current_timestamp();
        let muted_until = match raw_muted_until {
            Some(val) if val == -1.0 || val > now_ts => Some(val),
            _ => None,
        };

        let quiet_hours: serde_json::Value = row
            .try_get::<Option<String>, _>("quiet_hours")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        Ok(UserNotificationPreferences {
            user_id: user_str.to_string(),
            push_enabled,
            sound_enabled,
            subscribed_modules,
            module_rules,
            sound_signals,
            muted_until,
            quiet_hours,
        })
    }

    /// Обновить настройки уведомлений пользователя
    pub async fn set_notification_preferences(
        &self,
        user_id: &str,
        input: SetPreferencesInput,
    ) -> Result<UserNotificationPreferences> {
        let user_str = user_id.trim();
        let current = self.get_notification_preferences(user_str).await?;

        let new_push = input.push_enabled.unwrap_or(current.push_enabled);
        let new_sound = input.sound_enabled.unwrap_or(current.sound_enabled);
        let new_subscribed = input.subscribed_modules.or(current.subscribed_modules);
        let new_rules = input.module_rules.unwrap_or(current.module_rules);
        let new_signals = input.sound_signals.unwrap_or(current.sound_signals);
        let new_quiet = input.quiet_hours.unwrap_or(current.quiet_hours);

        let new_muted_until = match input.muted_until {
            Some(Some(val)) if val < 0.0 => Some(-1.0),
            Some(Some(val)) if val == 0.0 => None,
            Some(Some(val)) => Some(val),
            Some(None) => None,
            None => current.muted_until,
        };

        if let Some(pool) = &self.db_pool {
            let subscribed_json = new_subscribed
                .as_ref()
                .map(|s| serde_json::to_string(s).unwrap_or_default());
            let rules_json = serde_json::to_string(&new_rules).unwrap_or_default();
            let signals_json = serde_json::to_string(&new_signals).unwrap_or_default();
            let quiet_json = serde_json::to_string(&new_quiet).unwrap_or_default();

            sqlx::query(
                r#"
                INSERT INTO notification_preferences (user_id, push_enabled, sound_enabled, subscribed_modules, module_rules, sound_signals, muted_until, quiet_hours)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(user_id) DO UPDATE SET
                    push_enabled = excluded.push_enabled,
                    sound_enabled = excluded.sound_enabled,
                    subscribed_modules = excluded.subscribed_modules,
                    module_rules = excluded.module_rules,
                    sound_signals = excluded.sound_signals,
                    muted_until = excluded.muted_until,
                    quiet_hours = excluded.quiet_hours
                "#
            )
            .bind(user_str)
            .bind(new_push)
            .bind(new_sound)
            .bind(subscribed_json)
            .bind(rules_json)
            .bind(signals_json)
            .bind(new_muted_until)
            .bind(quiet_json)
            .execute(pool)
            .await?;
        }

        Ok(UserNotificationPreferences {
            user_id: user_str.to_string(),
            push_enabled: new_push,
            sound_enabled: new_sound,
            subscribed_modules: new_subscribed,
            module_rules: new_rules,
            sound_signals: new_signals,
            muted_until: new_muted_until,
            quiet_hours: new_quiet,
        })
    }

    /// Подсчитать количество непрочитанных уведомлений пользователя
    pub async fn count_unread_notifications(&self, user_id: &str) -> Result<i64> {
        let user_str = user_id.trim();

        if let Ok(guard) = self.unread_cache.read() {
            if let Some(&cnt) = guard.get(user_str) {
                return Ok(cnt);
            }
        }

        let cnt = match &self.db_pool {
            Some(pool) => {
                let row = sqlx::query(
                    "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL",
                )
                .bind(user_str)
                .fetch_one(pool)
                .await?;
                row.get::<i64, _>(0)
            }
            None => 0,
        };

        if let Ok(mut guard) = self.unread_cache.write() {
            guard.insert(user_str.to_string(), cnt);
        }

        Ok(cnt)
    }

    /// Отправить уведомление (согласовано со старым интерфейсом)
    pub async fn send_notification(
        &self,
        user_id: Option<&str>,
        title: &str,
        body: &str,
        severity: NotificationSeverity,
        category: &str,
        module_id: Option<&str>,
    ) -> Result<NotificationMessage> {
        let params = NotifyParams {
            user_id: user_id.unwrap_or("").to_string(),
            title: title.to_string(),
            body: body.to_string(),
            severity,
            category: category.to_string(),
            module_id: module_id.unwrap_or("core").to_string(),
            ..Default::default()
        };

        let notif = self.notify(params).await?;
        notif.ok_or_else(|| anyhow!("Notification omitted by user preferences"))
    }

    /// Полный метод создания и обработки уведомлений
    pub async fn notify(&self, params: NotifyParams) -> Result<Option<NotificationMessage>> {
        let user_str = params.user_id.trim();
        if user_str.is_empty() {
            let msg = self
                .i18n
                .as_ref()
                .map(|engine| engine.tr("ru", "notify_missing_user_id", None, None))
                .unwrap_or_else(|| "user_id is required for notify()".to_string());
            return Err(NmsError::Validation {
                message: msg,
                details: serde_json::json!({ "code": "NOTIFY_MISSING_USER_ID" }),
            }
            .into());
        }

        let mut title_str = params.title.trim().to_string();
        if title_str.is_empty() {
            let msg = self
                .i18n
                .as_ref()
                .map(|engine| engine.tr("ru", "notify_missing_title", None, None))
                .unwrap_or_else(|| "title is required for notify()".to_string());
            return Err(NmsError::Validation {
                message: msg,
                details: serde_json::json!({ "code": "NOTIFY_MISSING_TITLE" }),
            }
            .into());
        }

        if title_str.chars().count() > MAX_TITLE_LEN {
            title_str = title_str
                .chars()
                .take(MAX_TITLE_LEN - 3)
                .collect::<String>()
                + "...";
        }

        let mut body_str = params.body.clone();
        if body_str.chars().count() > MAX_BODY_LEN {
            body_str = body_str.chars().take(MAX_BODY_LEN - 3).collect::<String>() + "...";
        }

        let sev_str = params.severity.as_str();
        let cat_str = if ALLOWED_CATEGORIES.contains(&params.category.to_lowercase().as_str()) {
            params.category.to_lowercase()
        } else {
            "system".to_string()
        };

        let mod_id = if params.module_id.trim().is_empty() {
            "core".to_string()
        } else {
            params.module_id.trim().to_string()
        };

        let now_ts = current_timestamp();

        // 1. Проверка предпочтений пользователя, если база данных доступна
        let prefs = self.get_notification_preferences(user_str).await?;

        // Проверка глобального заглушения muted_until
        if let Some(m_until) = prefs.muted_until {
            if m_until == -1.0 || now_ts < m_until {
                info!(
                    "Notification omitted for user {}: temporarily muted",
                    user_str
                );
                return Ok(None);
            }
        }

        // Проверка правил модуля (module_rules)
        if let Some(mod_rule) = prefs.module_rules.get(&mod_id) {
            if mod_rule.get("enabled").and_then(|v| v.as_bool()) == Some(false)
                || mod_rule.get("disabled").and_then(|v| v.as_bool()) == Some(true)
            {
                info!(
                    "Notification omitted for user {}: module '{}' is disabled in rules",
                    user_str, mod_id
                );
                return Ok(None);
            }

            if let Some(m_until) = mod_rule.get("muted_until").and_then(|v| v.as_f64()) {
                if m_until == -1.0 || now_ts < m_until {
                    info!(
                        "Notification omitted for user {}: module '{}' muted until {}",
                        user_str, mod_id, m_until
                    );
                    return Ok(None);
                }
            }

            if let Some(min_sev_str) = mod_rule.get("min_severity").and_then(|v| v.as_str()) {
                let min_sev = NotificationSeverity::parse(min_sev_str);
                if params.severity.level() < min_sev.level() {
                    info!("Notification omitted for user {}: severity below threshold for module '{}'", user_str, mod_id);
                    return Ok(None);
                }
            }
        }

        // Проверка подписок на модули (subscribed_modules)
        if let Some(ref sub_modules) = prefs.subscribed_modules {
            if mod_id != "core" && !sub_modules.contains(&mod_id) {
                let explicitly_enabled = prefs
                    .module_rules
                    .get(&mod_id)
                    .and_then(|v| v.get("enabled"))
                    .and_then(|v| v.as_bool())
                    == Some(true);
                if !explicitly_enabled {
                    info!(
                        "Notification omitted for user {}: module '{}' not in subscribed_modules",
                        user_str, mod_id
                    );
                    return Ok(None);
                }
            }
        }

        let actions_json_str = params
            .actions
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());

        let effective_template = params.title_template.clone().or_else(|| {
            if title_str.contains("{count}") {
                Some(title_str.clone())
            } else {
                None
            }
        });

        let initial_title = if let Some(ref tpl) = effective_template {
            tpl.replace("{count}", "1")
        } else {
            title_str.clone()
        };

        let notification_id: i64;
        let group_count: i64;
        let final_title: String;

        if let Some(pool) = &self.db_pool {
            let cutoff = now_ts - 60.0;

            // Проверка дедупликации/группировки за последние 60 секунд
            let dup_row = sqlx::query(
                r#"
                SELECT id, group_count FROM notifications
                WHERE user_id = ? AND module_id = ? AND category = ? AND severity = ? AND (title = ? OR title_template = ?) AND read_at IS NULL AND created_at >= ?
                ORDER BY id DESC LIMIT 1
                "#
            )
            .bind(user_str)
            .bind(&mod_id)
            .bind(&cat_str)
            .bind(sev_str)
            .bind(&initial_title)
            .bind(effective_template.as_deref().unwrap_or(&initial_title))
            .bind(cutoff)
            .fetch_optional(pool)
            .await?;

            if let Some(dup) = dup_row {
                notification_id = dup.get("id");
                let old_cnt: i64 = dup.try_get("group_count").unwrap_or(1);
                group_count = old_cnt + 1;

                if let Some(ref tpl) = effective_template {
                    final_title = tpl.replace("{count}", &group_count.to_string());
                } else {
                    final_title = title_str.clone();
                }

                sqlx::query(
                    r#"
                    UPDATE notifications
                    SET group_count = ?, created_at = ?, title = ?, body = CASE WHEN ? != '' THEN ? ELSE body END, actions = COALESCE(?, actions)
                    WHERE id = ?
                    "#
                )
                .bind(group_count)
                .bind(now_ts)
                .bind(&final_title)
                .bind(&body_str)
                .bind(&body_str)
                .bind(&actions_json_str)
                .bind(notification_id)
                .execute(pool)
                .await?;
            } else {
                final_title = initial_title;
                let res = sqlx::query(
                    r#"
                    INSERT INTO notifications (module_id, user_id, title, body, severity, category, entity_id, target_url, group_count, actions, title_template, created_at, read_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, NULL)
                    "#
                )
                .bind(&mod_id)
                .bind(user_str)
                .bind(&final_title)
                .bind(&body_str)
                .bind(sev_str)
                .bind(&cat_str)
                .bind(&params.entity_id)
                .bind(&params.target_url)
                .bind(&actions_json_str)
                .bind(&effective_template)
                .bind(now_ts)
                .execute(pool)
                .await?;

                notification_id = res.last_insert_rowid();
                group_count = 1;
            }

            self.invalidate_unread_cache(Some(user_str));
        } else {
            notification_id = (now_ts * 1000.0) as i64;
            group_count = 1;
            final_title = initial_title;
        }

        let is_qh = is_quiet_hours(&prefs.quiet_hours, now_ts);
        let is_error = params.severity == NotificationSeverity::Error;

        let push_elig = params.allow_push && prefs.push_enabled && (!is_qh || is_error);
        let sound_elig = prefs.sound_enabled && (!is_qh || is_error);

        let mod_rule_sound = prefs
            .module_rules
            .get(&mod_id)
            .and_then(|v| v.get("sound_signal"))
            .and_then(|v| v.as_str());

        let sev_sound = prefs.sound_signals.get(sev_str).and_then(|v| v.as_str());

        let target_sound = mod_rule_sound
            .or(sev_sound)
            .unwrap_or("default")
            .to_string();

        let message = NotificationMessage {
            id: notification_id,
            module_id: mod_id.clone(),
            user_id: user_str.to_string(),
            title: final_title,
            body: body_str,
            severity: params.severity,
            category: cat_str,
            entity_id: params.entity_id,
            target_url: params.target_url,
            group_count,
            actions: params.actions,
            acknowledged_at: None,
            acknowledged_by: None,
            escalated_at: None,
            title_template: effective_template,
            created_at: now_ts,
            read_at: None,
            push_eligible: Some(push_elig),
            sound_eligible: Some(sound_elig),
            sound_signal: Some(target_sound),
        };

        // Публикация события в шину EventBus
        if let Ok(payload) = serde_json::to_value(&message) {
            let system_event = SystemEvent::new("core.notifications.created", payload, &mod_id);
            if let Err(e) = self.event_bus.publish(system_event, true) {
                warn!("Failed to publish notification event to EventBus: {}", e);
            }
        }

        info!(
            "Sent notification '{}' (id={}) to user {}",
            message.title, message.id, user_str
        );
        Ok(Some(message))
    }

    /// Получить список уведомлений пользователя с фильтрацией и пагинацией
    pub async fn get_user_notifications(
        &self,
        user_id: &str,
        filter: &NotificationFilter,
    ) -> Result<NotificationListResult> {
        let user_str = user_id.trim();
        let limit = filter.limit.unwrap_or(50).max(1);
        let offset = filter.offset.unwrap_or(0).max(0);

        let pool = match &self.db_pool {
            Some(p) => p,
            None => {
                return Ok(NotificationListResult {
                    items: vec![],
                    total: 0,
                    filtered_total: 0,
                    unread_count: 0,
                    limit,
                    offset,
                })
            }
        };

        let total_row = sqlx::query("SELECT COUNT(*) FROM notifications WHERE user_id = ?")
            .bind(user_str)
            .fetch_one(pool)
            .await?;
        let total: i64 = total_row.get(0);

        let unread_count = self.count_unread_notifications(user_str).await?;

        let mut where_clauses = vec!["user_id = ?".to_string()];
        let mut bind_params: Vec<String> = vec![user_str.to_string()];

        if filter.unread_only.unwrap_or(false) {
            where_clauses.push("read_at IS NULL".to_string());
        }

        if let Some(ref sev) = filter.severity {
            if !sev.trim().is_empty() {
                where_clauses.push("LOWER(severity) = ?".to_string());
                bind_params.push(sev.trim().to_lowercase());
            }
        }

        if let Some(ref cat) = filter.category {
            if !cat.trim().is_empty() {
                where_clauses.push("LOWER(category) = ?".to_string());
                bind_params.push(cat.trim().to_lowercase());
            }
        }

        if let Some(ref search) = filter.search {
            if !search.trim().is_empty() {
                where_clauses.push("(title LIKE ? OR body LIKE ?)".to_string());
                let s_param = format!("%{}%", search.trim());
                bind_params.push(s_param.clone());
                bind_params.push(s_param);
            }
        }

        let where_sql = where_clauses.join(" AND ");

        // Подсчет количества элементов по фильтру
        let count_query_str = format!("SELECT COUNT(*) FROM notifications WHERE {}", where_sql);
        let mut count_query = sqlx::query(&count_query_str);
        for p in &bind_params {
            count_query = count_query.bind(p);
        }
        let count_row = count_query.fetch_one(pool).await?;
        let filtered_total: i64 = count_row.get(0);

        // Получение элементов
        let items_query_str = format!(
            r#"
            SELECT id, module_id, user_id, title, body, severity, category, entity_id, target_url, group_count, actions, acknowledged_at, acknowledged_by, escalated_at, title_template, created_at, read_at
            FROM notifications
            WHERE {}
            ORDER BY id DESC
            LIMIT ? OFFSET ?
            "#,
            where_sql
        );

        let mut items_query = sqlx::query(&items_query_str);
        for p in &bind_params {
            items_query = items_query.bind(p);
        }
        items_query = items_query.bind(limit).bind(offset);

        let rows = items_query.fetch_all(pool).await?;
        let mut items = Vec::with_capacity(rows.len());

        for r in rows {
            let actions_raw: Option<String> = r.try_get("actions").ok().flatten();
            let actions = actions_raw.and_then(|s| serde_json::from_str(&s).ok());

            let sev_str: String = r.try_get("severity").unwrap_or_else(|_| "info".to_string());

            items.push(NotificationMessage {
                id: r.get("id"),
                module_id: r
                    .try_get("module_id")
                    .unwrap_or_else(|_| "core".to_string()),
                user_id: r.try_get("user_id").unwrap_or_default(),
                title: r.try_get("title").unwrap_or_default(),
                body: r.try_get("body").unwrap_or_default(),
                severity: NotificationSeverity::parse(&sev_str),
                category: r
                    .try_get("category")
                    .unwrap_or_else(|_| "system".to_string()),
                entity_id: r.try_get("entity_id").ok().flatten(),
                target_url: r.try_get("target_url").ok().flatten(),
                group_count: r.try_get("group_count").unwrap_or(1),
                actions,
                acknowledged_at: r.try_get("acknowledged_at").ok().flatten(),
                acknowledged_by: r.try_get("acknowledged_by").ok().flatten(),
                escalated_at: r.try_get("escalated_at").ok().flatten(),
                title_template: r.try_get("title_template").ok().flatten(),
                created_at: r.try_get("created_at").unwrap_or(0.0),
                read_at: r.try_get("read_at").ok().flatten(),
                push_eligible: None,
                sound_eligible: None,
                sound_signal: None,
            });
        }

        Ok(NotificationListResult {
            items,
            total,
            filtered_total,
            unread_count,
            limit,
            offset,
        })
    }

    /// Пометить уведомление как прочитанное
    pub async fn mark_as_read(&self, notification_id: i64, user_id: &str) -> Result<bool> {
        let user_str = user_id.trim();
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(false),
        };

        let now_ts = current_timestamp();
        let res = sqlx::query(
            "UPDATE notifications SET read_at = COALESCE(read_at, ?) WHERE id = ? AND user_id = ?",
        )
        .bind(now_ts)
        .bind(notification_id)
        .bind(user_str)
        .execute(pool)
        .await?;

        let updated = res.rows_affected() > 0;
        if updated {
            self.invalidate_unread_cache(Some(user_str));
        }

        Ok(updated)
    }

    /// Пометить уведомление как непрочитанное
    pub async fn mark_as_unread(&self, notification_id: i64, user_id: &str) -> Result<bool> {
        let user_str = user_id.trim();
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(false),
        };

        let res =
            sqlx::query("UPDATE notifications SET read_at = NULL WHERE id = ? AND user_id = ?")
                .bind(notification_id)
                .bind(user_str)
                .execute(pool)
                .await?;

        let updated = res.rows_affected() > 0;
        if updated {
            self.invalidate_unread_cache(Some(user_str));
        }

        Ok(updated)
    }

    /// Пометить все уведомления пользователя как прочитанные
    pub async fn mark_all_as_read(&self, user_id: &str) -> Result<i64> {
        let user_str = user_id.trim();
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(0),
        };

        let now_ts = current_timestamp();
        let res = sqlx::query(
            "UPDATE notifications SET read_at = ? WHERE user_id = ? AND read_at IS NULL",
        )
        .bind(now_ts)
        .bind(user_str)
        .execute(pool)
        .await?;

        let count = res.rows_affected() as i64;
        if count > 0 {
            self.invalidate_unread_cache(Some(user_str));
        }

        Ok(count)
    }

    /// Квитировать уведомление
    pub async fn acknowledge_notification(
        &self,
        notification_id: i64,
        user_id: &str,
    ) -> Result<bool> {
        let user_str = user_id.trim();
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(false),
        };

        let now_ts = current_timestamp();
        let res = sqlx::query(
            "UPDATE notifications SET acknowledged_at = COALESCE(acknowledged_at, ?), acknowledged_by = ? WHERE id = ? AND user_id = ?"
        )
        .bind(now_ts)
        .bind(user_str)
        .bind(notification_id)
        .bind(user_str)
        .execute(pool)
        .await?;

        let updated = res.rows_affected() > 0;
        if updated {
            self.invalidate_unread_cache(Some(user_str));
        }

        Ok(updated)
    }

    /// Квитировать все уведомления пользователя
    pub async fn acknowledge_all_notifications(&self, user_id: &str) -> Result<i64> {
        let user_str = user_id.trim();
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(0),
        };

        let now_ts = current_timestamp();
        let res = sqlx::query(
            "UPDATE notifications SET acknowledged_at = ?, acknowledged_by = ? WHERE user_id = ? AND acknowledged_at IS NULL"
        )
        .bind(now_ts)
        .bind(user_str)
        .bind(user_str)
        .execute(pool)
        .await?;

        let count = res.rows_affected() as i64;
        if count > 0 {
            self.invalidate_unread_cache(Some(user_str));
        }

        Ok(count)
    }

    /// Удалить конкретное уведомление пользователя
    pub async fn delete_notification(&self, notification_id: i64, user_id: &str) -> Result<bool> {
        let user_str = user_id.trim();
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(false),
        };

        let res = sqlx::query("DELETE FROM notifications WHERE id = ? AND user_id = ?")
            .bind(notification_id)
            .bind(user_str)
            .execute(pool)
            .await?;

        let deleted = res.rows_affected() > 0;
        if deleted {
            self.invalidate_unread_cache(Some(user_str));
        }

        Ok(deleted)
    }

    /// Удалить все прочитанные уведомления пользователя
    pub async fn clear_read_notifications(&self, user_id: &str) -> Result<i64> {
        let user_str = user_id.trim();
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(0),
        };

        let res =
            sqlx::query("DELETE FROM notifications WHERE user_id = ? AND read_at IS NOT NULL")
                .bind(user_str)
                .execute(pool)
                .await?;

        let count = res.rows_affected() as i64;
        if count > 0 {
            self.invalidate_unread_cache(Some(user_str));
        }

        Ok(count)
    }

    /// Удалить старые уведомления (retention), за исключением неквитированных/непрочитанных ошибок
    pub async fn prune_notifications(&self, days: i64) -> Result<i64> {
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(0),
        };

        let cutoff = current_timestamp() - (days.max(1) as f64 * 86400.0);
        let res = sqlx::query(
            "DELETE FROM notifications WHERE created_at < ? AND (read_at IS NOT NULL OR LOWER(severity) != 'error')"
        )
        .bind(cutoff)
        .execute(pool)
        .await?;

        let count = res.rows_affected() as i64;
        info!(
            "Pruned {} stale notifications older than {} days",
            count, days
        );
        Ok(count)
    }

    /// Удалить все уведомления от удаленного модуля
    pub async fn cleanup_module_notifications(&self, module_id: &str) -> Result<i64> {
        let mod_str = module_id.trim();
        if mod_str.is_empty() || mod_str == "core" {
            return Ok(0);
        }

        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(0),
        };

        let res = sqlx::query("DELETE FROM notifications WHERE module_id = ?")
            .bind(mod_str)
            .execute(pool)
            .await?;

        let count = res.rows_affected() as i64;
        if count > 0 {
            self.invalidate_unread_cache(None);
            info!(
                "Cleaned up {} notifications for uninstalled module '{}'",
                count, mod_str
            );
        }

        Ok(count)
    }

    /// Обработать эскалацию неквитированных и непрочитанных критических ошибок
    pub async fn process_alert_escalations(&self, escalation_minutes: i64) -> Result<i64> {
        let pool = match &self.db_pool {
            Some(p) => p,
            None => return Ok(0),
        };

        let now_ts = current_timestamp();
        let cutoff = now_ts - (escalation_minutes.max(1) as f64 * 60.0);

        let rows = sqlx::query(
            r#"
            SELECT id, user_id, module_id, title, body, severity, category, entity_id, target_url, group_count, actions, created_at
            FROM notifications
            WHERE LOWER(severity) = 'error'
              AND read_at IS NULL
              AND acknowledged_at IS NULL
              AND escalated_at IS NULL
              AND created_at <= ?
            "#
        )
        .bind(cutoff)
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            return Ok(0);
        }

        let mut escalated_count: i64 = 0;
        for r in rows {
            let n_id: i64 = r.get("id");
            let u_id: String = r.get("user_id");
            let mod_id: String = r
                .try_get("module_id")
                .unwrap_or_else(|_| "core".to_string());

            sqlx::query("UPDATE notifications SET escalated_at = ? WHERE id = ?")
                .bind(now_ts)
                .bind(n_id)
                .execute(pool)
                .await?;

            escalated_count += 1;

            let actions_raw: Option<String> = r.try_get("actions").ok().flatten();
            let actions = actions_raw.and_then(|s| serde_json::from_str(&s).ok());

            let payload_msg = NotificationMessage {
                id: n_id,
                module_id: mod_id.clone(),
                user_id: u_id,
                title: r.try_get("title").unwrap_or_default(),
                body: r.try_get("body").unwrap_or_default(),
                severity: NotificationSeverity::Error,
                category: r
                    .try_get("category")
                    .unwrap_or_else(|_| "system".to_string()),
                entity_id: r.try_get("entity_id").ok().flatten(),
                target_url: r.try_get("target_url").ok().flatten(),
                group_count: r.try_get("group_count").unwrap_or(1),
                actions,
                acknowledged_at: None,
                acknowledged_by: None,
                escalated_at: Some(now_ts),
                title_template: None,
                created_at: r.try_get("created_at").unwrap_or(0.0),
                read_at: None,
                push_eligible: Some(true),
                sound_eligible: Some(true),
                sound_signal: Some("error".to_string()),
            };

            if let Ok(payload) = serde_json::to_value(&payload_msg) {
                let system_event =
                    SystemEvent::new("core.notifications.escalated", payload, &mod_id);
                let _ = self.event_bus.publish(system_event, true);
            }
        }

        if escalated_count > 0 {
            info!(
                "Escalated {} unacknowledged critical notifications older than {} minutes",
                escalated_count, escalation_minutes
            );
        }

        Ok(escalated_count)
    }

    /// Экспортировать лог уведомлений в формате CSV или JSON
    pub async fn export_user_notifications(
        &self,
        user_id: &str,
        export_format: &str,
        filter: &NotificationFilter,
    ) -> Result<(String, String)> {
        let export_filter = NotificationFilter {
            limit: Some(1000),
            offset: Some(0),
            unread_only: filter.unread_only,
            severity: filter.severity.clone(),
            category: filter.category.clone(),
            search: filter.search.clone(),
        };

        let res = self.get_user_notifications(user_id, &export_filter).await?;

        if export_format.to_lowercase().trim() == "json" {
            let json_data = serde_json::to_string_pretty(&res.items)?;
            return Ok((json_data, "application/json".to_string()));
        }

        let mut csv_out = String::from(
            "ID,Module,Title,Body,Severity,Category,Group Count,Created At,Read At,Acknowledged At,Escalated At\n"
        );

        let fmt_time = |ts_opt: Option<f64>| -> String {
            match ts_opt {
                Some(ts) => match Local.timestamp_opt(ts as i64, 0).single() {
                    Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
                    None => String::new(),
                },
                None => String::new(),
            }
        };

        for item in res.items {
            let row_str = format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},\"{}\",\"{}\",\"{}\",\"{}\"\n",
                item.id,
                item.module_id,
                item.title.replace('"', "\"\""),
                item.body.replace('"', "\"\""),
                item.severity.as_str(),
                item.category,
                item.group_count,
                fmt_time(Some(item.created_at)),
                fmt_time(item.read_at),
                fmt_time(item.acknowledged_at),
                fmt_time(item.escalated_at)
            );
            csv_out.push_str(&row_str);
        }

        Ok((csv_out, "text/csv".to_string()))
    }
}

/// Вспомогательная функция отправки уведомления пользователю (1-в-1 с Python notify.notify)
pub async fn notify(
    _pool: &sqlx::SqlitePool,
    params: NotifyParams,
) -> Result<Option<NotificationMessage>> {
    let engine = NotificationEngine::new(crate::bus::EVENT_BUS.clone());
    engine.notify(params).await
}
