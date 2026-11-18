use std::path::Path;

/// 实现 static 部署（软链接切换）
///
/// 将 enabled_dir 的软链接指向新的 ready_dir。
/// 流程：删除旧软链接 → 创建新软链接
pub fn static_deploy(enabled_dir: &Path, ready_dir: &Path) -> Result<(), crate::DeployError> {
    if !ready_dir.exists() {
        return Err(crate::DeployError::DeployFailed(format!(
            "目标版本目录不存在: {:?}",
            ready_dir
        )));
    }

    // 确保 enabled 的父目录存在
    if let Some(parent) = enabled_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 如果 enabled_dir 已存在（是软链接或目录），先删除
    if enabled_dir.exists() || enabled_dir.is_symlink() {
        if enabled_dir.is_symlink() || enabled_dir.is_dir() {
            std::fs::remove_dir_all(enabled_dir)?;
        } else {
            std::fs::remove_file(enabled_dir)?;
        }
    }

    // 创建新软链接
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(ready_dir, enabled_dir)?;
    }
    #[cfg(not(unix))]
    {
        // 非 Unix 系统：使用目录拷贝作为 fallback
        crate::deploy::copy_dir(ready_dir, enabled_dir)?;
    }

    tracing::info!("软链接已更新: {:?} → {:?}", enabled_dir, ready_dir);
    Ok(())
}
