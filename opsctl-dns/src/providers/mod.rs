// DNS 平台适配器工厂与各平台实现。

use opsctl_core::config::{DnsAccount, DnsProviderKind};

use crate::{DnsError, DnsProvider};

#[cfg(test)]
pub(crate) mod test_server;

pub mod aliyun;
pub mod cloudflare;
pub mod dnspod;

pub(crate) fn fqdn_to_host(name: &str, zone: &str) -> String {
    if name.eq_ignore_ascii_case(zone) {
        return "@".to_string();
    }
    let suffix = format!(".{}", zone.to_lowercase());
    name.to_lowercase()
        .strip_suffix(&suffix)
        .unwrap_or(name)
        .to_string()
}

pub(crate) fn host_to_fqdn(host: &str, zone: &str) -> String {
    if host == "@" {
        zone.to_string()
    } else {
        format!("{host}.{zone}")
    }
}

pub fn build_provider(account: &DnsAccount) -> Result<Box<dyn DnsProvider>, DnsError> {
    let required = |field: Option<&str>, name: &str| -> Result<String, DnsError> {
        field
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                DnsError::Config(format!(
                    "账户 {} ({}) 缺少凭据字段 {name}",
                    account.name,
                    account.provider.as_str()
                ))
            })
    };

    match account.provider {
        DnsProviderKind::Aliyun => {
            let id = required(account.access_key_id.as_deref(), "access_key_id")?;
            let secret =
                required(account.access_key_secret.as_deref(), "access_key_secret")?;
            Ok(Box::new(aliyun::AliyunProvider::new(id, secret)))
        }
        DnsProviderKind::Dnspod => {
            let id = required(account.secret_id.as_deref(), "secret_id")?;
            let key = required(account.secret_key.as_deref(), "secret_key")?;
            Ok(Box::new(dnspod::DnspodProvider::new(id, key)))
        }
        DnsProviderKind::Cloudflare => {
            let token = required(account.api_token.as_deref(), "api_token")?;
            Ok(Box::new(cloudflare::CloudflareProvider::new(token)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(provider: DnsProviderKind) -> DnsAccount {
        DnsAccount {
            name: "test".into(),
            provider,
            access_key_id: Some("ak".into()),
            access_key_secret: Some("sk".into()),
            secret_id: Some("id".into()),
            secret_key: Some("key".into()),
            api_token: Some("token".into()),
        }
    }

    #[test]
    fn build_provider_rejects_missing_aliyun_secret() {
        let mut acc = account(DnsProviderKind::Aliyun);
        acc.access_key_secret = None;
        assert!(build_provider(&acc).is_err());
    }

    #[test]
    fn build_provider_rejects_missing_cloudflare_token() {
        let mut acc = account(DnsProviderKind::Cloudflare);
        acc.api_token = None;
        assert!(build_provider(&acc).is_err());
    }

    #[test]
    fn build_provider_accepts_all_three_platforms() {
        for kind in [
            DnsProviderKind::Aliyun,
            DnsProviderKind::Dnspod,
            DnsProviderKind::Cloudflare,
        ] {
            assert!(build_provider(&account(kind)).is_ok(), "{kind:?}");
        }
    }
}
