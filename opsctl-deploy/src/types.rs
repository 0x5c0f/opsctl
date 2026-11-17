use std::path::PathBuf;
use opsctl_core::config::{DeployConfig, DeployType, DeployProject};

/// 部署结果
#[derive(Debug, Clone)]
pub struct DeployResult {
    pub project_name: String,
    pub version_id: String,
    pub success: bool,
    pub message: String,
    pub deploy_type: DeployType,
}

/// 部署上下文（从配置构建而来）
#[derive(Debug, Clone)]
pub struct DeployContext {
    pub project_name: String,
    pub deploy_type: DeployType,
    pub command: String,
    pub health_check_url: Option<String>,
    pub www_uid: Option<u32>,
    pub www_gid: Option<u32>,
    pub keep_versions: usize,
    pub upload_dir: PathBuf,
    pub ready_dir: PathBuf,
    pub enabled_dir: PathBuf,
    pub lock_file: PathBuf,
}

impl DeployContext {
    pub fn from_project(
        proj: &DeployProject,
        base_dir: &std::path::Path,
    ) -> Self {
        let upload_dir = proj.upload_dir.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| base_dir.join("uploads"));

        let ready_dir = proj.ready_dir.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| base_dir.join("ready"));

        let enabled_dir = proj.enabled_dir.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| base_dir.join("enabled"));

        // 锁文件在 uploads/ 下，按项目名隔离
        let lock_file = upload_dir.join(format!(".{}.lock", proj.name));

        DeployContext {
            project_name: proj.name.clone(),
            deploy_type: proj.deploy_type.clone(),
            command: proj.command.clone(),
            health_check_url: proj.health_check_url.clone(),
            www_uid: proj.www_uid,
            www_gid: proj.www_gid,
            keep_versions: proj.keep_versions,
            upload_dir,
            ready_dir,
            enabled_dir,
            lock_file,
        }
    }

    /// 从旧项目配置构建（向后兼容）
    pub fn from_config(
        project_name: &str,
        config: &DeployConfig,
        base_dir: &std::path::Path,
    ) -> Self {
        let upload_dir = config.upload_dir.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| base_dir.join("uploads"));

        let ready_dir = config.ready_dir.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| base_dir.join("ready"));

        let enabled_dir = config.enabled_dir.as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| base_dir.join("enabled"));

        let lock_file = upload_dir.join(format!(".{}.lock", project_name));

        DeployContext {
            project_name: project_name.to_string(),
            deploy_type: config.r#type.clone(),
            command: config.command.clone(),
            health_check_url: config.health_check_url.clone(),
            www_uid: config.www_uid,
            www_gid: config.www_gid,
            keep_versions: config.keep_versions,
            upload_dir,
            ready_dir,
            enabled_dir,
            lock_file,
        }
    }
}
