//! # Системный сервис рассылки уведомлений и алертов (NotifyService)
//!
//! Обеспечивает отправку тревожных сообщений, уведомлений о сбоях сетевых устройств
//! в системный журнал и во внешние системы через Webhooks (например, Telegram, Slack, Mattermost).

use nms_common::error::Result;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

/// Категория важности аварийного уведомления / алерта
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    /// Информационное сообщение (нормальное функционирование, восстановление доступности линка)
    Info,
    /// Предупреждение (деградация сервиса, высокий RTT, потеря части сетевых пакетов)
    Warning,
    /// Критическая авария (устройство недоступно, отказ сервиса, сбой электропитания)
    Critical,
}

/// Модель системного уведомления/алерта
///
/// # Примеры
/// ```rust
/// use nms_core::services::notify::{AlertMessage, AlertSeverity};
///
/// let alert = AlertMessage {
///     title: "Хост 192.168.1.1 недоступен".into(),
///     body: "Потеряно 100% пакетов за последние 60 секунд".into(),
///     severity: AlertSeverity::Critical,
///     source: "ping-collector".into(),
/// };
/// assert_eq!(alert.severity, AlertSeverity::Critical);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertMessage {
    /// Краткий заголовок уведомления
    pub title: String,
    /// Подробный текст оповещения
    pub body: String,
    /// Уровень важности события ([`AlertSeverity`])
    pub severity: AlertSeverity,
    /// Идентификатор подсистемы или плагина-источника (например, `"plugin.snmp"` или `"system.ping"`)
    pub source: String,
}

/// Сервис отправки и маршрутизации системных уведомлений и алертов
#[derive(Debug, Clone, Default)]
pub struct NotifyService {
    http_client: reqwest::Client,
}

impl NotifyService {
    /// Создать новый экземпляр [`NotifyService`] с настроенным HTTP-клиентом (таймаут 5 сек)
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
    /// В зависимости от важности алерта выполняет логирование с соответствующим уровнем (`info!`, `warn!`, `error!`).
    /// Если передан `webhook_url`, выполняет неблокирующую асинхронную отправку HTTP POST с JSON полезной нагрузкой.
    ///
    /// # Аргументы
    /// * `alert` — Сообщение оповещения ([`AlertMessage`]).
    /// * `webhook_url` — Опциональный HTTP/HTTPS URL вебхука для доставки алерта во внешнюю систему.
    ///
    /// # Возвращаемое значение
    /// `Ok(())` при успешной постановке в обработку.
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
