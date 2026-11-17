use thiserror::Error;

/// 部署模块专用错误类型
#[derive(Debug, Error)]
pub enum DeployError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("ZIP 处理错误: {0}")]
    Zip(String),

    #[error("锁定失败: {0}")]
    LockFailed(String),

    #[error("健康检查失败: {0}")]
    HealthCheck(String),

    #[error("命令安全检查失败: {0}")]
    CommandRejected(String),

    #[error("部署跳过: {0}")]
    LockSkipped(String),

    #[error("部署失败: {0}")]
    DeployFailed(String),
}

impl From<opsctl_core::OpsctlError> for DeployError {
    fn from(e: opsctl_core::OpsctlError) -> Self {
        DeployError::Config(e.to_string())
    }
}

impl From<zip::result::ZipError> for DeployError {
    fn from(e: zip::result::ZipError) -> Self {
        DeployError::Zip(e.to_string())
    }
}
