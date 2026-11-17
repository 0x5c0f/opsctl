use md5::{Md5, Digest};
use std::time::{SystemTime, UNIX_EPOCH};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

/// certd API 客户端
///
/// 认证协议：
/// 1. 构造 JSON: {"keyId":"...","t":<unix_ts>,"encrypt":false,"signType":"md5"}
/// 2. 签名 = MD5(json_str + keySecret)
/// 3. Token = base64(json_str) + "." + base64(sign)
/// 4. 请求头: x-certd-token: <token>
pub struct CertdClient {
    base_url: String,
    key_id: String,
    key_secret: String,
}

impl CertdClient {
    pub fn new(base_url: &str, key_id: &str, key_secret: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            key_id: key_id.to_string(),
            key_secret: key_secret.to_string(),
        }
    }

    /// 从 certd API 获取证书数据
    pub fn get_cert(&self, cert_id: u32) -> Result<CertResponseData, crate::CertError> {
        let token = self.generate_token()?;

        let body = serde_json::json!({
            "certId": cert_id,
        });

        let url = format!("{}/api/v1/cert/get", self.base_url.trim_end_matches('/'));
        tracing::debug!("certd API 请求: POST {} (certId={})", url, cert_id);

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .build()
            .into();

        let response = agent.post(&url)
            .header("x-certd-token", &token)
            .send_json(&body)
            .map_err(|e| crate::CertError::Api(format!("API 请求失败: {e}")))?;

        let status = response.status().as_u16();
        // 读取响应体（无论状态码，都需要解析错误消息）
        let mut resp_body = response.into_body();
        let resp_bytes = resp_body.read_to_vec()
            .map_err(|e| crate::CertError::Api(format!("读取响应失败: {e}")))?;
        let resp_text = String::from_utf8(resp_bytes)
            .map_err(|e| crate::CertError::Api(format!("响应编码错误: {e}")))?;

        if status != 200 {
            return Err(crate::CertError::Api(format!("API 返回 HTTP {}: {}", status, &resp_text[..resp_text.len().min(300)])));
        }

        let resp_body: serde_json::Value = serde_json::from_str(&resp_text)
            .map_err(|e| crate::CertError::Api(format!("响应解析失败: {e}")))?;

        // certd API 响应: code=0 成功，其他为错误
        let code = resp_body.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            let msg = resp_body.get("message")
                .and_then(|m| m.as_str())
                .or_else(|| resp_body.get("msg").and_then(|m| m.as_str()))
                .unwrap_or("unknown error");
            return Err(crate::CertError::Api(format!("API 错误 (code={}): {}", code, msg)));
        }

        let data = resp_body.get("data")
            .ok_or_else(|| crate::CertError::Api("响应缺少 data 字段".into()))?;

        let cert_pem = data.get("crt")
            .and_then(|c| c.as_str())
            .ok_or_else(|| crate::CertError::Api("响应缺少 crt 字段".into()))?;

        let key_pem = data.get("key")
            .and_then(|k| k.as_str())
            .ok_or_else(|| crate::CertError::Api("响应缺少 key 字段".into()))?;

        let ic = data.get("ic").and_then(|v| v.as_str());
        let oc = data.get("oc").and_then(|v| v.as_str());

        let pfx = data.get("pfx").and_then(|v| v.as_str()).map(String::from);
        let der = data.get("der").and_then(|v| v.as_str()).map(String::from);
        let jks = data.get("jks").and_then(|v| v.as_str()).map(String::from);
        let p7b = data.get("p7b").and_then(|v| v.as_str()).map(String::from);

        // 从 detail 中提取过期时间戳（API 直接返回，比 x509 解析更可靠）
        let detail = data.get("detail");
        let not_after_ts = detail
            .and_then(|d| d.get("notAfter"))
            .and_then(|t| t.as_u64());
        let detail_domains = detail
            .and_then(|d| d.get("domains"))
            .and_then(|d| d.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(CertResponseData {
            cert_pem: cert_pem.to_string(),
            key_pem: key_pem.to_string(),
            ic_pem: ic.map(String::from),
            oc_pem: oc.map(String::from),
            pfx,
            der,
            jks,
            p7b,
            not_after_ts,
            detail_domains,
        })
    }

pub(crate) fn build_token_content(key_id: &str, timestamp: u64) -> String {
    let escaped_key_id = serde_json::to_string(key_id).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "{{\"keyId\":{escaped_key_id},\"t\":{timestamp},\"encrypt\":false,\"signType\":\"md5\"}}"
    )
}

/// 生成 x-certd-token
fn generate_token(&self) -> Result<String, crate::CertError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| crate::CertError::Api(format!("时间错误: {e}")))?
            .as_secs();

        let content_str = Self::build_token_content(&self.key_id, timestamp);

        // 2. 签名 = MD5(content_str + keySecret)
        let sign_input = format!("{}{}", content_str, self.key_secret);
        let mut hasher = Md5::new();
        hasher.update(sign_input.as_bytes());
        let sign = format!("{:x}", hasher.finalize());

        // 3. Token = base64(content_str).base64(sign)
        let content_b64 = BASE64.encode(content_str.as_bytes());
        let sign_b64 = BASE64.encode(sign.as_bytes());

        Ok(format!("{}.{}", content_b64, sign_b64))
    }
}

/// certd API 返回的证书数据
#[derive(Debug, Clone)]
pub struct CertResponseData {
    pub cert_pem: String,
    pub key_pem: String,
    pub ic_pem: Option<String>,
    pub oc_pem: Option<String>,
    pub pfx: Option<String>,
    pub der: Option<String>,
    pub jks: Option<String>,
    pub p7b: Option<String>,
    /// API 直接返回的过期时间戳 (detail.notAfter)
    pub not_after_ts: Option<u64>,
    pub detail_domains: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_content_uses_documented_field_order() {
        assert_eq!(
            CertdClient::build_token_content("key-123", 1_700_000_000),
            r#"{"keyId":"key-123","t":1700000000,"encrypt":false,"signType":"md5"}"#
        );
    }

    #[test]
    fn token_content_escapes_key_id() {
        assert_eq!(
            CertdClient::build_token_content("a\"b", 1),
            r#"{"keyId":"a\"b","t":1,"encrypt":false,"signType":"md5"}"#
        );
    }
}
