use std::path::Path;
use x509_parser::prelude::*;

#[derive(Debug, Clone)]
pub struct CertInfo {
    pub main_domain: String,
    pub all_domains: Vec<String>,
    pub not_before: String,
    pub not_after: String,
    pub not_after_timestamp: u64,
}

/// 从 PEM 格式文本解析证书
pub fn parse_pem_certificate(pem_data: &str) -> Result<CertInfo, crate::CertError> {
    let (_, pem) = parse_x509_pem(pem_data.as_bytes())
        .map_err(|e| crate::CertError::CertParse(format!("PEM 解析失败: {e}")))?;

    let (_, x509) = X509Certificate::from_der(&pem.contents)
        .map_err(|e| crate::CertError::CertParse(format!("DER 解析失败: {e}")))?;

    extract_cert_info(&x509)
}

/// 从 PEM 文件解析证书
pub fn parse_pem_file(path: &Path) -> Result<CertInfo, crate::CertError> {
    let content = std::fs::read_to_string(path)
        .map_err(crate::CertError::Io)?;

    let (_, pem) = parse_x509_pem(content.as_bytes())
        .map_err(|e| crate::CertError::CertParse(format!("PEM 解析失败: {e}")))?;

    let (_, x509) = X509Certificate::from_der(&pem.contents)
        .map_err(|e| crate::CertError::CertParse(format!("DER 解析失败: {e}")))?;

    extract_cert_info(&x509)
}

fn extract_cert_info(x509: &X509Certificate) -> Result<CertInfo, crate::CertError> {
    let subject = x509.subject();
    let cn = subject.iter_common_name()
        .next()
        .map(|attr| attr.as_str().unwrap_or("").to_string())
        .unwrap_or_default();

    let mut domains: Vec<String> = Vec::new();
    if let Ok(Some(san)) = x509.subject_alternative_name() {
        for name in &san.value.general_names {
            if let GeneralName::DNSName(dns) = name {
                domains.push(dns.to_string());
            }
        }
    }

    if !cn.is_empty() && !domains.contains(&cn) {
        domains.push(cn.clone());
    }
    domains.sort();
    domains.dedup();

    // CN 始终作为主域名
    let main_domain = if !cn.is_empty() {
        cn
    } else {
        domains.first().cloned().unwrap_or_else(|| "unknown".to_string())
    };

    let validity = x509.validity();
    let not_before = timestamp_to_string(validity.not_before.timestamp());
    let not_after = timestamp_to_string(validity.not_after.timestamp());
    let not_after_timestamp = validity.not_after.timestamp() as u64;

    Ok(CertInfo {
        main_domain,
        all_domains: domains,
        not_before,
        not_after,
        not_after_timestamp,
    })
}

fn timestamp_to_string(ts: i64) -> String {
    if ts < 0 {
        return "invalid".to_string();
    }
    let days = ts as u64 / 86400;
    let time_secs = ts as u64 % 86400;
    let hours = time_secs / 3600;
    let mins = (time_secs % 3600) / 60;
    let secs = time_secs % 60;
    let (year, month, day) = days_to_date(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hours, mins, secs)
}

fn days_to_date(days: u64) -> (u64, u64, u64) {
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if d < days_in_year { break; }
        d -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) { [31,29,31,30,31,30,31,31,30,31,30,31] }
                               else { [31,28,31,30,31,30,31,31,30,31,30,31] };
    let mut m = 0u64;
    for (i, &md) in month_days.iter().enumerate() {
        if d < md as i64 { m = i as u64; break; }
        d -= md as i64;
    }
    (y as u64, m + 1, (d + 1) as u64)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PEM: &str = include_str!("/tmp/test_cert.pem");

    #[test]
    fn test_parse_valid_pem() {
        let info = parse_pem_certificate(TEST_PEM).unwrap();
        assert_eq!(info.main_domain, "test.example.com");
        assert!(info.all_domains.contains(&"test.example.com".to_string()));
        assert!(info.all_domains.contains(&"api.example.com".to_string()));
        assert!(info.all_domains.contains(&"www.example.com".to_string()));
        assert!(!info.not_before.is_empty());
        assert!(!info.not_after.is_empty());
        assert!(info.not_after_timestamp > 0);
    }

    #[test]
    fn test_parse_invalid_pem() {
        let result = parse_pem_certificate("not a certificate");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_string() {
        let result = parse_pem_certificate("");
        assert!(result.is_err());
    }

    #[test]
    fn test_timestamp_to_string() {
        // 2024-01-15 12:00:00 UTC = 1705316400
        let result = timestamp_to_string(1705316400);
        assert!(result.contains("2024-01-15"));
    }

    #[test]
    fn test_timestamp_negative() {
        let result = timestamp_to_string(-1);
        assert_eq!(result, "invalid");
    }
}
