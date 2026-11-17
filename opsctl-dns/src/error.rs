use thiserror::Error;

#[derive(Debug, Error)]
pub enum DnsError {
    #[error("DNS 配置错误: {0}")]
    Config(String),
    #[error("DNS 参数校验失败: {0}")]
    Validation(String),
    #[error("平台认证失败: {0}")]
    Auth(String),
    #[error("平台限流: {0}")]
    RateLimited(String),
    #[error("平台资源不存在: {0}")]
    NotFound(String),
    #[error("平台不支持该操作: {0}")]
    Unsupported(String),
    #[error("网络错误: {0}")]
    Network(String),
    #[error("平台 API 错误 [{code}]: {message}")]
    Api { code: String, message: String },
}

impl From<std::io::Error> for DnsError {
    fn from(e: std::io::Error) -> Self {
        DnsError::Network(e.to_string())
    }
}
