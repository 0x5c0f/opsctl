use thiserror::Error;

/// opsctl 统一错误类型
#[derive(Debug, Error)]
pub enum OpsctlError {
    #[error("配置错误: {0}")]
    Config(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl From<&str> for OpsctlError {
    fn from(s: &str) -> Self {
        OpsctlError::Other(s.to_string())
    }
}

impl From<String> for OpsctlError {
    fn from(s: String) -> Self {
        OpsctlError::Other(s)
    }
}
