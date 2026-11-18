use crate::DeployError;
use crate::types::{DeployContext, DeployResult};
use std::path::{Path, PathBuf};

/// 执行一次完整的部署流程
///
/// 流程：锁定 → 扫描 → 检查ZIP → 解压 → 部署 → 设所有权 → 重启 → 健康检查 → 历史记录 → 清理
/// version_id: 从 ZIP 文件名提取的版本号（如 v2.1）
pub fn run_deploy(ctx: &DeployContext, version_id: &str) -> DeployResult {
    let project = &ctx.project_name;

    tracing::info!(
        "开始部署: project={}, type={}, version={}",
        project,
        ctx.deploy_type,
        version_id
    );

    let result = run_deploy_inner(ctx, version_id);

    match &result {
        Ok(msg) => {
            tracing::info!("部署成功: project={}, version={}", project, version_id);
            DeployResult {
                project_name: project.clone(),
                version_id: version_id.to_string(),
                success: true,
                message: msg.clone(),
                deploy_type: ctx.deploy_type.clone(),
            }
        }
        Err(e) => {
            tracing::error!("部署失败: project={}, error={}", project, e);
            DeployResult {
                project_name: project.clone(),
                version_id: version_id.to_string(),
                success: false,
                message: e.to_string(),
                deploy_type: ctx.deploy_type.clone(),
            }
        }
    }
}

fn run_deploy_inner(ctx: &DeployContext, version_id: &str) -> Result<String, DeployError> {
    let enabled_link = ctx.enabled_dir.join(format!("{}_latest", ctx.project_name));

    // 1. 持久化锁 — 存在则跳过，部署完成后不释放
    acquire_lock(&ctx.lock_file)?;

    // 2. Docker 模式：不扫描 ZIP，直接重启 + 健康检查
    if matches!(ctx.deploy_type, opsctl_core::config::DeployType::Docker) {
        if !ctx.command.is_empty() {
            crate::safety::execute_command(&ctx.command)?;
        }
        if let Some(ref health_url) = ctx.health_check_url {
            crate::health_check::health_check_with_retry(health_url, 30, 5, 6, 3)?;
        }
        return Ok("Docker 重启完成".into());
    }

    // 3. 扫描上传目录
    let zip_path = crate::scanner::find_first_upload(&ctx.upload_dir, &ctx.project_name)?;
    let zip_path = zip_path.ok_or_else(|| {
        DeployError::DeployFailed(format!(
            "未找到匹配的 zip 文件 (project={})",
            ctx.project_name
        ))
    })?;
    tracing::info!("找到上传文件: {:?}", zip_path);

    // 4. 重复版本检测
    let ready_path = ctx.ready_dir.join(version_id);
    if ready_path.exists() {
        tracing::warn!("版本重复，跳过: {:?}", ready_path);
        return Ok(format!("跳过: 版本 {} 已存在", version_id));
    }

    // 5. 检查 ZIP 完整性
    crate::extractor::check_zip_integrity(&zip_path)?;
    tracing::info!("ZIP 完整性检查通过");

    // 6. 解压到 ready_dir/version
    crate::extractor::extract_zip(&zip_path, &ready_path)?;

    // 7. 根据部署类型执行
    match ctx.deploy_type {
        opsctl_core::config::DeployType::Static => {
            crate::static_deploy::static_deploy(&enabled_link, &ready_path)?;
        }
        opsctl_core::config::DeployType::Dynamic => {
            tracing::info!(
                "dynamic 部署: 拷贝覆盖 {:?} → {:?}",
                ready_path,
                &enabled_link
            );
            copy_dir(&ready_path, &enabled_link)?;
        }
        opsctl_core::config::DeployType::Docker => {
            unreachable!("docker mode handled above");
        }
    }

    // 8. 设置文件所有权（对目标目录）
    if let (Some(uid), Some(gid)) = (ctx.www_uid, ctx.www_gid) {
        set_ownership(&enabled_link, uid, gid)?;
    }

    // 9. 执行重启命令
    if !ctx.command.is_empty() {
        crate::safety::execute_command(&ctx.command)?;
    }

    // 9. 健康检查
    if let Some(ref health_url) = ctx.health_check_url {
        crate::health_check::health_check_with_retry(health_url, 30, 5, 6, 3)?;
    }

    // 10. 记录部署历史
    let basedir = &ctx.ready_dir.parent().unwrap_or(Path::new("."));
    crate::history::record_deployment(basedir, &ctx.project_name, version_id, &ready_path)?;

    // 11. 自动清理旧版本
    if ctx.keep_versions > 0 {
        crate::history::cleanup_old_versions(basedir, &ctx.project_name, ctx.keep_versions)?;
    }

    // 12. 删除 zip 文件
    std::fs::remove_file(&zip_path)?;
    tracing::info!("已删除上传文件: {:?}", zip_path);

    Ok(format!("部署完成 (version={})", version_id))
}

