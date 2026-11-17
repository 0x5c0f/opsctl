use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use opsctl_core::config::DeployConfigRoot;

/// 守护模式：定时扫描所有项目的 upload_dir
pub fn run_daemon(
    config: &DeployConfigRoot,
    interval_secs: u64,
    running: Arc<AtomicBool>,
) {
    let base_dir = std::path::Path::new(
        config.defaults.base_dir.as_deref().unwrap_or("/data/autoDeploy")
    );
    let interval = Duration::from_secs(interval_secs);

    tracing::info!("守护模式启动: interval={}s, {} 个项目", interval_secs, config.projects.len());

    while running.load(Ordering::Relaxed) {
        for proj in &config.projects {
            let ctx = crate::DeployContext::from_project(proj, base_dir);

            // 检查锁文件
            if ctx.lock_file.exists() {
                continue;
            }

            // 检查是否有匹配的 ZIP
            if let Ok(Some(_zip)) = crate::scanner::find_first_upload(&ctx.upload_dir, &proj.name) {
                tracing::info!("检测到更新: project={}", proj.name);
                let version_id = _zip.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let result = crate::deploy::run_deploy(&ctx, version_id);
                if result.success {
                    tracing::info!("守护模式部署成功: {} ({})", proj.name, version_id);
                } else {
                    tracing::error!("守护模式部署失败: {} — {}", proj.name, result.message);
                }
            }
        }
        std::thread::sleep(interval);
    }

    tracing::info!("守护模式退出");
}
