use opsctl_core::config::{DnsAccount, DnsConfig, DnsProviderKind};

use crate::{
    DnsError, ProviderRecord, ProviderZone, RecordInput, RecordUpdate, providers::build_provider,
};

/// 根据 `--account` 选择账户；未指定时返回全部已配置账户。
pub fn selected_accounts<'a>(
    config: &'a DnsConfig,
    name: Option<&str>,
) -> Result<Vec<&'a DnsAccount>, DnsError> {
    if config.accounts.is_empty() {
        return Err(DnsError::Config(
            "未配置 DNS 账户，请在 ~/.opsctl/config.toml 中增加 [[dns.accounts]]".into(),
        ));
    }
    if let Some(name) = name {
        let account = config
            .accounts
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| DnsError::Config(format!("未找到 DNS 账户: {name}")))?;
        return Ok(vec![account]);
    }
    Ok(config.accounts.iter().collect())
}

pub fn verify(account: &DnsAccount) -> Result<(), DnsError> {
    build_provider(account)?.verify()
}

pub fn list_domains(account: &DnsAccount) -> Result<Vec<crate::ProviderDomain>, DnsError> {
    build_provider(account)?.list_domains()
}

pub fn find_zone(account: &DnsAccount, domain: &str) -> Result<ProviderZone, DnsError> {
    let provider = build_provider(account)?;
    provider
        .list_domains()?
        .into_iter()
        .find(|d| d.name.eq_ignore_ascii_case(domain))
        .map(|d| ProviderZone {
            name: d.name,
            provider_domain_id: d.provider_domain_id,
        })
        .ok_or_else(|| DnsError::NotFound(format!("账户 {} 下未找到域名 {domain}", account.name)))
}

pub fn list_records(
    account: &DnsAccount,
    zone: &ProviderZone,
) -> Result<Vec<ProviderRecord>, DnsError> {
    build_provider(account)?.list_records(zone)
}

pub fn create_record(
    account: &DnsAccount,
    zone: &ProviderZone,
    input: &RecordInput,
) -> Result<ProviderRecord, DnsError> {
    build_provider(account)?.create_record(zone, input)
}

pub fn update_record(
    account: &DnsAccount,
    zone: &ProviderZone,
    record: &ProviderRecord,
    update: &RecordUpdate,
) -> Result<ProviderRecord, DnsError> {
    build_provider(account)?.update_record(zone, record, update)
}

pub fn delete_record(
    account: &DnsAccount,
    zone: &ProviderZone,
    record_id: &str,
) -> Result<(), DnsError> {
    build_provider(account)?.delete_record(zone, record_id)
}

pub fn set_record_status(
    account: &DnsAccount,
    zone: &ProviderZone,
    record_id: &str,
    enabled: bool,
) -> Result<(), DnsError> {
    build_provider(account)?.set_record_status(zone, record_id, enabled)
}

#[allow(clippy::too_many_arguments)]
pub fn record_input(
    account: &DnsAccount,
    host: &str,
    rtype: &str,
    value: &str,
    ttl: u64,
    line: Option<&str>,
    proxied: Option<bool>,
    remark: Option<&str>,
) -> Result<RecordInput, DnsError> {
    let host = crate::validation::validate_host(host)?;
    let rtype = crate::validation::validate_rtype(rtype)?;
    let value = crate::validation::validate_value(value)?;
    let ttl = crate::validation::validate_ttl(ttl)?;
    let line = line
        .map(|l| crate::validation::normalize_line(l, account.provider))
        .unwrap_or_else(|| crate::validation::normalize_line("default", account.provider));
    Ok(RecordInput {
        host,
        rtype,
        value,
        ttl,
        line: Some(line),
        proxied,
        remark: remark.map(str::to_string),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn record_update(
    account: &DnsAccount,
    host: Option<&str>,
    rtype: Option<&str>,
    value: Option<&str>,
    ttl: Option<u64>,
    line: Option<&str>,
    proxied: Option<bool>,
    remark: Option<&str>,
) -> Result<RecordUpdate, DnsError> {
    if host.is_none()
        && rtype.is_none()
        && value.is_none()
        && ttl.is_none()
        && line.is_none()
        && proxied.is_none()
        && remark.is_none()
    {
        return Err(DnsError::Validation("至少提供一个要修改的字段".into()));
    }
    Ok(RecordUpdate {
        host: host.map(crate::validation::validate_host).transpose()?,
        rtype: rtype.map(crate::validation::validate_rtype).transpose()?,
        value: value.map(crate::validation::validate_value).transpose()?,
        ttl: ttl.map(crate::validation::validate_ttl).transpose()?,
        line: line.map(|l| crate::validation::normalize_line(l, account.provider)),
        proxied,
        remark: remark.map(str::to_string),
    })
}

pub fn provider_name(provider: DnsProviderKind) -> &'static str {
    provider.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opsctl_core::config::{DnsAccount, DnsProviderKind};

    fn config() -> DnsConfig {
        DnsConfig {
            notify: Default::default(),
            accounts: vec![
                DnsAccount {
                    name: "aliyun-prod".into(),
                    provider: DnsProviderKind::Aliyun,
                    access_key_id: Some("ak".into()),
                    access_key_secret: Some("sk".into()),
                    secret_id: None,
                    secret_key: None,
                    api_token: None,
                },
                DnsAccount {
                    name: "cf-prod".into(),
                    provider: DnsProviderKind::Cloudflare,
                    access_key_id: None,
                    access_key_secret: None,
                    secret_id: None,
                    secret_key: None,
                    api_token: Some("token".into()),
                },
            ],
        }
    }

    #[test]
    fn selected_accounts_returns_all_when_no_name_given() {
        let cfg = config();
        let selected = selected_accounts(&cfg, None).unwrap();
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].name, "aliyun-prod");
        assert_eq!(selected[1].name, "cf-prod");
    }

    #[test]
    fn selected_accounts_returns_named_account() {
        let cfg = config();
        let selected = selected_accounts(&cfg, Some("cf-prod")).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].provider, DnsProviderKind::Cloudflare);
    }

    #[test]
    fn selected_accounts_rejects_unknown_name() {
        let cfg = config();
        assert!(selected_accounts(&cfg, Some("missing")).is_err());
    }

    #[test]
    fn selected_accounts_rejects_empty_config() {
        let cfg = DnsConfig::default();
        assert!(selected_accounts(&cfg, None).is_err());
    }

    #[test]
    fn record_input_rejects_empty_host() {
        let cfg = config();
        let acc = &cfg.accounts[0];
        assert!(record_input(acc, " ", "A", "1.2.3.4", 600, None, None, None).is_err());
    }

    #[test]
    fn record_input_normalizes_dnspod_default_line() {
        let cfg = config();
        let acc = &cfg.accounts[0];
        let input = record_input(acc, "www", "A", "1.2.3.4", 600, None, None, None).unwrap();
        assert_eq!(input.line.as_deref(), Some("default"));
    }

    #[test]
    fn record_update_requires_at_least_one_field() {
        let cfg = config();
        let acc = &cfg.accounts[0];
        assert!(record_update(acc, None, None, None, None, None, None, None).is_err());
    }
}