// ─── 锁机制（持久化，不自动释放） ─────────────────────────────

fn acquire_lock(lock_file: &Path) -> Result<(), DeployError> {
    if lock_file.exists() {
        return Err(DeployError::LockSkipped(format!(
            "锁文件存在，跳过部署。删除 {} 后触发下次更新",
            lock_file.display()
        )));
    }
    std::fs::write(lock_file, format!("{}", std::process::id()))?;
    tracing::debug!("部署锁已创建: {:?}", lock_file);
    Ok(())
}

// ─── 辅助函数 ───────────────────────────────────────────────

fn set_ownership(path: &Path, uid: u32, gid: u32) -> Result<(), DeployError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let path = ensure_chown_path_allowed(path)?;
        let meta = std::fs::metadata(&path)?;
        if meta.uid() != uid || meta.gid() != gid {
            let output = std::process::Command::new("chown")
                .arg("-R")
                .arg(format!("{}:{}", uid, gid))
                .arg(&path)
                .output()
                .map_err(|e| DeployError::DeployFailed(format!("chown 执行失败: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("chown 失败: {}", stderr.trim());
            } else {
                tracing::debug!("所有权已设置: {}:{} → {:?}", uid, gid, path);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, uid, gid);
        tracing::warn!("非 Unix 系统，跳过 chown");
    }
    Ok(())
}

fn lexical_absolute(path: &Path) -> PathBuf {
    use std::path::Component;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::RootDir => normalized.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }
    normalized
}

fn ensure_chown_path_allowed(path: &Path) -> Result<PathBuf, DeployError> {
    let resolved = path
        .canonicalize()
        .unwrap_or_else(|_| lexical_absolute(path));
    let allowed_root = Path::new("/data");
    if resolved == allowed_root || !resolved.starts_with(allowed_root) {
        return Err(DeployError::DeployFailed(format!(
            "拒绝在允许目录外设置所有权: {:?}（允许根路径: /data）",
            resolved
        )));
    }
    Ok(resolved)
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<(), DeployError> {
    std::fs::create_dir_all(dst)?;

    for entry in walkdir(src)? {
        let path = entry.path();
        let relative = path
            .strip_prefix(src)
            .map_err(|e| DeployError::DeployFailed(format!("路径错误: {e}")))?;
        let target = dst.join(relative);

        if path.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            std::fs::copy(path, &target)?;
        }
    }

    Ok(())
}

fn walkdir(dir: &Path) -> Result<Vec<std::fs::DirEntry>, DeployError> {
    let mut entries = Vec::new();
    walkdir_inner(dir, &mut entries)?;
    Ok(entries)
}

fn walkdir_inner(dir: &Path, entries: &mut Vec<std::fs::DirEntry>) -> Result<(), DeployError> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walkdir_inner(&path, entries)?;
            }
            entries.push(entry);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("opsctl_deploy_{name}_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copy_dir_merges_without_deleting_existing_files() {
        let src = temp_dir("src");
        let dst = temp_dir("dst");
        std::fs::write(src.join("new.dll"), b"new").unwrap();
        std::fs::write(dst.join("keep.txt"), b"keep").unwrap();

        copy_dir(&src, &dst).unwrap();

        assert_eq!(std::fs::read_to_string(dst.join("new.dll")).unwrap(), "new");
        assert_eq!(
            std::fs::read_to_string(dst.join("keep.txt")).unwrap(),
            "keep"
        );

        let _ = std::fs::remove_dir_all(src);
        let _ = std::fs::remove_dir_all(dst);
    }
}

#[cfg(test)]
mod chown_tests {
    use super::*;

    #[test]
    fn chown_allows_data_paths() {
        assert!(ensure_chown_path_allowed(Path::new("/data/app")).is_ok());
    }

    #[test]
    fn chown_rejects_system_paths() {
        assert!(ensure_chown_path_allowed(Path::new("/bin/app")).is_err());
        assert!(ensure_chown_path_allowed(Path::new("/usr/local")).is_err());
        assert!(ensure_chown_path_allowed(Path::new("/data/../etc")).is_err());
    }
}
