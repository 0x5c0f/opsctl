use std::path::Path;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use sha2::{Sha256, Digest};

/// 证书各部分数据
pub struct CertParts {
    pub crt: String,        // 完整证书链 (fullchain)
    pub key: String,        // 私钥
    pub ic: Option<String>, // 中间证书
    pub oc: Option<String>, // 服务器证书
    pub pfx: Option<String>, // PFX 格式，Base64 编码
    pub der: Option<String>, // DER 格式，Base64 编码
    pub jks: Option<String>, // JKS 格式，Base64 编码
    pub p7b: Option<String>, // P7B 格式，Base64 编码
}

/// 证书格式输出
///
/// 支持格式:
/// - crt: 完整证书链 → fullchain.cer
/// - key: 私钥 → <domain>.key
/// - ic: 中间证书 → <domain>.ic
/// - oc: 服务器证书 → <domain>.cer
/// - one: 一站式证书 → <domain>.one（crt + key）
/// - pfx/der/jks: API 返回的 Base64 内容，解码后写入对应二进制文件
/// - p7b: API 返回的文本/PEM 内容，直接写入 <domain>.p7b
pub fn write_format(
    parts: &CertParts,
    format: &str,
    domain_dir: &Path,
    domain: &str,
) -> Result<(), crate::CertError> {
    match format {
        "crt" => write_file(domain_dir, "fullchain.cer", parts.crt.as_bytes()),
        "key" => write_file(domain_dir, &format!("{domain}.key"), parts.key.as_bytes()),
        "ic" => match parts.ic.as_deref() {
            Some(data) => write_file(domain_dir, &format!("{domain}.ic"), data.as_bytes()),
            None => {
                tracing::warn!("未收到中间证书 (ic)，跳过");
                Ok(())
            }
        },
        "oc" => match parts.oc.as_deref() {
            Some(data) => write_file(domain_dir, &format!("{domain}.cer"), data.as_bytes()),
            None => {
                tracing::warn!("未收到服务器证书 (oc)，跳过");
                Ok(())
            }
        },
        "one" => {
            let combined = format!("{}\n{}", parts.crt, parts.key);
            write_file(domain_dir, &format!("{domain}.one"), combined.as_bytes())
        }
        "pfx" => write_base64(domain_dir, &format!("{domain}.pfx"), parts.pfx.as_deref()),
        "der" => write_base64(domain_dir, &format!("{domain}.der"), parts.der.as_deref()),
        "jks" => write_base64(domain_dir, &format!("{domain}.jks"), parts.jks.as_deref()),
        "p7b" => match parts.p7b.as_deref() {
            Some(data) => write_file(domain_dir, &format!("{domain}.p7b"), data.as_bytes()),
            None => {
                tracing::warn!("API 未返回 p7b，跳过");
                Ok(())
            }
        },
        other => {
            tracing::warn!("未知证书格式: {other}");
            Ok(())
        }
    }
}

/// 计算文件 SHA256 指纹
pub fn file_sha256(path: &Path) -> Result<String, crate::CertError> {
    let content = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_base64(domain_dir: &Path, name: &str, data: Option<&str>) -> Result<(), crate::CertError> {
    let Some(encoded) = data else {
        tracing::warn!("API 未返回 {name}，跳过");
        return Ok(());
    };
    let decoded = BASE64.decode(encoded).map_err(|e| {
        crate::CertError::Config(format!("{name} 不是合法 Base64 数据: {e}"))
    })?;
    write_file(domain_dir, name, &decoded)
}

fn write_file(dir: &Path, name: &str, data: &[u8]) -> Result<(), crate::CertError> {
    let path = dir.join(name);
    std::fs::write(&path, data)?;
    tracing::debug!("已写入: {:?}", path);
    Ok(())
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
            pfx: Some(base64::engine::general_purpose::STANDARD.encode(b"PFXDATA")),
            der: Some(base64::engine::general_purpose::STANDARD.encode(b"DERDATA")),
            jks: Some(base64::engine::general_purpose::STANDARD.encode(b"JKSDATA")),
            p7b: Some("P7BDATA".into()),
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
    fn test_write_error_propagates() {
        let dp = test_dir();
        let parts = test_parts();
        std::fs::create_dir_all(dp.join("ex.com.key")).unwrap();
        assert!(write_format(&parts, "key", &dp, "ex.com").is_err());
    }

    #[test]
    fn test_unknown_format_no_error() {
        let dp = test_dir();
        let parts = test_parts();
        write_format(&parts, "unknown_fmt", &dp, "example.com").unwrap();
    }

    #[test]
    fn test_base64_formats_write_decoded_binary() {
        use base64::Engine as _;

        let dp = test_dir();
        let parts = test_parts();
        for fmt in &["pfx", "der", "jks", "p7b"] {
            write_format(&parts, fmt, &dp, "example.com").unwrap();
        }
        assert_eq!(
            std::fs::read(dp.join("example.com.pfx")).unwrap(),
            b"PFXDATA"
        );
        assert_eq!(
            std::fs::read(dp.join("example.com.der")).unwrap(),
            b"DERDATA"
        );
        assert_eq!(
            std::fs::read(dp.join("example.com.jks")).unwrap(),
            b"JKSDATA"
        );
        assert_eq!(
            std::fs::read(dp.join("example.com.p7b")).unwrap(),
            b"P7BDATA"
        );
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
