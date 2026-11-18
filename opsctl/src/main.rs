mod cli;
mod handlers;
mod helpers;

use clap::Parser;
use cli::Cli;

/// opsctl — 统一运维工具集。错误出口集中在 main，handler 不终止进程。
fn main() {
    let cli = Cli::parse();

    let global_cfg = match helpers::load_config(cli.config.as_deref()) {
        Ok(cfg) => cfg,
        Err(e) => {
            opsctl_core::logging::init(cli.log_level.as_deref(), "warn");
            tracing::error!("配置加载失败: {e}");
            std::process::exit(1);
        }
    };

    opsctl_core::logging::init(cli.log_level.as_deref(), &global_cfg.log_level);
    tracing::debug!("opsctl 启动");

    if let Err(e) = cli::dispatch(&global_cfg, &cli) {
        tracing::error!("{e}");
        eprintln!("❌ {e}");
        std::process::exit(1);
    }
}
