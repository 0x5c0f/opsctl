use std::path::Path;
use sha2::{Sha256, Digest};

/// 证书各部分数据
pub struct CertParts {
    pub crt: String,   // 完整证书链 (fullchain)
    pub key: String,   // 私钥
    pub ic: Option<String>,  // 中间证书
    pub oc: Option<String>,  // 服务器证书
}

/// 证书格式输出
///
/// 支持的格式:
/// - crt: 完整证书链 → fullchain.cer（来自 crt 字段）
/// - key: 私钥 → <domain>.key（来自 key 字段）
/// - ic: 中间证书 → <domain>.ic（来自 ic 字段，与 Python 一致）
/// - oc: 服务器证书 → <domain>.cer（来自 oc 字段，与 Python 一致）
/// - pfx: PKCS#12 → pfx.txt（转换说明）
/// - der: DER 格式 → <domain>.der
/// - jks: Java KeyStore → jks.txt（转换说明）
/// - one: 一站式证书 → one.txt（crt + key 合并）
pub fn write_format(parts: &CertParts, format: &str, domain_dir: &Path, domain: &str) -> Result<(), crate::CertError> {
    match format {
        "crt" => write_file(domain_dir, "fullchain.cer", parts.crt.as_bytes()),
        "key" => write_file(domain_dir, &format!("{}.key", domain), parts.key.as_bytes()),
        "ic" => {
            if let Some(ref data) = parts.ic {
                write_file(domain_dir, &format!("{}.ic", domain), data.as_bytes());
            } else {
                tracing::warn!("未收到中间证书 (ic)，跳过");
            }
        }
        "oc" => {
            if let Some(ref data) = parts.oc {
                write_file(domain_dir, &format!("{}.cer", domain), data.as_bytes());
            } else {
                tracing::warn!("未收到服务器证书 (oc)，跳过");
            }
        }
        "one" => {
            // 一站式: cert + key 拼接
            let combined = format!("{}\n{}", parts.crt, parts.key);
            write_file(domain_dir, &format!("{}.one", domain), combined.as_bytes());
        }
        "pfx" => {
            let note = format!("PFX format requires openssl conversion:\n  openssl pkcs12 -export -out {}.pfx -inkey {}.key -in fullchain.cer\n", domain, domain);
            write_file(domain_dir, "pfx.txt", note.as_bytes())
        }
        "der" => {
            write_file(domain_dir, &format!("{}.der", domain), parts.crt.as_bytes())
        }
        "jks" => {
            let note = format!("JKS format requires keytool conversion:\n  keytool -importkeystore -srckeystore {}.pfx -srcstoretype PKCS12 -destkeystore {}.jks -deststoretype JKS\n", domain, domain);
            write_file(domain_dir, "jks.txt", note.as_bytes())
        }
        other => {
            tracing::warn!("未知证书格式: {}", other);
        }
    }
    Ok(())
}

/// 计算文件 SHA256 指纹
pub fn file_sha256(path: &Path) -> Result<String, crate::CertError> {
    let content = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_file(dir: &Path, name: &str, data: &[u8]) {
    let path = dir.join(name);
    match std::fs::write(&path, data) {
        Ok(_) => tracing::debug!("已写入: {:?}", path),
        Err(e) => tracing::error!("写入失败 {:?}: {}", path, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn test_dir() -> std::path::PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("opsctl_fmt_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn test_parts() -> CertParts {
        CertParts {
            crt: "TEST_CERT".into(),
            key: "TEST_KEY".into(),
            ic: Some("TEST_IC".into()),
            oc: Some("TEST_OC".into()),
        }
    }

    #[test]
    fn test_write_known_formats() {
        let dp = test_dir();
        let parts = test_parts();
        for fmt in &["crt", "key", "ic", "oc", "der"] {
            write_format(&parts, fmt, &dp, "example.com").unwrap();
            let expected = match *fmt {
                "crt" => dp.join("fullchain.cer"),
                "key" => dp.join("example.com.key"),
                "ic" => dp.join("example.com.ic"),
                "oc" => dp.join("example.com.cer"),
                "der" => dp.join("example.com.der"),
                _ => unreachable!(),
            };
            assert!(expected.exists(), "missing file for format: {fmt}");
        }
    }

    #[test]
    fn test_crt_key_different() {
        let dp = test_dir();
        let parts = test_parts();
        write_format(&parts, "crt", &dp, "ex.com").unwrap();
        write_format(&parts, "key", &dp, "ex.com").unwrap();
        let crt = std::fs::read_to_string(dp.join("fullchain.cer")).unwrap();
        let key = std::fs::read_to_string(dp.join("ex.com.key")).unwrap();
        assert_eq!(crt, "TEST_CERT");
        assert_eq!(key, "TEST_KEY");
        assert_ne!(crt, key, "crt and key files must have different content");
    }

    #[test]
    fn test_one_format_combined() {
        let dp = test_dir();
        let parts = test_parts();
        write_format(&parts, "one", &dp, "ex.com").unwrap();
        let content = std::fs::read_to_string(dp.join("ex.com.one")).unwrap();
        assert!(content.contains("TEST_CERT"));
        assert!(content.contains("TEST_KEY"));
    }

    #[test]
    fn test_write_info_format_pfx() {
        let dp = test_dir();
        let parts = test_parts();
        write_format(&parts, "pfx", &dp, "example.com").unwrap();
        let p = dp.join("pfx.txt");
        assert!(p.exists());
        let content = std::fs::read_to_string(&p).unwrap();
        assert!(content.contains("openssl pkcs12"));
    }

    #[test]
    fn test_unknown_format_no_error() {
        let dp = test_dir();
        let parts = test_parts();
        write_format(&parts, "unknown_fmt", &dp, "example.com").unwrap();
    }

    #[test]
    fn test_file_sha256() {
        let dp = test_dir();
        let file = dp.join("test.bin");
        std::fs::write(&file, b"hello world").unwrap();
        let hash = file_sha256(&file).unwrap();
        assert_eq!(hash.len(), 64);
        assert_eq!(hash, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    }
}
