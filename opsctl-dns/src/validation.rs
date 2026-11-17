use opsctl_core::config::DnsProviderKind;

use crate::error::DnsError;

const ALLOWED_TYPES: [&str; 8] = ["A", "AAAA", "CNAME", "MX", "TXT", "NS", "SRV", "CAA"];

pub fn validate_host(host: &str) -> Result<String, DnsError> {
    let host = host.trim();
    if host.is_empty() {
        return Err(DnsError::Validation(
            "主机记录不能为空，根记录请使用 @".into(),
        ));
    }
    Ok(host.to_string())
}

pub fn validate_rtype(rtype: &str) -> Result<String, DnsError> {
    let rtype = rtype.trim().to_ascii_uppercase();
    if !ALLOWED_TYPES.contains(&rtype.as_str()) {
        return Err(DnsError::Validation(format!(
            "不支持的记录类型 {rtype}，允许: {}",
            ALLOWED_TYPES.join("/")
        )));
    }
    Ok(rtype)
}

pub fn validate_ttl(ttl: u64) -> Result<u64, DnsError> {
    if !(1..=86_400).contains(&ttl) {
        return Err(DnsError::Validation("TTL 需在 1-86400 秒之间".into()));
    }
    Ok(ttl)
}

pub fn validate_value(value: &str) -> Result<String, DnsError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(DnsError::Validation("记录值不能为空".into()));
    }
    Ok(value.to_string())
}

/// CLI 统一使用 `default`；各平台映射为自己的默认线路。
pub fn normalize_line(line: &str, provider: DnsProviderKind) -> String {
    let line = line.trim();
    match provider {
        DnsProviderKind::Cloudflare => String::new(),
        DnsProviderKind::Dnspod => {
            if line.is_empty() || line.eq_ignore_ascii_case("default") || line == "默认" {
                "默认".to_string()
            } else {
                line.to_string()
            }
        }
        DnsProviderKind::Aliyun => {
            if line.is_empty() {
                "default".to_string()
            } else {
                line.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_host() {
        assert!(validate_host("   ").is_err());
    }

    #[test]
    fn accepts_root_host() {
        assert_eq!(validate_host("@").unwrap(), "@");
    }

    #[test]
    fn rejects_unsupported_record_type() {
        assert!(validate_rtype("HTTPS").is_err());
    }

    #[test]
    fn accepts_all_supported_record_types() {
        for t in ["A", "AAAA", "CNAME", "MX", "TXT", "NS", "SRV", "CAA"] {
            assert_eq!(validate_rtype(t).unwrap(), t);
        }
    }

    #[test]
    fn rejects_ttl_out_of_range() {
        assert!(validate_ttl(0).is_err());
        assert!(validate_ttl(86_401).is_err());
    }

    #[test]
    fn accepts_ttl_boundaries() {
        assert_eq!(validate_ttl(1).unwrap(), 1);
        assert_eq!(validate_ttl(86_400).unwrap(), 86_400);
    }

    #[test]
    fn maps_default_line_to_dnspod_china_value() {
        assert_eq!(normalize_line("default", DnsProviderKind::Dnspod), "默认");
    }

    #[test]
    fn ignores_line_for_cloudflare() {
        assert_eq!(normalize_line("anything", DnsProviderKind::Cloudflare), "");
    }
}
