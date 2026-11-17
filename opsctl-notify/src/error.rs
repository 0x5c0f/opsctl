use thiserror::Error;

/// 通知模块专用错误类型
#[derive(Debug, Error)]
pub enum NotifyError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 序列化错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Webhook 响应错误: HTTP {status} — {body}")]
    WebhookResponse { status: u16, body: String },

    #[error("配置错误: {0}")]
    Config(String),
}
