use std::path::Path;
use opsctl_core::GlobalConfig;
use opsctl_deploy::{DeployContext, DeployResult};
use opsctl_core::config::DeployProject;

/// 从全局配置构建所有项目的部署上下文
pub fn load_deploy_projects(config: &GlobalConfig) -> Vec<DeployContext> {
    let base_dir = Path::new(
        config.deploy.defaults.base_dir.as_deref().unwrap_or("/data/autoDeploy")
    );
    config.deploy.projects.iter()
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

pub fn print_deploy_result(result: &DeployResult) {
    if result.message.starts_with("跳过:") {
        println!("⏭️  {} (version={}): {}", result.project_name, result.version_id, result.message);
    } else if result.success {
        println!("✅ 部署成功: {} (version={})", result.project_name, result.version_id);
    } else {
        eprintln!("❌ 部署失败: {} — {}", result.project_name, result.message);
        std::process::exit(1);
    }
}
