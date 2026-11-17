use serde_json::Value;
use sha2::Digest;

use crate::{
    DnsError, DnsProvider, ProviderDomain, ProviderRecord, ProviderZone, RecordInput, RecordUpdate,
    signing::hmac_sha256,
};

const CHINA_ENDPOINT: &str = "https://dnspod.tencentcloudapi.com";
const API_VERSION: &str = "2021-03-23";
const PAGE_SIZE: u64 = 100;

pub(crate) struct Tc3Input<'a> {
    pub secret_id: &'a str,
    pub secret_key: &'a str,
    pub timestamp: i64,
    pub service: &'a str,
    pub host: &'a str,
    pub action: &'a str,
    pub payload: &'a str,
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha2::Sha256::digest(data))
}

fn date_from_timestamp(timestamp: i64) -> String {
    let days = timestamp.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

/// 腾讯云 TC3-HMAC-SHA256 签名。
pub(crate) fn tc3_authorization(input: &Tc3Input<'_>) -> String {
    let date = date_from_timestamp(input.timestamp);
    let canonical_request = format!(
        "POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:{}\nx-tc-action:{}\n\ncontent-type;host;x-tc-action\n{}",
        input.host,
        input.action.to_lowercase(),
        sha256_hex(input.payload.as_bytes())
    );
    let credential_scope = format!("{}/{}/tc3_request", date, input.service);
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{}\n{}\n{}",
        input.timestamp,
        credential_scope,
        sha256_hex(canonical_request.as_bytes())
    );

    let secret_date = hmac_sha256(
        format!("TC3{}", input.secret_key).as_bytes(),
        date.as_bytes(),
    );
    let secret_service = hmac_sha256(&secret_date, input.service.as_bytes());
    let secret_signing = hmac_sha256(&secret_service, b"tc3_request");
    let signature = hex::encode(hmac_sha256(&secret_signing, string_to_sign.as_bytes()));

    format!(
        "TC3-HMAC-SHA256 Credential={}/{}, SignedHeaders=content-type;host;x-tc-action, Signature={}",
        input.secret_id, credential_scope, signature
    )
}

fn map_error(err: &Value) -> DnsError {
    let code = err
        .get("Code")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let message = err
        .get("Message")
        .and_then(|v| v.as_str())
        .unwrap_or("腾讯云返回未知错误")
        .to_string();
    if code.starts_with("AuthFailure") || code.starts_with("InvalidCredential") {
        DnsError::Auth(format!("腾讯云凭据无效: {message}"))
    } else if code == "UnauthorizedOperation" {
        DnsError::Auth(format!(
            "腾讯云密钥无权限（CAM 未授权该接口或服务未开通）: {message}"
        ))
    } else if code.contains("LimitExceeded") || code.contains("RequestLimit") {
        DnsError::RateLimited(format!("腾讯云限流: {message}"))
    } else if code.starts_with("ResourceNotFound") {
        DnsError::NotFound(message)
    } else {
        DnsError::Api { code, message }
    }
}

