use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// 初始化 tracing 日志系统
///
/// 优先级: --log-level CLI 参数 > RUST_LOG 环境变量 > 配置文件 log_level
pub fn init(level_override: Option<&str>, config_level: &str) {
    let mut filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config_level));

    if let Some(level) = level_override
        && let Ok(directive) = level.parse::<tracing_subscriber::filter::Directive>()
    {
        filter = filter.add_directive(directive);
    }

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true).with_thread_ids(false))
        .with(filter)
        .init();
}
