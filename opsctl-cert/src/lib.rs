pub mod certd;
pub mod certificate;
pub mod error;
pub mod formats;

pub use certd::CertdClient;
pub use certificate::CertInfo;
pub use error::CertError;
pub use formats::CertParts;

use md5::{Digest, Md5};
use std::path::{Path, PathBuf};

/// 执行一次证书更新
pub fn update_certs(
    config: &opsctl_core::config::CertConfig,
) -> Result<Vec<UpdateResult>, CertError> {
    let client = CertdClient::new(
        &config.certd_base_url,
        &config.certd_key_id,
        &config.certd_key_secret,
    );
    let cert_dir = resolve_cert_dir(config);

    let mut results = Vec::new();
    for cert_id in &config.cert_ids {
        tracing::info!("正在下载证书: cert_id={}", cert_id);
        match client.get_cert(*cert_id) {
            Ok(cert_data) => {
                let parts = formats::CertParts {
                    crt: cert_data.cert_pem.clone(),
                    key: cert_data.key_pem.clone(),
                    ic: cert_data.ic_pem.clone(),
                    oc: cert_data.oc_pem.clone(),
                    pfx: cert_data.pfx.clone(),
                    der: cert_data.der.clone(),
                    jks: cert_data.jks.clone(),
                    p7b: cert_data.p7b.clone(),
                };
                let cert_info = certificate::parse_pem_certificate(&cert_data.cert_pem)?;
                let domain = select_primary_domain(&cert_data.detail_domains, &cert_info);
                let safe_domain = sanitize_domain(&domain);

                let domain_dir = cert_dir.join(&safe_domain);

                // MD5 对比：远程证书 vs 本地已有证书，内容相同则跳过
                let new_md5 = hash_md5(cert_data.cert_pem.as_bytes());
                let local_crt = domain_dir.join("fullchain.cer");
                let local_md5 = if local_crt.exists() {
                    let content = std::fs::read_to_string(&local_crt).unwrap_or_default();
                    Some(hash_md5(content.as_bytes()))
                } else {
                    None
                };

                if let Some(ref local) = local_md5
                    && local == &new_md5
                {
                    tracing::info!("证书未变更，跳过: {} (cert_id={})", domain, cert_id);
                    results.push(UpdateResult {
                        domain: domain.clone(),
                        cert_id: Some(*cert_id),
                        success: true,
                        changed: false,
                        message: "证书未变更，已跳过".into(),
                    });
                    continue;
                }

                // 证书有变更，写入文件
                std::fs::create_dir_all(&domain_dir)?;

                let formats = if config.formats.is_empty() {
                    vec!["crt".to_string(), "key".to_string()]
                } else {
                    config.formats.clone()
                };

                for fmt in &formats {
                    formats::write_format(&parts, fmt, &domain_dir, &safe_domain)?;
                }

                // 保存元数据
                let meta = serde_json::json!({
                    "cert_id": cert_id,
                    "domain": domain,
                    "not_after": cert_info.not_after,
                    "updated_at": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                });
                std::fs::write(
                    domain_dir.join(".opsctl_cert_meta.json"),
                    serde_json::to_string_pretty(&meta).unwrap_or_default(),
                )
                .unwrap_or_else(|e| tracing::warn!("保存元数据失败: {e}"));

                tracing::info!(
                    "证书已更新: {} (cert_id={}, md5={})",
                    domain,
                    cert_id,
                    &new_md5[..8]
                );
                results.push(UpdateResult {
                    domain: domain.clone(),
                    cert_id: Some(*cert_id),
                    success: true,
                    changed: true,
                    message: format!("过期时间: {}", cert_info.not_after),
                });
            }
            Err(e) => {
                tracing::error!("证书下载失败 (cert_id={}): {}", cert_id, e);
                results.push(UpdateResult {
                    domain: format!("cert_id={}", cert_id),
                    cert_id: Some(*cert_id),
                    success: false,
                    changed: false,
                    message: e.to_string(),
                });
            }
        }
    }

    Ok(results)
}

