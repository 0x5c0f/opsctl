use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use opsctl_core::GlobalConfig;
use opsctl_core::config::DeployType;
use opsctl_deploy::DeployContext;

/// 处理 `opsctl deploy run`
pub fn handle_run(global_cfg: &GlobalConfig, project: Option<String>) {
    if let Some(ref proj_name) = project {
        let Some(proj) = crate::helpers::find_project(global_cfg, proj_name) else {
            eprintln!("错误: 未找到项目 '{}'", proj_name);
            std::process::exit(1);
        };
        let base_dir = std::path::Path::new(
            global_cfg.deploy.defaults.base_dir.as_deref().unwrap_or("/data/autoDeploy")
        );
        let ctx = DeployContext::from_project(proj, base_dir);
        deploy_one(&ctx, global_cfg);
    } else {
        let ctxs = crate::helpers::load_deploy_projects(global_cfg);
        if ctxs.is_empty() {
            eprintln!("警告: 未配置任何部署项目 (请在 ~/.opsctl/config.toml 中设置 [[deploy.projects]])");
            return;
        }
        for ctx in &ctxs {
            deploy_one(ctx, global_cfg);
        }
    }
}

fn deploy_one(ctx: &DeployContext, global_cfg: &GlobalConfig) {
    // 检查持久化锁
    if ctx.lock_file.exists() {
        println!("⏭️  [{}] 锁文件存在，跳过", ctx.project_name);
        return;
    }

    // Docker 模式：跳过 ZIP 扫描，直接重启
    let version_id: String;
    let mut zip_file: Option<std::path::PathBuf> = None;
    if matches!(ctx.deploy_type, opsctl_core::config::DeployType::Docker) {
        version_id = "docker".into();
    } else {
        // Static / Dynamic：扫描上传目录
        let zip_path = match opsctl_deploy::scanner::find_first_upload(&ctx.upload_dir, &ctx.project_name) {
            Ok(Some(p)) => p,
            Ok(None) => {
                println!("⏭️  [{}] 无待部署 ZIP，已重建锁", ctx.project_name);
                let _ = std::fs::write(&ctx.lock_file, format!("{}", std::process::id()));
                return;
            }
            Err(e) => {
                eprintln!("❌ [{}] 扫描失败: {}，已重建锁", ctx.project_name, e);
                let _ = std::fs::write(&ctx.lock_file, format!("{}", std::process::id()));
                return;
            }
        };
        version_id = zip_path.file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        zip_file = Some(zip_path);
    }

    let result = opsctl_deploy::run_deploy(ctx, &version_id);
    // 部署失败且是 ZIP 损坏时，删除坏包避免卡住后续部署
    if !result.success
        && result.message.contains("ZIP")
        && let Some(ref p) = zip_file
    {
        let _ = std::fs::remove_file(p);
        tracing::warn!("已删除损坏的 ZIP: {:?}", p);
    }
    crate::helpers::send_deploy_notification(&result, global_cfg);
    crate::helpers::print_deploy_result(&result);
}

/// 处理 `opsctl deploy daemon`
pub fn handle_daemon(global_cfg: &GlobalConfig, interval: u64) {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    match ctrlc::set_handler(move || {
        tracing::info!("收到退出信号，正在停止守护模式...");
        r.store(false, Ordering::Relaxed);
    }) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("错误: 无法设置信号处理器: {e}");
            std::process::exit(1);
        }
    }

    opsctl_deploy::daemon::run_daemon(&global_cfg.deploy, interval, running);
}

/// 处理 `opsctl deploy history`
pub fn handle_history(global_cfg: &GlobalConfig, project: Option<String>, limit: usize) {
    let basedir = std::path::Path::new(
        global_cfg.deploy.defaults.base_dir.as_deref().unwrap_or("/data/autoDeploy")
    );

    if let Some(ref proj) = project {
        print_history(basedir, proj, limit);
    } else {
        let mut count = 0usize;
        for proj in &global_cfg.deploy.projects {
            if print_history(basedir, &proj.name, limit) {
                count += 1;
                println!();
            }
        }
        if count == 0 {
            println!("暂无部署历史记录");
        }
    }
}

