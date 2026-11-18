use serde::Deserialize;
use std::path::PathBuf;

/// opsctl 全局配置（~/.opsctl/config.toml）
#[derive(Debug, Clone, Deserialize, Default)]
pub struct GlobalConfig {
    /// 日志级别 (error/warn/info/debug/trace)
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub notify: NotifyConfig,
    #[serde(default)]
    pub cert: CertConfig,
    #[serde(default)]
    pub deploy: DeployConfigRoot,
    #[serde(default)]
    pub dns: DnsConfig,
}

/// DNS 通知级别。
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
pub enum DnsNotifyLevel {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "failure")]
    #[default]
    Failure,
    #[serde(rename = "all")]
    All,
}

/// DNS 平台类型（第一版仅三个）。
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub enum DnsProviderKind {
    #[serde(rename = "aliyun")]
    Aliyun,
    #[serde(rename = "dnspod")]
    Dnspod,
    #[serde(rename = "cloudflare")]
    Cloudflare,
}

impl DnsProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DnsProviderKind::Aliyun => "aliyun",
            DnsProviderKind::Dnspod => "dnspod",
            DnsProviderKind::Cloudflare => "cloudflare",
        }
    }
}

/// 一个 DNS 平台账户；凭据字段按 provider 必填，由 opsctl-dns 校验。
#[derive(Debug, Clone, Deserialize)]
pub struct DnsAccount {
    pub name: String,
    pub provider: DnsProviderKind,
    pub access_key_id: Option<String>,
    pub access_key_secret: Option<String>,
    pub secret_id: Option<String>,
    pub secret_key: Option<String>,
    pub api_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DnsConfig {
    #[serde(default)]
    pub notify: DnsNotifyLevel,
    #[serde(default)]
    pub accounts: Vec<DnsAccount>,
}

/// 通知模块配置
#[derive(Debug, Clone, Deserialize, Default)]
pub struct NotifyConfig {
    #[serde(default)]
    pub webhooks: Vec<WebhookEntry>,
}

/// 单个 webhook 配置项
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookEntry {
    pub r#type: String,
    pub url: String,
}

/// 证书模块配置
#[derive(Debug, Clone, Deserialize)]
pub struct CertConfig {
    #[serde(default)]
    pub certd_base_url: String,
    #[serde(default)]
    pub certd_key_id: String,
    #[serde(default)]
    pub certd_key_secret: String,
    #[serde(default)]
    pub cert_ids: Vec<u32>,
    #[serde(default)]
    pub cert_dir: Option<String>,
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub domain_reload_commands: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub check: CertCheckConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CertCheckConfig {
    #[serde(default = "default_warn_days")]
    pub warn_days: u32,
    #[serde(default = "default_true")]
    pub expired_check: bool,
    #[serde(default = "default_true")]
    pub notification: bool,
}

/// 部署模块默认配置（全局配置中的默认值）
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DeployDefaultsConfig {
    #[serde(default)]
    pub base_dir: Option<String>,
    #[serde(default)]
    pub health_check: HealthCheckDefaults,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HealthCheckDefaults {
    #[serde(default = "default_health_timeout")]
    pub timeout: u64,
}

/// 部署模块集中式配置（全局配置顶层 [deploy] 段）
#[derive(Debug, Clone, Deserialize, Default)]
pub struct DeployConfigRoot {
    #[serde(default)]
    pub projects: Vec<DeployProject>,
    #[serde(default)]
    pub defaults: DeployDefaultsConfig,
}

/// 全局配置中的单项目定义
#[derive(Debug, Clone, Deserialize)]
pub struct DeployProject {
    /// 项目名，用于匹配 ZIP 文件名前缀
    pub name: String,
    /// 部署类型
    #[serde(rename = "type")]
    pub deploy_type: DeployType,
    /// 重启命令
    pub command: String,
    #[serde(default)]
    pub health_check_url: Option<String>,
    #[serde(default)]
    pub www_uid: Option<u32>,
    #[serde(default)]
    pub www_gid: Option<u32>,
    #[serde(default = "default_keep_versions")]
    pub keep_versions: usize,
    #[serde(default)]
    pub upload_dir: Option<String>,
    #[serde(default)]
    pub ready_dir: Option<String>,
    #[serde(default)]
    pub enabled_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeployConfig {
    pub r#type: DeployType,
    pub command: String,
    #[serde(default)]
    pub health_check_url: Option<String>,
    #[serde(default)]
    pub www_uid: Option<u32>,
    #[serde(default)]
    pub www_gid: Option<u32>,
    #[serde(default = "default_keep_versions")]
    pub keep_versions: usize,
    #[serde(default)]
    pub upload_dir: Option<String>,
    #[serde(default)]
    pub ready_dir: Option<String>,
    #[serde(default)]
    pub enabled_dir: Option<String>,
}

/// 项目级部署配置（.opsctl.toml）— 向后兼容
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectDeployConfig {
    pub deploy: DeployConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub enum DeployType {
    #[serde(rename = "static")]
    Static,
    #[serde(rename = "dynamic")]
    Dynamic,
    #[serde(rename = "docker")]
    Docker,
}

impl std::fmt::Display for DeployType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployType::Static => write!(f, "static"),
            DeployType::Dynamic => write!(f, "dynamic"),
            DeployType::Docker => write!(f, "docker"),
        }
    }
}

// ─── default values ─────────────────────────────────────────

fn default_health_timeout() -> u64 {
    30
}

fn default_keep_versions() -> usize {
    5
}

fn default_warn_days() -> u32 {
    30
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".into()
}

impl Default for CertConfig {
    fn default() -> Self {
        Self {
            certd_base_url: String::new(),
            certd_key_id: String::new(),
            certd_key_secret: String::new(),
            cert_ids: Vec::new(),
            cert_dir: None,
            formats: vec!["crt".into(), "key".into()],
            domain_reload_commands: std::collections::HashMap::new(),
            check: CertCheckConfig::default(),
        }
    }
}

impl Default for CertCheckConfig {
    fn default() -> Self {
        Self {
            warn_days: default_warn_days(),
            expired_check: true,
            notification: true,
        }
    }
}

impl Default for HealthCheckDefaults {
    fn default() -> Self {
        Self {
            timeout: default_health_timeout(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deploy_type_display() {
        assert_eq!(DeployType::Static.to_string(), "static");
        assert_eq!(DeployType::Dynamic.to_string(), "dynamic");
        assert_eq!(DeployType::Docker.to_string(), "docker");
    }

    #[test]
    fn test_default_values() {
        let config = GlobalConfig::default();
        assert!(config.notify.webhooks.is_empty());
        assert_eq!(config.cert.formats, vec!["crt", "key"]);
        assert_eq!(config.deploy.defaults.health_check.timeout, 30);
    }

    #[test]
    fn test_config_from_toml() {
        let toml_str = r#"
            [cert]
            certd_base_url = "http://example.com"
            certd_key_id = "key1"
            cert_ids = [1, 2]

            [notify]
            webhooks = [
                { type = "wework", url = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=xxx" }
            ]
        "#;
        let config: GlobalConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.cert.certd_base_url, "http://example.com");
        assert_eq!(config.cert.cert_ids, vec![1, 2]);
        assert_eq!(config.notify.webhooks.len(), 1);
        assert_eq!(config.notify.webhooks[0].r#type, "wework");
    }

    #[test]
    fn test_project_deploy_config() {
        let toml_str = r#"
            [deploy]
            type = "static"
            command = "systemctl restart nginx"
            health_check_url = "https://example.com/health"
            keep_versions = 10
        "#;
        let config: ProjectDeployConfig = toml::from_str(toml_str).unwrap();
        assert!(matches!(config.deploy.r#type, DeployType::Static));
        assert_eq!(config.deploy.command, "systemctl restart nginx");
        assert_eq!(config.deploy.keep_versions, 10);
        assert_eq!(
            config.deploy.health_check_url,
            Some("https://example.com/health".into())
        );
    }

    #[test]
    fn test_project_config_missing_fields_default() {
        // keep_versions 等字段应有默认值
        let toml_str = r#"
            [deploy]
            type = "static"
            command = "echo hello"
        "#;
        let config: ProjectDeployConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.deploy.keep_versions, 5); // 默认值
        assert!(config.deploy.health_check_url.is_none());
    }

    #[test]
    fn test_deploy_type_deserialize() {
        let cfg: ProjectDeployConfig = toml::from_str(
            r#"
            [deploy]
            type = "static"
            command = "test"
        "#,
        )
        .unwrap();
        assert!(matches!(cfg.deploy.r#type, DeployType::Static));
    }

    #[test]
    fn test_dns_config_from_toml() {
        let config: GlobalConfig = toml::from_str(
            r#"
            [dns]
            notify = "all"

            [[dns.accounts]]
            name = "aliyun-prod"
            provider = "aliyun"
            access_key_id = "ak"
            access_key_secret = "sk"

            [[dns.accounts]]
            name = "cf-prod"
            provider = "cloudflare"
            api_token = "token"
        "#,
        )
        .unwrap();
        assert_eq!(config.dns.notify, DnsNotifyLevel::All);
        assert_eq!(config.dns.accounts.len(), 2);
        assert_eq!(config.dns.accounts[0].provider, DnsProviderKind::Aliyun);
        assert_eq!(config.dns.accounts[0].access_key_id.as_deref(), Some("ak"));
        assert_eq!(config.dns.accounts[1].provider, DnsProviderKind::Cloudflare);
        assert_eq!(config.dns.accounts[1].api_token.as_deref(), Some("token"));
    }

    #[test]
    fn test_dns_notify_defaults_to_failure() {
        let config: GlobalConfig = toml::from_str("[dns]\n").unwrap();
        assert_eq!(config.dns.notify, DnsNotifyLevel::Failure);
        assert!(config.dns.accounts.is_empty());
    }
}

impl GlobalConfig {
    /// 从默认路径加载全局配置
    pub fn load() -> Result<Self, crate::OpsctlError> {
        let path = Self::default_path()?;
        if path.exists() {
            Self::from_file(&path)
        } else {
            tracing::info!("全局配置文件不存在，使用默认配置: {:?}", path);
            Ok(GlobalConfig::default())
        }
    }

    /// 从指定路径加载全局配置
    pub fn from_file(path: &std::path::Path) -> Result<Self, crate::OpsctlError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::OpsctlError::Config(format!("读取配置文件失败 {}: {e}", path.display()))
        })?;
        let config: GlobalConfig = toml::from_str(&content).map_err(|e| {
            crate::OpsctlError::Config(format!("解析配置文件失败 {}: {e}", path.display()))
        })?;
        Ok(config)
    }

    /// 全局配置默认路径
    pub fn default_path() -> Result<PathBuf, crate::OpsctlError> {
        let base = dirs::home_dir()
            .ok_or_else(|| crate::OpsctlError::Config("无法获取 home 目录".into()))?;
        Ok(base.join(".opsctl").join("config.toml"))
    }

    /// 加载项目级部署配置
    pub fn load_project_deploy_config(
        project_dir: &std::path::Path,
    ) -> Result<Option<ProjectDeployConfig>, crate::OpsctlError> {
        let path = project_dir.join(".opsctl.toml");
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            crate::OpsctlError::Config(format!("读取项目配置失败 {}: {e}", path.display()))
        })?;
        let config: ProjectDeployConfig = toml::from_str(&content).map_err(|e| {
            crate::OpsctlError::Config(format!("解析项目配置失败 {}: {e}", path.display()))
        })?;
        Ok(Some(config))
    }
}
