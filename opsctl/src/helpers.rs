use opsctl_core::GlobalConfig;
use opsctl_core::config::DeployProject;
use opsctl_deploy::{DeployContext, DeployResult};
use std::path::Path;

/// 从全局配置构建所有项目的部署上下文
pub fn load_deploy_projects(config: &GlobalConfig) -> Vec<DeployContext> {
    let base_dir = Path::new(
        config
            .deploy
            .defaults
            .base_dir
            .as_deref()
            .unwrap_or("/data/autoDeploy"),
    );
    config
        .deploy
        .projects
        .iter()
        .map(|proj| DeployContext::from_project(proj, base_dir))
        .collect()
}

/// 从全局配置查找指定项目
pub fn find_project<'a>(config: &'a GlobalConfig, name: &str) -> Option<&'a DeployProject> {
    config.deploy.projects.iter().find(|p| p.name == name)
}

pub fn send_deploy_notification(result: &DeployResult, global_cfg: &GlobalConfig) {
    // 版本重复跳过的，不发通知
    if result.message.starts_with("跳过:") {
        return;
    }
    let notifier = opsctl_notify::NotifyManager::from_config(&global_cfg.notify.webhooks);
    if !notifier.is_empty() {
        let (title, message) = opsctl_notify::messages::build_deploy_message(
            &result.project_name,
            &result.version_id,
            result.success,
            &result.deploy_type,
            &result.message,
        );
        let (success_count, failures) = notifier.send_all(&title, &message);
        if success_count > 0 {
            tracing::info!("已发送 {}/{} 通知", success_count, notifier.len());
        }
        for (platform, error) in &failures {
            tracing::warn!("通知发送失败 ({}): {}", platform, error);
        }
    }
}

pub fn print_deploy_result(result: &DeployResult) -> Result<(), opsctl_core::OpsctlError> {
    if result.message.starts_with("跳过:") {
        println!(
            "⏭️  {} (version={}): {}",
            result.project_name, result.version_id, result.message
        );
    } else if result.success {
        println!(
            "✅ 部署成功: {} (version={})",
            result.project_name, result.version_id
        );
    } else {
        return Err(opsctl_core::OpsctlError::Other(format!(
            "部署失败: {} — {}",
            result.project_name, result.message
        )));
    }
    Ok(())
}

/// 按 CLI 参数加载全局配置：显式 --config 优先，否则用默认路径。
pub fn load_config(config_path: Option<&str>) -> Result<GlobalConfig, opsctl_core::OpsctlError> {
    match config_path {
        Some(path) => GlobalConfig::from_file(Path::new(path)),
        None => GlobalConfig::load(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_config_reads_explicit_path() {
        let dir = std::env::temp_dir().join(format!("opsctl_cfg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "[dns]\nnotify = \"none\"\n").unwrap();

        let cfg = load_config(Some(path.to_str().unwrap())).unwrap();
        assert_eq!(cfg.dns.notify, opsctl_core::config::DnsNotifyLevel::None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_reports_missing_explicit_path() {
        let missing = std::env::temp_dir().join("opsctl_definitely_missing_config.toml");
        let _ = std::fs::remove_file(&missing);
        assert!(load_config(Some(missing.to_str().unwrap())).is_err());
    }
}
