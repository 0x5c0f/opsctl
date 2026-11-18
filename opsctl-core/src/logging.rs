use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// 构建日志过滤器
///
/// 优先级: --log-level CLI 参数 > RUST_LOG 环境变量 > 配置文件 log_level
pub fn build_filter(level_override: Option<&str>, config_level: &str) -> EnvFilter {
    let mut filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config_level));

    if let Some(level) = level_override
        && let Some(directive) = parse_level_directive(level)
    {
        filter = filter.add_directive(directive);
    }

    filter
}

/// 只接受纯日志级别（error/warn/info/debug/trace）；其他输入忽略。
///
/// 注意：`"bogus".parse::<Directive>()` 会成功并得到一个 `bogus=trace` 的
/// target 过滤指令，会意外屏蔽其他日志，因此必须先校验级别本身。
fn parse_level_directive(level: &str) -> Option<tracing_subscriber::filter::Directive> {
    let normalized = level.trim().to_ascii_lowercase();
    if !matches!(
        normalized.as_str(),
        "off" | "error" | "warn" | "info" | "debug" | "trace"
    ) {
        tracing::warn!("忽略无效日志级别: {level}");
        return None;
    }
    normalized.parse().ok()
}

/// 初始化 tracing 日志系统
///
/// 日志固定写入 stderr，避免污染 stdout 上的命令输出（例如 `--json`）。
pub fn init(level_override: Option<&str>, config_level: &str) {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_writer(std::io::stderr),
        )
        .with(build_filter(level_override, config_level))
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_level_overrides_config_level() {
        let filter = build_filter(Some("debug"), "error");
        assert_eq!(filter.to_string(), "debug");
    }

    #[test]
    fn config_level_used_without_override() {
        let filter = build_filter(None, "warn");
        assert_eq!(filter.to_string(), "warn");
    }

    #[test]
    fn invalid_override_falls_back_to_config_level() {
        let filter = build_filter(Some("not-a-level"), "info");
        assert_eq!(filter.to_string(), "info");
    }
}
