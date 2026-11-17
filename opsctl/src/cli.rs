use clap::{Parser, Subcommand};

/// opsctl — 统一运维工具集
#[derive(Parser, Debug)]
#[command(name = "opsctl", version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// 配置文件路径（默认 ~/.opsctl/config.toml）
    #[arg(short = 'c', long)]
    pub config: Option<String>,

    /// 日志级别 (error/warn/info/debug/trace)
    #[arg(short = 'L', long)]
    pub log_level: Option<String>,

    /// 输出 JSON（主要供 dns 子命令脚本消费）
    #[arg(short = 'j', long)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 自动部署管理
    Deploy {
        #[command(subcommand)]
        sub: DeploySubcommands,
    },
    /// SSL 证书管理
    Cert {
        #[command(subcommand)]
        sub: CertSubcommands,
    },
    /// 多平台 DNS 解析管理
    Dns {
        #[command(subcommand)]
        sub: DnsSubcommands,
    },
    /// 初始化配置文件
    Init {
        /// 生成项目级配置 (.opsctl.toml) 而非全局配置
        #[arg(long, short)]
        project: bool,
        /// 强制覆盖已有配置文件
        #[arg(long, short)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum DeploySubcommands {
    /// 执行一次部署（不指定 --project 则部署全部）
    Run {
        /// 只处理指定项目
        #[arg(short = 'p', long)]
        project: Option<String>,
    },
    /// 守护模式，持续监听 upload_dir
    Daemon {
        /// 轮询间隔秒数
        #[arg(short = 'i', long, default_value_t = 10)]
        interval: u64,
    },
    /// 查看部署历史版本
    History {
        /// 只处理指定项目
        #[arg(short = 'p', long)]
        project: Option<String>,
        /// 显示的历史版本条数
        #[arg(short = 'l', long, default_value_t = 20)]
        limit: usize,
    },
    /// 回滚到指定版本
    Rollback {
        /// 要回滚到的版本 ID
        version_id: String,
    },
    /// 初始化所有项目的锁文件（首次使用）
    Init {
        /// 只处理指定项目
        #[arg(short = 'p', long)]
        project: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum CertSubcommands {
    /// 从 certd API 更新证书
    Update {
        /// 只更新指定证书 ID
        #[arg(short = 'i', long)]
        cert_id: Option<u32>,
    },
    /// 列出本地已下载的证书
    List,
    /// 检查证书过期状态
    Check {
        /// 过期告警阈值天数
        #[arg(short = 'w', long, default_value_t = 30)]
        warn_days: u32,
    },
}

#[derive(Subcommand, Debug)]
pub enum DnsSubcommands {
    /// DNS 账户
    Account {
        #[command(subcommand)]
        sub: DnsAccountSubcommands,
    },
    /// 域名查询
    Domain {
        #[command(subcommand)]
        sub: DnsDomainSubcommands,
    },
    /// 解析记录管理
    Record {
        #[command(subcommand)]
        sub: DnsRecordSubcommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum DnsAccountSubcommands {
    /// 列出已配置账户
    List,
    /// 验证账户凭据连通性
    Verify {
        /// 只操作指定账户
        #[arg(short = 'a', long)]
        account: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DnsDomainSubcommands {
    /// 实时列出域名
    List {
        /// 只操作指定账户
        #[arg(short = 'a', long)]
        account: Option<String>,
        /// 按域名关键字过滤
        #[arg(short = 'k', long)]
        keyword: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DnsRecordSubcommands {
    /// 实时列出记录
    List {
        /// 要操作的域名
        #[arg(short = 'd', long)]
        domain: String,
        /// 只操作指定账户
        #[arg(short = 'a', long)]
        account: Option<String>,
        /// 按记录类型过滤（A/AAAA/CNAME/MX/TXT/NS/SRV/CAA）
        #[arg(short = 't', long)]
        r#type: Option<String>,
        /// 按主机记录/记录值/备注过滤
        #[arg(short = 'k', long)]
        keyword: Option<String>,
    },
    /// 创建记录
    Create {
        /// 要操作的域名
        #[arg(short = 'd', long)]
        domain: String,
        /// 主机记录，根记录用 @
        #[arg(short = 'H', long)]
        host: String,
        /// 记录类型（A/AAAA/CNAME/MX/TXT/NS/SRV/CAA）
        #[arg(short = 't', long)]
        r#type: String,
        /// 记录值
        #[arg(short = 'v', long)]
        value: String,
        /// TTL 秒数（1-86400）
        #[arg(short = 'T', long, default_value_t = 600)]
        ttl: u64,
        /// 解析线路，default 会映射为平台默认值
        #[arg(short = 'l', long)]
        line: Option<String>,
        /// Cloudflare 代理状态，其他平台忽略
        #[arg(short = 'p', long)]
        proxied: Option<bool>,
        /// 记录备注，空串表示清除
        #[arg(short = 'r', long)]
        remark: Option<String>,
        /// 只操作指定账户
        #[arg(short = 'a', long)]
        account: Option<String>,
        /// 只预览，不调用写接口
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// 修改记录
    Update {
        /// 平台返回的记录 ID
        record_id: String,
        /// 要操作的域名
        #[arg(short = 'd', long)]
        domain: String,
        /// 主机记录，根记录用 @
        #[arg(short = 'H', long)]
        host: Option<String>,
        /// 记录类型（A/AAAA/CNAME/MX/TXT/NS/SRV/CAA）
        #[arg(short = 't', long)]
        r#type: Option<String>,
        /// 记录值
        #[arg(short = 'v', long)]
        value: Option<String>,
        /// TTL 秒数（1-86400）
        #[arg(short = 'T', long)]
        ttl: Option<u64>,
        /// 解析线路，default 会映射为平台默认值
        #[arg(short = 'l', long)]
        line: Option<String>,
        /// Cloudflare 代理状态，其他平台忽略
        #[arg(short = 'p', long)]
        proxied: Option<bool>,
        /// 记录备注，空串表示清除
        #[arg(short = 'r', long)]
        remark: Option<String>,
        /// 只操作指定账户
        #[arg(short = 'a', long)]
        account: Option<String>,
        /// 只预览，不调用写接口
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// 删除记录
    Delete {
        /// 平台返回的记录 ID
        record_id: String,
        /// 要操作的域名
        #[arg(short = 'd', long)]
        domain: String,
        /// 只操作指定账户
        #[arg(short = 'a', long)]
        account: Option<String>,
        /// 只预览，不调用写接口
        #[arg(short = 'n', long)]
        dry_run: bool,
        /// 非交互终端删除确认，必须显式给出
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// 启用记录
    Enable {
        /// 平台返回的记录 ID
        record_id: String,
        /// 要操作的域名
        #[arg(short = 'd', long)]
        domain: String,
        /// 只操作指定账户
        #[arg(short = 'a', long)]
        account: Option<String>,
        /// 只预览，不调用写接口
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// 停用记录
    Disable {
        /// 平台返回的记录 ID
        record_id: String,
        /// 要操作的域名
        #[arg(short = 'd', long)]
        domain: String,
        /// 只操作指定账户
        #[arg(short = 'a', long)]
        account: Option<String>,
        /// 只预览，不调用写接口
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_short_flags_parse() {
        let cli = Cli::try_parse_from([
            "opsctl", "-L", "debug", "-j", "dns", "account", "verify", "-a", "aliyun-prod",
        ])
        .unwrap();
        assert_eq!(cli.log_level.as_deref(), Some("debug"));
        assert!(cli.json);
        match cli.command {
            Commands::Dns {
                sub: DnsSubcommands::Account {
                    sub: DnsAccountSubcommands::Verify { account },
                },
            } => assert_eq!(account.as_deref(), Some("aliyun-prod")),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn global_flags_after_subcommand_are_rejected() {
        assert!(
            Cli::try_parse_from(["opsctl", "dns", "-c", "/tmp/x.toml", "account", "list"])
                .is_err()
        );
        assert!(Cli::try_parse_from(["opsctl", "deploy", "-j", "run"]).is_err());
        assert!(Cli::try_parse_from(["opsctl", "cert", "-L", "debug", "check"]).is_err());
    }

    #[test]
    fn dns_record_create_short_flags_parse() {
        let cli = Cli::try_parse_from([
            "opsctl", "-j", "dns", "record", "create", "-d", "example.com", "-H", "www", "-t",
            "A", "-v", "1.2.3.4", "-T", "300", "-l", "default", "-p", "true", "-r", "web", "-a",
            "cf-prod", "-n",
        ])
        .unwrap();
        assert!(cli.json);
        match cli.command {
            Commands::Dns {
                sub: DnsSubcommands::Record {
                    sub: DnsRecordSubcommands::Create {
                        domain,
                        host,
                        r#type,
                        value,
                        ttl,
                        line,
                        proxied,
                        remark,
                        account,
                        dry_run,
                    },
                },
            } => {
                assert_eq!(domain, "example.com");
                assert_eq!(host, "www");
                assert_eq!(r#type, "A");
                assert_eq!(value, "1.2.3.4");
                assert_eq!(ttl, 300);
                assert_eq!(line.as_deref(), Some("default"));
                assert_eq!(proxied, Some(true));
                assert_eq!(remark.as_deref(), Some("web"));
                assert_eq!(account.as_deref(), Some("cf-prod"));
                assert!(dry_run);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn existing_commands_short_flags_parse() {
        let cli = Cli::try_parse_from(["opsctl", "deploy", "run", "-p", "myapp"]).unwrap();
        match cli.command {
            Commands::Deploy {
                sub: DeploySubcommands::Run { project },
            } => assert_eq!(project.as_deref(), Some("myapp")),
            other => panic!("unexpected command: {other:?}"),
        }

        let cli = Cli::try_parse_from(["opsctl", "deploy", "daemon", "-i", "5"]).unwrap();
        match cli.command {
            Commands::Deploy {
                sub: DeploySubcommands::Daemon { interval },
            } => assert_eq!(interval, 5),
            other => panic!("unexpected command: {other:?}"),
        }

        let cli =
            Cli::try_parse_from(["opsctl", "deploy", "history", "-p", "myapp", "-l", "7"]).unwrap();
        match cli.command {
            Commands::Deploy {
                sub: DeploySubcommands::History { project, limit },
            } => {
                assert_eq!(project.as_deref(), Some("myapp"));
                assert_eq!(limit, 7);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let cli = Cli::try_parse_from(["opsctl", "cert", "update", "-i", "3"]).unwrap();
        match cli.command {
            Commands::Cert {
                sub: CertSubcommands::Update { cert_id },
            } => assert_eq!(cert_id, Some(3)),
            other => panic!("unexpected command: {other:?}"),
        }

        let cli = Cli::try_parse_from(["opsctl", "cert", "check", "-w", "45"]).unwrap();
        match cli.command {
            Commands::Cert {
                sub: CertSubcommands::Check { warn_days },
            } => assert_eq!(warn_days, 45),
            other => panic!("unexpected command: {other:?}"),
        }

        let cli = Cli::try_parse_from(["opsctl", "init", "-p", "-f"]).unwrap();
        match cli.command {
            Commands::Init { project, force } => {
                assert!(project);
                assert!(force);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn help_texts_describe_parameters() {
        use clap::CommandFactory;

        let mut cmd = Cli::command();
        let root_help = cmd.render_help().to_string();
        for phrase in ["配置文件路径", "日志级别", "输出 JSON"] {
            assert!(root_help.contains(phrase), "missing root help: {phrase}");
        }

        let dns = cmd.find_subcommand_mut("dns").unwrap();
        let record = dns.find_subcommand_mut("record").unwrap();
        let create_help = record
            .find_subcommand_mut("create")
            .unwrap()
            .render_help()
            .to_string();
        for phrase in [
            "要操作的域名",
            "主机记录，根记录用",
            "记录类型",
            "记录值",
            "TTL 秒数",
            "解析线路",
            "Cloudflare 代理状态",
            "记录备注",
            "只操作指定账户",
            "只预览",
        ] {
            assert!(create_help.contains(phrase), "missing create help: {phrase}");
        }

        let delete_help = record
            .find_subcommand_mut("delete")
            .unwrap()
            .render_help()
            .to_string();
        assert!(delete_help.contains("平台返回的记录 ID"));
        assert!(delete_help.contains("非交互终端删除确认"));

        let deploy = cmd.find_subcommand_mut("deploy").unwrap();
        let run_help = deploy
            .find_subcommand_mut("run")
            .unwrap()
            .render_help()
            .to_string();
        assert!(run_help.contains("只处理指定项目"));
        let rollback_help = deploy
            .find_subcommand_mut("rollback")
            .unwrap()
            .render_help()
            .to_string();
        assert!(rollback_help.contains("要回滚到的版本 ID"));

        let cert = cmd.find_subcommand_mut("cert").unwrap();
        let check_help = cert
            .find_subcommand_mut("check")
            .unwrap()
            .render_help()
            .to_string();
        assert!(check_help.contains("过期告警阈值天数"));
    }
}
