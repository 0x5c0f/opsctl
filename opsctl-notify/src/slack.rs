use crate::Notifier;
use crate::error::NotifyError;
use serde::Serialize;

/// Slack Incoming Webhook 通知
///
/// 文档: https://api.slack.com/messaging/webhooks
/// 使用简单文本消息（也可使用 Block Kit）
pub struct SlackNotifier {
    webhook_url: String,
}

#[derive(Serialize)]
struct SlackPayload {
    text: String,
}

impl SlackNotifier {
    pub fn new(webhook_url: &str) -> Self {
        Self {
            webhook_url: webhook_url.to_string(),
        }
    }
}

impl Notifier for SlackNotifier {
    fn send(&self, title: &str, message: &str) -> Result<(), NotifyError> {
        let text = format!("*{}*\n{}", title, message);
        let payload = SlackPayload { text };

        let body = serde_json::to_string(&payload)?;

        let mut resp = ureq::post(&self.webhook_url)
            .header("Content-Type", "application/json")
            .send(body.as_str())
            .map_err(|e| NotifyError::Http(e.to_string()))?;

        let status = resp.status();
        // Slack 返回 200 OK 表示成功
        if status != 200 {
            let resp_body = resp.body_mut().read_to_string().unwrap_or_default();
            return Err(NotifyError::WebhookResponse {
                status: status.as_u16(),
                body: resp_body,
            });
        }

        Ok(())
    }

    fn platform_name(&self) -> &'static str {
        "Slack"
    }
}
