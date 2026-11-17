use crate::error::NotifyError;
use crate::Notifier;
use serde::Serialize;

/// 飞书机器人 webhook 通知
///
/// 文档: https://open.feishu.cn/document/client-docs/bot-v3/add-custom-bot
/// 使用 interactive (卡片) 消息类型
pub struct FeishuNotifier {
    webhook_url: String,
}

#[derive(Serialize)]
struct FeishuPayload {
    msg_type: String,
    card: CardContent,
}

#[derive(Serialize)]
struct CardContent {
    header: CardHeader,
    elements: Vec<CardElement>,
}

#[derive(Serialize)]
struct CardHeader {
    title: CardTitle,
    template: String,
}

#[derive(Serialize)]
struct CardTitle {
    tag: String,
    content: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum CardElement {
    Markdown { tag: String, content: String },
}

impl FeishuNotifier {
    pub fn new(webhook_url: &str) -> Self {
        Self {
            webhook_url: webhook_url.to_string(),
        }
    }
}

impl Notifier for FeishuNotifier {
    fn send(&self, title: &str, message: &str) -> Result<(), NotifyError> {
        let payload = FeishuPayload {
            msg_type: "interactive".into(),
            card: CardContent {
                header: CardHeader {
                    title: CardTitle {
                        tag: "plain_text".into(),
                        content: title.to_string(),
                    },
                    template: "blue".into(),
                },
                elements: vec![
                    CardElement::Markdown {
                        tag: "markdown".into(),
                        content: message.to_string(),
                    },
                ],
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
        "飞书"
    }
}