fn parse_domain(v: &Value) -> Option<ProviderDomain> {
    let dns_status = v.get("DNSStatus").and_then(|s| s.as_str()).unwrap_or("");
    if dns_status.eq_ignore_ascii_case("DNSERROR") {
        return None;
    }
    Some(ProviderDomain {
        name: v.get("Name")?.as_str()?.to_string(),
        provider_domain_id: v
            .get("DomainId")
            .map(|v| v.to_string().trim_matches('"').to_string())
            .unwrap_or_default(),
        status: v
            .get("Status")
            .and_then(|s| s.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

fn parse_record(v: &Value) -> Option<ProviderRecord> {
    let provider_record_id = match v.get("RecordId")? {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => return None,
    };
    Some(ProviderRecord {
        provider_record_id,
        host: v
            .get("Name")
            .and_then(|n| n.as_str())
            .unwrap_or("@")
            .to_string(),
        rtype: v.get("Type")?.as_str()?.to_string(),
        value: v.get("Value")?.as_str()?.to_string(),
        ttl: v.get("TTL").and_then(|t| t.as_u64()).unwrap_or(600),
        line: v.get("Line").and_then(|l| l.as_str()).map(str::to_string),
        proxied: None,
        enabled: v
            .get("Status")
            .and_then(|s| s.as_str())
            .map(|s| s.eq_ignore_ascii_case("ENABLE"))
            .unwrap_or(true),
        remark: v
            .get("Remark")
            .and_then(|r| r.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    })
}

fn numeric_id(id: &str) -> Result<i64, DnsError> {
    id.parse::<i64>().map_err(|_| DnsError::Api {
        code: "BadRecordId".into(),
        message: format!("腾讯云记录 ID 必须是数字: {id}"),
    })
}

pub struct DnspodProvider {
    secret_id: String,
    secret_key: String,
    endpoint: String,
}

impl DnspodProvider {
    pub fn new(secret_id: impl Into<String>, secret_key: impl Into<String>) -> Self {
        Self {
            secret_id: secret_id.into(),
            secret_key: secret_key.into(),
            endpoint: CHINA_ENDPOINT.to_string(),
        }
    }

    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = endpoint.trim_end_matches('/').to_string();
        self
    }

    fn host_header(&self) -> String {
        self.endpoint
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string()
    }

    fn default_line(&self) -> &'static str {
        "默认"
    }

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .http_status_as_error(false)
            .build()
            .into()
    }

    fn call(&self, action: &str, payload: Value) -> Result<Value, DnsError> {
        let body = serde_json::to_string(&payload).map_err(|e| DnsError::Api {
            code: "BadRequest".into(),
            message: format!("请求序列化失败: {e}"),
        })?;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let host = self.host_header();
        let authorization = tc3_authorization(&Tc3Input {
            secret_id: &self.secret_id,
            secret_key: &self.secret_key,
            timestamp,
            service: "dnspod",
            host: &host,
            action,
            payload: &body,
        });

        let response = Self::agent()
            .post(&format!("{}/", self.endpoint))
            .header("Authorization", &authorization)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("X-TC-Action", action)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("X-TC-Version", API_VERSION)
            .send(&body)
            .map_err(|e| DnsError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        let mut resp_body = response.into_body();
        let bytes = resp_body
            .read_to_vec()
            .map_err(|e| DnsError::Network(format!("读取响应失败: {e}")))?;
        let parsed: Value = serde_json::from_slice(&bytes).map_err(|e| DnsError::Api {
            code: "BadResponse".into(),
            message: format!("响应不是合法 JSON: {e}"),
        })?;
        if let Some(err) = parsed.pointer("/Response/Error") {
            return Err(map_error(err));
        }
        if !(200..300).contains(&status) {
            return Err(DnsError::Api {
                code: format!("HTTP{status}"),
                message: "腾讯云返回非 2xx 响应且无错误详情".into(),
            });
        }
        parsed
            .get("Response")
            .cloned()
            .ok_or_else(|| DnsError::Api {
                code: "BadResponse".into(),
                message: "响应缺少 Response 字段".into(),
            })
    }
}

impl DnsProvider for DnspodProvider {
    fn verify(&self) -> Result<(), DnsError> {
        self.call("DescribeDomainList", serde_json::json!({"Limit":1,"Offset":0}))?;
        Ok(())
    }

    fn list_domains(&self) -> Result<Vec<ProviderDomain>, DnsError> {
        let mut out = Vec::new();
        let mut offset = 0u64;
        loop {
            let body = self.call(
                "DescribeDomainList",
                serde_json::json!({"Limit":PAGE_SIZE,"Offset":offset}),
            )?;
            let batch: Vec<ProviderDomain> = body
                .get("DomainList")
                .and_then(|v| v.as_array())
                .map(|items| items.iter().filter_map(parse_domain).collect())
                .unwrap_or_default();
            let done = (batch.len() as u64) < PAGE_SIZE;
            out.extend(batch);
            if done {
                break;
            }
            offset += PAGE_SIZE;
        }
        Ok(out)
    }

    fn list_records(&self, zone: &ProviderZone) -> Result<Vec<ProviderRecord>, DnsError> {
        let mut out = Vec::new();
        let mut offset = 0u64;
        loop {
            let body = self.call(
                "DescribeRecordList",
                serde_json::json!({"Domain":zone.name,"Limit":PAGE_SIZE,"Offset":offset}),
            )?;
            let batch: Vec<ProviderRecord> = body
                .get("RecordList")
                .and_then(|v| v.as_array())
                .map(|items| items.iter().filter_map(parse_record).collect())
                .unwrap_or_default();
            let done = (batch.len() as u64) < PAGE_SIZE;
            out.extend(batch);
            if done {
                break;
            }
            offset += PAGE_SIZE;
        }
        Ok(out)
    }

    fn create_record(
        &self,
        zone: &ProviderZone,
        input: &RecordInput,
    ) -> Result<ProviderRecord, DnsError> {
        let line = input
            .line
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.default_line().to_string());
        let mut payload = serde_json::json!({
            "Domain": zone.name,
            "SubDomain": input.host,
            "RecordType": input.rtype,
            "RecordLine": line,
            "Value": input.value,
            "TTL": input.ttl,
        });
        if let Some(remark) = input.remark.as_deref().filter(|r| !r.is_empty()) {
            payload["Remark"] = Value::String(remark.to_string());
        }
        let body = self.call("CreateRecord", payload)?;
        let record_id = match body.get("RecordId") {
            Some(Value::Number(n)) => n.to_string(),
            Some(Value::String(s)) => s.clone(),
            _ => {
                return Err(DnsError::Api {
                    code: "BadResponse".into(),
                    message: "腾讯云未返回 RecordId".into(),
                })
            }
        };
        Ok(ProviderRecord {
            provider_record_id: record_id,
            host: input.host.clone(),
            rtype: input.rtype.clone(),
            value: input.value.clone(),
            ttl: input.ttl,
            line: Some(line),
            proxied: None,
            enabled: true,
            remark: input.remark.clone(),
        })
    }

    fn update_record(
        &self,
        zone: &ProviderZone,
        record: &ProviderRecord,
        update: &RecordUpdate,
    ) -> Result<ProviderRecord, DnsError> {
        let host = update.host.clone().unwrap_or_else(|| record.host.clone());
        let rtype = update.rtype.clone().unwrap_or_else(|| record.rtype.clone());
        let value = update.value.clone().unwrap_or_else(|| record.value.clone());
        let ttl = update.ttl.unwrap_or(record.ttl);
        let line = update
            .line
            .clone()
            .filter(|s| !s.is_empty())
            .or_else(|| record.line.clone())
            .unwrap_or_else(|| self.default_line().to_string());
        let remark = update.remark.clone().or_else(|| record.remark.clone());
        let mut payload = serde_json::json!({
            "Domain": zone.name,
            "RecordId": numeric_id(&record.provider_record_id)?,
            "SubDomain": host,
            "RecordType": rtype,
            "RecordLine": line,
            "Value": value,
            "TTL": ttl,
        });
        if let Some(remark) = remark.as_deref() {
            payload["Remark"] = Value::String(remark.to_string());
        }
        self.call("ModifyRecord", payload)?;
        Ok(ProviderRecord {
            provider_record_id: record.provider_record_id.clone(),
            host,
            rtype,
            value,
            ttl,
            line: Some(line),
            proxied: None,
            enabled: record.enabled,
            remark,
        })
    }

    fn delete_record(&self, zone: &ProviderZone, record_id: &str) -> Result<(), DnsError> {
        self.call(
            "DeleteRecord",
            serde_json::json!({"Domain":zone.name,"RecordId":numeric_id(record_id)?}),
        )?;
        Ok(())
    }

    fn set_record_status(
        &self,
        zone: &ProviderZone,
        record_id: &str,
        enabled: bool,
    ) -> Result<(), DnsError> {
        self.call(
            "ModifyRecordStatus",
            serde_json::json!({
                "Domain": zone.name,
                "RecordId": numeric_id(record_id)?,
                "Status": if enabled { "ENABLE" } else { "DISABLE" },
            }),
        )?;
        Ok(())
    }

    fn supports_record_remark(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::test_server::{MockResponse, MockServer};
    use crate::types::{ProviderZone, RecordInput, RecordUpdate};
    use crate::DnsError;
    use serde_json::Value;

    #[test]
    fn tc3_signature_baseline() {
        let auth = tc3_authorization(&Tc3Input {
            secret_id: "AKIDtest",
            secret_key: "testkey",
            timestamp: 1_551_113_065,
            service: "dnspod",
            host: "dnspod.tencentcloudapi.com",
            action: "DescribeRecordList",
            payload: r#"{"Domain":"example.com","Limit":20,"Offset":0}"#,
        });
        assert_eq!(
            auth,
            "TC3-HMAC-SHA256 Credential=AKIDtest/2019-02-25/dnspod/tc3_request, \
             SignedHeaders=content-type;host;x-tc-action, \
             Signature=2ea8fa24e4ccf049105b13ee826fa606dfa6c1f160fa1e7151568725e8e329d4"
        );
    }

    #[test]
    fn verify_sends_tc3_authorization_header() {
        let server = MockServer::new(vec![MockResponse::ok(
            r#"{"Response":{"DomainList":[],"TotalCount":0}}"#,
        )]);
        let provider = DnspodProvider::new("id", "key").with_endpoint(server.base_url());
        provider.verify().unwrap();

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/");
        let auth = requests[0].authorization.as_deref().unwrap_or("");
        assert!(auth.starts_with("TC3-HMAC-SHA256 Credential=id/"));
        assert!(auth.contains("SignedHeaders=content-type;host;x-tc-action"));
        assert!(auth.contains("Signature="));
    }

    #[test]
    fn maps_auth_failure_to_auth_error() {
        let server = MockServer::new(vec![MockResponse::ok(
            r#"{"Response":{"Error":{"Code":"AuthFailure.SignatureFailure","Message":"bad sign"}}}"#,
        )]);
        let provider = DnspodProvider::new("id", "key").with_endpoint(server.base_url());
        assert!(matches!(provider.verify(), Err(DnsError::Auth(_))));
    }

    #[test]
    fn maps_dnspod_record_fields() {
        let server = MockServer::new(vec![MockResponse::ok(
            r#"{
                "Response":{"RecordList":[{
                    "RecordId":123,"Name":"www","Type":"A","Value":"1.2.3.4",
                    "TTL":600,"Line":"默认","Status":"ENABLE","Remark":"web"
                }]}
            }"#,
        )]);
        let provider = DnspodProvider::new("id", "key").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        let records = provider.list_records(&zone).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provider_record_id, "123");
        assert_eq!(records[0].host, "www");
        assert_eq!(records[0].rtype, "A");
        assert_eq!(records[0].value, "1.2.3.4");
        assert_eq!(records[0].line.as_deref(), Some("默认"));
        assert!(records[0].enabled);
        assert_eq!(records[0].remark.as_deref(), Some("web"));
    }

    #[test]
    fn create_record_sends_default_line_and_remark() {
        let server = MockServer::new(vec![MockResponse::ok(
            r#"{"Response":{"RecordId":456}}"#,
        )]);
        let provider = DnspodProvider::new("id", "key").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        let input = RecordInput {
            host: "api".into(),
            rtype: "A".into(),
            value: "5.6.7.8".into(),
            ttl: 300,
            line: None,
            proxied: None,
            remark: Some("api".into()),
        };
        let created = provider.create_record(&zone, &input).unwrap();
        assert_eq!(created.provider_record_id, "456");
        assert_eq!(created.line.as_deref(), Some("默认"));

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        let sent: Value = serde_json::from_str(&requests[0].body).unwrap();
        assert_eq!(sent["SubDomain"], "api");
        assert_eq!(sent["RecordLine"], "默认");
        assert_eq!(sent["Remark"], "api");
    }

    #[test]
    fn update_record_sends_modify_record() {
        let server = MockServer::new(vec![MockResponse::ok(r#"{"Response":{}}"#)]);
        let provider = DnspodProvider::new("id", "key").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        let existing = ProviderRecord {
            provider_record_id: "123".into(),
            host: "www".into(),
            rtype: "A".into(),
            value: "1.2.3.4".into(),
            ttl: 600,
            line: Some("默认".into()),
            proxied: None,
            enabled: true,
            remark: Some("old".into()),
        };
        let update = RecordUpdate {
            value: Some("9.9.9.9".into()),
            remark: Some("new".into()),
            ..Default::default()
        };
        let updated = provider.update_record(&zone, &existing, &update).unwrap();
        assert_eq!(updated.value, "9.9.9.9");

        let sent: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
        assert_eq!(sent["RecordId"], 123);
        assert_eq!(sent["Value"], "9.9.9.9");
        assert_eq!(sent["Remark"], "new");
    }

    #[test]
    fn set_record_status_sends_modify_record_status() {
        let server = MockServer::new(vec![MockResponse::ok(r#"{"Response":{}}"#)]);
        let provider = DnspodProvider::new("id", "key").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        provider.set_record_status(&zone, "123", false).unwrap();
        let sent: Value = serde_json::from_str(&server.requests()[0].body).unwrap();
        assert_eq!(sent["Status"], "DISABLE");
    }

    #[test]
    fn delete_record_sends_delete_record() {
        let server = MockServer::new(vec![MockResponse::ok(r#"{"Response":{}}"#)]);
        let provider = DnspodProvider::new("id", "key").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        provider.delete_record(&zone, "123").unwrap();
        assert_eq!(server.requests().len(), 1);
    }
}
