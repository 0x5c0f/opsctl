use thiserror::Error;

#[derive(Debug, Error)]
pub enum CertError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("API 错误: {0}")]
    Api(String),

    #[error("证书解析错误: {0}")]
    CertParse(String),

    #[error("配置错误: {0}")]
    Config(String),
}

impl From<opsctl_core::OpsctlError> for CertError {
    fn from(e: opsctl_core::OpsctlError) -> Self {
        CertError::Config(e.to_string())
    }
}
