use opsctl_core::config::DeployType;
use std::time::SystemTime;

/// 构建部署通知消息
pub fn build_deploy_message(
    project: &str,
    version_id: &str,
    success: bool,
    deploy_type: &DeployType,
    message: &str,
) -> (String, String) {
    let status_emoji = if success { "✅" } else { "❌" };
    let status_text = if success {
        "部署成功"
    } else {
        "部署失败"
    };
    let now = format_timestamp();

    let title = format!("{} {} · {}", status_emoji, status_text, project);

    let body = format!(
        "**项目**: {}\n**版本**: {}\n**模式**: {}\n**结果**: {}\n**时间**: {}\n\n{}",
        project, version_id, deploy_type, status_text, now, message,
    );

    (title, body)
}

/// 构建证书更新通知消息
pub fn build_cert_message(domain: &str, success: bool, message: &str) -> (String, String) {
    let status_emoji = if success { "🔒" } else { "❌" };
    let status_text = if success {
        "证书更新成功"
    } else {
        "证书更新失败"
    };
    let now = format_timestamp();

    let title = format!("{} {} · {}", status_emoji, status_text, domain);

    let body = format!(
        "**域名**: {}\n**结果**: {}\n**时间**: {}\n\n{}",
        domain, status_text, now, message,
    );

    (title, body)
}

/// 构建证书检查通知消息
pub fn build_cert_check_message(summary: &str, details: &str) -> (String, String) {
    let now = format_timestamp();
    let title = format!("🔍 证书检查报告 · {}", now);

    let body = format!("**概要**\n{}\n\n**详情**\n{}", summary, details);

    (title, body)
}

fn format_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let mins = (time_secs % 3600) / 60;
    let secs_remain = time_secs % 60;

    format!("{:02}:{:02}:{:02}", hours, mins, secs_remain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_deploy_message_success() {
        let (title, body) =
            build_deploy_message("myapp", "v1", true, &DeployType::Static, "deploy ok");
        assert!(title.contains("✅"));
        assert!(title.contains("myapp"));
        assert!(body.contains("myapp"));
        assert!(body.contains("v1"));
        assert!(body.contains("static"));
        assert!(body.contains("deploy ok"));
    }

    #[test]
    fn test_build_deploy_message_failure() {
        let (title, body) =
            build_deploy_message("myapp", "v2", false, &DeployType::Docker, "timeout");
        assert!(title.contains("❌"));
        assert!(body.contains("docker"));
        assert!(body.contains("timeout"));
    }

    #[test]
    fn test_build_cert_message_success() {
        let (title, body) = build_cert_message("example.com", true, "OK");
        assert!(title.contains("🔒"));
        assert!(title.contains("example.com"));
        assert!(body.contains("example.com"));
    }

    #[test]
    fn test_build_cert_message_failure() {
        let (title, body) = build_cert_message("bad.com", false, "DNS failed");
        assert!(title.contains("❌"));
        assert!(body.contains("DNS failed"));
    }

    #[test]
    fn test_build_cert_check_message() {
        let (title, body) = build_cert_check_message("2 expiring", "example.com");
        assert!(title.contains("🔍"));
        assert!(body.contains("2 expiring"));
        assert!(body.contains("example.com"));
    }
}
