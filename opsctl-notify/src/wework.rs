use crate::Notifier;
use crate::error::NotifyError;
use serde::Serialize;

/// 企业微信机器人 webhook 通知
///
/// 文档: https://developer.work.weixin.qq.com/document/path/91770
/// 使用 markdown 消息类型
pub struct WeWorkNotifier {
    webhook_url: String,
}

#[derive(Serialize)]
struct WeWorkPayload {
    msgtype: String,
    markdown: MarkdownContent,
}

#[derive(Serialize)]
struct MarkdownContent {
    content: String,
}

impl WeWorkNotifier {
    pub fn new(webhook_url: &str) -> Self {
        Self {
            webhook_url: webhook_url.to_string(),
        }
    }
}

impl Notifier for WeWorkNotifier {
    fn send(&self, title: &str, message: &str) -> Result<(), NotifyError> {
        let content = format!("## {}\n{}", title, message);
        let payload = WeWorkPayload {
            msgtype: "markdown".into(),
            markdown: MarkdownContent { content },
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
        "企业微信"
    }
}
