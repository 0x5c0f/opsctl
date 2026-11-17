use crate::error::NotifyError;
use crate::Notifier;
use serde::Serialize;

/// 钉钉机器人 webhook 通知
///
/// 文档: https://open.dingtalk.com/document/orgapp/custom-bot-send-message
/// 使用 markdown 消息类型
pub struct DingTalkNotifier {
    webhook_url: String,
}

#[derive(Serialize)]
struct DingTalkPayload {
    msgtype: String,
    markdown: DingTalkMarkdown,
}

#[derive(Serialize)]
struct DingTalkMarkdown {
    title: String,
    text: String,
}

impl DingTalkNotifier {
    pub fn new(webhook_url: &str) -> Self {
        Self {
            webhook_url: webhook_url.to_string(),
        }
    }
}

impl Notifier for DingTalkNotifier {
    fn send(&self, title: &str, message: &str) -> Result<(), NotifyError> {
        let text = format!("## {}\n{}", title, message);
        let payload = DingTalkPayload {
            msgtype: "markdown".into(),
            markdown: DingTalkMarkdown {
                title: title.to_string(),
                text,
            },
        };

        let body = serde_json::to_string(&payload)?;

        let mut resp = ureq::post(&self.webhook_url)
            .header("Content-Type", "application/json")
            .send(body.as_str())
            .map_err(|e| NotifyError::Http(e.to_string()))?;

        let status = resp.status();
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
        "钉钉"
    }
}