fn print_history(basedir: &std::path::Path, project: &str, limit: usize) -> bool {
    let records = match opsctl_deploy::get_project_history(basedir, project, limit) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("错误: {e}");
            return false;
        }
    };
    if records.is_empty() {
        return false;
    }
    println!("{:<25} {:<21} {:<14} 项目", "版本ID", "时间", "状态");
    println!("{}", "-".repeat(75));
    for line in opsctl_deploy::format_history(&records) {
        println!("{}", line);
    }
    true
}

pub(crate) fn rollback_support_error(deploy_type: DeployType) -> Option<&'static str> {
    match deploy_type {
        DeployType::Static => None,
        DeployType::Dynamic => Some("dynamic 模式不支持回滚"),
        DeployType::Docker => Some("docker 模式不支持回滚"),
    }
}

/// 处理 `opsctl deploy rollback`
pub fn handle_rollback(global_cfg: &GlobalConfig, version_id: &str) {
    let basedir = std::path::Path::new(
        global_cfg.deploy.defaults.base_dir.as_deref().unwrap_or("/data/autoDeploy")
    );
    // 从历史记录中获取项目名以匹配配置
    let proj_name = opsctl_deploy::load_history(basedir)
        .ok().as_ref()
        .and_then(|r| r.iter().find(|v| v.id == version_id))
        .map(|v| v.project.clone());

    let project = proj_name
        .as_ref()
        .and_then(|name| crate::helpers::find_project(global_cfg, name));
    if let Some(proj) = project
        && let Some(reason) = rollback_support_error(proj.deploy_type.clone())
    {
        eprintln!("❌ 回滚失败: {reason}");
        std::process::exit(1);
    }

    match opsctl_deploy::rollback_to(basedir, version_id) {
        Ok(_) => {
            println!("✅ 回滚成功到版本: {}", version_id);
            // 执行重启 + 健康检查
            if let Some(proj) = project {
                let ctx = DeployContext::from_project(proj, basedir);
                if !ctx.command.is_empty() {
                    tracing::info!("回滚后执行重启: {}", ctx.command);
                    if let Err(e) = opsctl_deploy::safety::execute_command(&ctx.command) {
                        tracing::error!("回滚后重启失败: {}", e);
                    }
                }
                if let Some(ref url) = ctx.health_check_url {
                    let _ = opsctl_deploy::health_check::health_check_with_retry(url, 30, 3, 3, 2);
                }
            }
        }
        Err(e) => {
            eprintln!("❌ 回滚失败: {e}");
            std::process::exit(1);
        }
    }
}

/// 处理 `opsctl deploy init` — 初始化所有项目的锁文件
pub fn handle_deploy_init(global_cfg: &GlobalConfig, project: Option<String>) {
    let base_dir = std::path::Path::new(
        global_cfg.deploy.defaults.base_dir.as_deref().unwrap_or("/data/autoDeploy")
    );
    let uploads = base_dir.join("uploads");
    std::fs::create_dir_all(&uploads).unwrap_or_else(|e| {
        eprintln!("错误: 创建 uploads 目录失败: {e}");
        std::process::exit(1);
    });

    if let Some(ref proj_name) = project {
        let lock_file = uploads.join(format!(".{}.lock", proj_name));
        create_lock(&lock_file, proj_name);
    } else {
        let mut count = 0usize;
        for proj in &global_cfg.deploy.projects {
            let lock_file = uploads.join(format!(".{}.lock", proj.name));
            if create_lock(&lock_file, &proj.name) {
                count += 1;
            }
        }
        if count == 0 {
            println!("所有项目锁文件均已存在，无需初始化");
        }
    }
}

fn create_lock(lock_file: &std::path::Path, proj_name: &str) -> bool {
    if lock_file.exists() {
        println!("⏭️  [{}] 已存在", proj_name);
        false
    } else {
        std::fs::write(lock_file, format!("{}", std::process::id()))
            .unwrap_or_else(|e| { eprintln!("❌ [{}] 创建失败: {e}", proj_name); std::process::exit(1); });
        println!("✅ [{}] 锁文件已创建", proj_name);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opsctl_core::config::DeployType;

    #[test]
    fn rollback_is_only_supported_for_static() {
        assert!(rollback_support_error(DeployType::Static).is_none());
        assert!(rollback_support_error(DeployType::Dynamic).is_some());
        assert!(rollback_support_error(DeployType::Docker).is_some());
    }
}
