pub mod cert;
pub mod deploy;
pub mod dns;
pub mod init;

/// 命令处理结果。handler 不再直接终止进程，错误统一冒泡到 `main`。
pub type HandlerResult = Result<(), opsctl_core::OpsctlError>;
