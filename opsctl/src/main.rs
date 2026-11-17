mod cli;
mod handlers;
mod helpers;

use cli::{Cli, Commands, DeploySubcommands, CertSubcommands};
use clap::Parser;
use std::path::Path;

/// opsctl — 统一运维工具集
///
/// 入口：解析 CLI 参数 → 加载配置 → 分发到对应的 handler。
fn main() {
    let cli = Cli::parse();

    // 加载全局配置
    let global_cfg = match &cli.config {
        Some(path) => opsctl_core::GlobalConfig::from_file(Path::new(path)),
        None => opsctl_core::GlobalConfig::load(),
    };

    let global_cfg = match global_cfg {
        Ok(cfg) => cfg,
        Err(e) => {
            // 配置加载失败时仍初始化日志以输出错误
            opsctl_core::logging::init(cli.log_level.as_deref(), "warn");
            tracing::error!("配置加载失败: {e}");
            eprintln!("错误: {e}");
            std::process::exit(1);
        }
    };

    // 初始化日志（优先级: CLI > RUST_LOG > 配置文件）
    opsctl_core::logging::init(cli.log_level.as_deref(), &global_cfg.log_level);

    tracing::debug!("opsctl 启动");

    // 分发子命令
    match &cli.command {
        Commands::Deploy { sub } => match sub {
            DeploySubcommands::Run { project } => {
                handlers::deploy::handle_run(&global_cfg, project.clone());
            }
            DeploySubcommands::Daemon { interval } => {
                handlers::deploy::handle_daemon(&global_cfg, *interval);
            }
            DeploySubcommands::History { project, limit } => {
                handlers::deploy::handle_history(&global_cfg, project.clone(), *limit);
            }
            DeploySubcommands::Rollback { version_id } => {
                handlers::deploy::handle_rollback(&global_cfg, version_id);
            }
            DeploySubcommands::Init { project } => {
                handlers::deploy::handle_deploy_init(&global_cfg, project.clone());
            }
        },
        Commands::Cert { sub } => match sub {
            CertSubcommands::Update { cert_id } => {
                handlers::cert::handle_update(&global_cfg, *cert_id);
            }
            CertSubcommands::List => {
                handlers::cert::handle_list(&global_cfg);
            }
            CertSubcommands::Check { warn_days } => {
                handlers::cert::handle_check(&global_cfg, *warn_days);
            }
        },
        Commands::Dns { sub } => {
            handlers::dns::handle_dns(&global_cfg, sub, cli.json);
        }
        Commands::Init { project, force } => {
            handlers::init::handle_init(*project, *force);
        },
    }
}
