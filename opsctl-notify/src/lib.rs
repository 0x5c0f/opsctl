pub mod dingtalk;
pub mod error;
pub mod feishu;
pub mod messages;
pub mod slack;
pub mod wework;

use error::NotifyError;
use opsctl_core::config::WebhookEntry;

/// Notifier trait — 所有通知平台实现此接口
pub trait Notifier: Send + Sync {
    fn send(&self, title: &str, message: &str) -> Result<(), NotifyError>;
    fn platform_name(&self) -> &'static str;
}

/// 通知管理器 — 管理多个平台的通知发送
pub struct NotifyManager {
    notifiers: Vec<Box<dyn Notifier>>,
}

impl NotifyManager {
    /// 从配置中的 webhook 列表创建通知管理器
    pub fn from_config(webhooks: &[WebhookEntry]) -> Self {
        let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();

        for entry in webhooks {
            match entry.r#type.to_lowercase().as_str() {
                "wework" => {
                    notifiers.push(Box::new(wework::WeWorkNotifier::new(&entry.url)));
                }
                "dingtalk" => {
                    notifiers.push(Box::new(dingtalk::DingTalkNotifier::new(&entry.url)));
                }
                "feishu" => {
                    notifiers.push(Box::new(feishu::FeishuNotifier::new(&entry.url)));
                }
                "slack" => {
                    notifiers.push(Box::new(slack::SlackNotifier::new(&entry.url)));
                }
                other => {
                    tracing::warn!("未知的通知平台类型: {}", other);
                }
            }
        }

        NotifyManager { notifiers }
    }

    /// 发送通知到所有已配置的平台
    ///
    /// 单个平台失败不会影响其他平台的发送。
    /// 返回成功发送的平台数量和失败列表。
    pub fn send_all(&self, title: &str, message: &str) -> (usize, Vec<(String, String)>) {
        let mut success_count = 0;
        let mut failures = Vec::new();

        for notifier in &self.notifiers {
            match notifier.send(title, message) {
                Ok(_) => {
                    tracing::debug!("通知发送成功: {}", notifier.platform_name());
                    success_count += 1;
                }
                Err(e) => {
                    tracing::error!("通知发送失败 ({}): {e}", notifier.platform_name());
                    failures.push((notifier.platform_name().to_string(), e.to_string()));
                }
            }
        }

        (success_count, failures)
    }

    /// 是否配置了任何通知渠道
    pub fn is_empty(&self) -> bool {
        self.notifiers.is_empty()
    }

    /// 已配置的通知渠道数量
    pub fn len(&self) -> usize {
        self.notifiers.len()
    }
}