/// 扫描本地证书目录，返回所有证书信息
pub fn list_certs(config: &opsctl_core::config::CertConfig) -> Result<Vec<CertEntry>, CertError> {
    let cert_dir = resolve_cert_dir(config);
    if !cert_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&cert_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let cert_file = path.join("fullchain.cer");
        if !cert_file.exists() {
            continue;
        }

        match certificate::parse_pem_file(&cert_file) {
            Ok(info) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let expiry = info.not_after_timestamp;
                let remaining_days = if expiry > now {
                    Some((expiry - now) / 86400)
                } else {
                    None
                };
                // 读取元数据获取 cert_id
                let cert_id = read_cert_meta_id(&path);
                entries.push(CertEntry {
                    cert_id,
                    domain: info.main_domain.clone(),
                    not_after: info.not_after.clone(),
                    remaining_days,
                });
            }
            Err(e) => {
                tracing::warn!("证书解析失败 {:?}: {}", cert_file, e);
            }
        }
    }

    Ok(entries)
}

/// 检查证书过期状态
pub fn check_certs(config: &opsctl_core::config::CertConfig) -> Result<CheckResult, CertError> {
    let entries = list_certs(config)?;
    let warn_days = config.check.warn_days as u64;

    let mut expiring = Vec::new();
    let mut expired = Vec::new();

    for entry in &entries {
        match entry.remaining_days {
            Some(0) => {
                expired.push(entry.clone());
            }
            Some(days) if days <= warn_days => {
                expiring.push(entry.clone());
            }
            None => {
                expired.push(entry.clone());
            }
            _ => {}
        }
    }

    Ok(CheckResult { expiring, expired })
}

fn resolve_cert_dir(config: &opsctl_core::config::CertConfig) -> PathBuf {
    if let Some(ref dir) = config.cert_dir {
        PathBuf::from(dir)
    } else {
        let home = dirs::home_dir().unwrap_or_else(|| Path::new("/root").to_path_buf());
        home.join(".opsctl/certificates")
    }
}

fn select_primary_domain(detail_domains: &[String], info: &CertInfo) -> String {
    detail_domains
        .iter()
        .find(|domain| !domain.starts_with("*."))
        .or_else(|| detail_domains.first())
        .cloned()
        .or_else(|| {
            info.all_domains
                .iter()
                .find(|domain| !domain.starts_with("*."))
                .cloned()
        })
        .unwrap_or_else(|| info.main_domain.clone())
}

fn sanitize_domain(domain: &str) -> String {
    let mut out = String::with_capacity(domain.len());
    for ch in domain.chars() {
        if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    let cleaned = out.trim().trim_matches(['.', ' ']).to_string();
    if cleaned.is_empty() {
        "certificate".to_string()
    } else {
        cleaned
    }
}

fn hash_md5(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn read_cert_meta_id(domain_dir: &Path) -> Option<u32> {
    let meta_file = domain_dir.join(".opsctl_cert_meta.json");
    let content = std::fs::read_to_string(&meta_file).ok()?;
    let meta: serde_json::Value = serde_json::from_str(&content).ok()?;
    meta.get("cert_id")?.as_u64().map(|v| v as u32)
}

// ─── 公共类型 ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub domain: String,
    pub cert_id: Option<u32>,
    pub success: bool,
    pub changed: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct CertEntry {
    pub cert_id: Option<u32>,
    pub domain: String,
    pub not_after: String,
    pub remaining_days: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub expiring: Vec<CertEntry>,
    pub expired: Vec<CertEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_domain_prefers_non_wildcard_from_detail() {
        let info = CertInfo {
            main_domain: "fallback.example.com".into(),
            all_domains: vec!["fallback.example.com".into()],
            not_before: String::new(),
            not_after: String::new(),
            not_after_timestamp: 0,
        };
        let detail = vec![
            "*.xx.example.com".into(),
            "xx.example.com".into(),
            "other.example.com".into(),
        ];
        assert_eq!(select_primary_domain(&detail, &info), "xx.example.com");
    }

    #[test]
    fn primary_domain_falls_back_to_certificate_info() {
        let info = CertInfo {
            main_domain: "test.example.com".into(),
            all_domains: vec!["test.example.com".into()],
            not_before: String::new(),
            not_after: String::new(),
            not_after_timestamp: 0,
        };
        assert_eq!(select_primary_domain(&[], &info), "test.example.com");
    }

    #[test]
    fn sanitize_domain_replaces_unsafe_characters() {
        assert_eq!(sanitize_domain("*.example.com"), "_.example.com");
        assert_eq!(sanitize_domain("a/b:c"), "a_b_c");
    }
}
