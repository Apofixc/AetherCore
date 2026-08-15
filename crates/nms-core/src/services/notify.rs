//! # Системный сервис рассылки уведомлений и алертов (NotifyService)
//!
//! Обеспечивает отправку тревожных сообщений, уведомлений о сбоях сетевых устройств
//! в системный журнал и во внешние системы через Webhooks (например, Telegram, Slack, Mattermost).

use nms_common::error::Result;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

/// Категория важности аварийного уведомления
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    /// Информационное сообщение (нормальное функционирование, восстановление линка)
    Info,
    /// Предупреждение (деградация сервиса, высокий RTT, потеря части пакетов)
    Warning,
    /// Критическая авария (устройство недоступно, отказ сервиса)
    Critical,
}

/// Модель системного уведомления/алерта
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMessage {
    /// Заголовок уведомления
    pub title: String,
    /// Подробный текст оповещения
    pub body: String,
    /// Уровень важности события ([`AlertSeverity`])
    pub severity: AlertSeverity,
    /// Источник оповещения (например, `"plugin.snmp"` или `"system.ping"`)
    pub source: String,
}

/// Сервис отправки и маршрутизации уведомлений
#[derive(Debug, Clone, Default)]
pub struct NotifyService {
    http_client: reqwest::Client,
}

impl NotifyService {
    /// Создать новый экземпляр NotifyService с настроенным HTTP-клиентом
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Отправить уведомление через системный журнал и внешний Webhook
    ///
    /// # Аргументы
    /// * `alert` — Сообщение оповещения ([`AlertMessage`]).
    /// * `webhook_url` — Опциональный HTTP/HTTPS URL вебхука для доставки алерта.
    pub async fn send_alert(&self, alert: AlertMessage, webhook_url: Option<&str>) -> Result<()> {
        match alert.severity {
            AlertSeverity::Info => {
                info!(target: "nms::notify", "[INFO] [{}] {}: {}", alert.source, alert.title, alert.body);
            }
            AlertSeverity::Warning => {
                warn!(target: "nms::notify", "[WARN] [{}] {}: {}", alert.source, alert.title, alert.body);
            }
            AlertSeverity::Critical => {
                error!(target: "nms::notify", "[CRITICAL] [{}] {}: {}", alert.source, alert.title, alert.body);
            }
        }

        // Если указан URL для вебхука — отправляем асинхронный HTTP POST
        if let Some(url) = webhook_url {
            let client = self.http_client.clone();
            let payload = serde_json::json!({
                "title": alert.title,
                "body": alert.body,
                "severity": alert.severity,
                "source": alert.source,
            });
            let url = url.to_string();

            tokio::spawn(async move {
                if let Err(e) = client.post(&url).json(&payload).send().await {
                    warn!("Failed to dispatch webhook alert to {}: {}", url, e);
                }
            });
        }

        Ok(())
    }
}
