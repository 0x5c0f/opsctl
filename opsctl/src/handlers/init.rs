use std::path::Path;

use opsctl_core::OpsctlError;

use super::HandlerResult;

/// 全局配置模板 (~/.opsctl/config.toml)
const GLOBAL_CONFIG_TEMPLATE: &str = r#"# opsctl 全局配置
# 更多选项见 README.md

# 日志级别 (error/warn/info/debug/trace)
# log_level = "info"

# ── 通知 ──────────────────────────────────────────────────────
# 部署 / 证书操作后自动发送通知到以下 webhook
# [[notify.webhooks]]
# type = "wework"     # wework | dingtalk | feishu | slack
# url  = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxx"

# ── SSL 证书 ──────────────────────────────────────────────────
[cert]
# certd 平台地址（必填）
# certd_base_url = "http://127.0.0.1:7001"
# certd_key_id = "your-key-id"
# certd_key_secret = "your-key-secret"
#
# # 要下载的证书 ID 列表
# cert_ids = [1, 2, 3]
#
# # 证书存储位置（默认 ~/.opsctl/certificates）
# cert_dir = "/opt/certs"
#
# # 启用的证书格式 (crt / key / ic / oc / pfx / der / jks / one)
# formats = ["crt", "key"]   # crt/key/ic/oc/one 为 PEM 文本；pfx/der/jks 为 Base64 内容；p7b 为文本
#
# # 按域名的重启命令
# [cert.domain_reload_commands]
# "example.com" = "systemctl reload nginx"

# ── DNS 多平台解析管理 ────────────────────────────────────────
# 第一版支持 aliyun / dnspod / cloudflare，纯实时调用，无本地缓存。
# 账户示例保持注释；取消注释并填入真实凭据后即可使用 dns 命令。
[dns]
notify = "failure"                 # none | failure | all
#
# [[dns.accounts]]
# name = "aliyun-prod"
# provider = "aliyun"
# access_key_id = "..."
# access_key_secret = "..."
#
# [[dns.accounts]]
# name = "dnspod-prod"
# provider = "dnspod"
# secret_id = "..."
# secret_key = "..."
#
# [[dns.accounts]]
# name = "cf-prod"
# provider = "cloudflare"
# api_token = "..."

# ── 部署项目列表 ──────────────────────────────────────────────
# 每个 [[deploy.projects]] 段定义一个项目
# ZIP 命名规范: {项目名}_{版本号}.zip（如 myapp_v2.1.zip）
# 锁文件在 uploads/.{项目名}.lock，删除后触发部署
#
# [[deploy.projects]]
# name = "myapp"
# type = "static"                          # static | dynamic | docker
# command = "systemctl reload nginx"
# health_check_url = "https://example.com"
# keep_versions = 5
# www_uid = 1010
# www_gid = 1010

[deploy.defaults]
# base_dir = "/data/autoDeploy"           # 共享工作目录根路径
[deploy.defaults.health_check]
timeout = 30
"#;

/// 项目配置模板 (.opsctl.toml)
const PROJECT_CONFIG_TEMPLATE: &str = r#"# opsctl 项目配置 — 放在项目根目录，覆盖全局配置
#
# DNS 解析管理是全局能力，请在 ~/.opsctl/config.toml 的 [dns] 段配置。
#
# ZIP 命名规范: {项目名}_{版本号}.zip（如 myapp_v2.1.zip）
# 上传到 uploads/ 后，删除 uploads/.{项目名}.lock 触发部署

[deploy]
# 部署类型（必填）
type = "static"               # static | dynamic | docker

# 重启命令（必填）
command = "systemctl reload nginx"

# 健康检查 URL（推荐）
health_check_url = "https://example.com"

# 文件所有权（可选）
# www_uid = 1010
# www_gid = 1010

# 保留历史版本数（默认 5）
keep_versions = 5

# 上传/就绪/启用目录（可选，默认 basedir 下的 uploads/ ready/ enabled/）
# upload_dir = "/data/myapp/uploads"
# ready_dir = "/data/myapp/ready"
# enabled_dir = "/data/myapp/enabled"
"#;

pub fn handle_init(project: bool, force: bool) -> HandlerResult {
    if project {
        init_project_config(force)
    } else {
        init_global_config(force)
    }
}

fn init_global_config(force: bool) -> HandlerResult {
    let home = std::env::var("HOME")
        .map_err(|_| OpsctlError::Other("无法获取 home 目录 (未设置 HOME 环境变量)".into()))?;
    let config_dir = Path::new(&home).join(".opsctl");
    let config_path = config_dir.join("config.toml");

    if config_path.exists() && !force {
        return Err(OpsctlError::Other(format!(
            "配置文件已存在: {} (使用 --force 强制覆盖)",
            config_path.display()
        )));
    }

    std::fs::create_dir_all(&config_dir)
        .map_err(|e| OpsctlError::Other(format!("创建配置目录失败: {e}")))?;
    std::fs::write(&config_path, GLOBAL_CONFIG_TEMPLATE)
        .map_err(|e| OpsctlError::Other(format!("写入配置文件失败: {e}")))?;

    println!("✅ 全局配置已生成: {:?}", config_path);
    println!("   编辑此文件以配置通知、证书源、部署默认值和 DNS 解析管理。");
    Ok(())
}

fn init_project_config(force: bool) -> HandlerResult {
    let config_path = Path::new(".opsctl.toml");

    if config_path.exists() && !force {
        return Err(OpsctlError::Other(
            "配置文件 .opsctl.toml 已存在 (使用 --force 强制覆盖)".into(),
        ));
    }

    std::fs::write(config_path, PROJECT_CONFIG_TEMPLATE)
        .map_err(|e| OpsctlError::Other(format!("写入配置文件失败: {e}")))?;

    let abs = std::env::current_dir()
        .unwrap_or_else(|_| Path::new(".").to_path_buf())
        .join(config_path);

    println!("✅ 项目配置已生成: {:?}", abs);
    println!("   编辑此文件，填写 deploy.type 和 deploy.command 后即可使用 opsctl deploy run。");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_template_has_active_dns_section() {
        assert!(GLOBAL_CONFIG_TEMPLATE.contains("\n[dns]\n"));
        assert!(GLOBAL_CONFIG_TEMPLATE.contains("notify = \"failure\""));
        assert!(GLOBAL_CONFIG_TEMPLATE.contains("[[dns.accounts]]"));
    }

    #[test]
    fn project_template_points_dns_to_global_config() {
        assert!(PROJECT_CONFIG_TEMPLATE.contains("~/.opsctl/config.toml"));
        assert!(PROJECT_CONFIG_TEMPLATE.contains("[dns]"));
    }
}
