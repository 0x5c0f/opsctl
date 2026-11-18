use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::Value;

use crate::{
    DnsError, DnsProvider, ProviderDomain, ProviderRecord, ProviderZone, RecordInput, RecordUpdate,
    signing::{hmac_sha1, percent_encode as sign_percent_encode},
};

const DEFAULT_ENDPOINT: &str = "https://alidns.aliyuncs.com";
const API_VERSION: &str = "2015-01-09";
const PAGE_SIZE: u64 = 100;
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 阿里云 RPC V1 签名（POP / HMAC-SHA1）。
pub(crate) fn sign_pop(secret: &str, params: &BTreeMap<String, String>) -> String {
    let canonical = params
        .iter()
        .map(|(k, v)| format!("{}={}", sign_percent_encode(k), sign_percent_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let string_to_sign = format!("GET&%2F&{}", sign_percent_encode(&canonical));
    let digest = hmac_sha1(format!("{secret}&").as_bytes(), string_to_sign.as_bytes());
    BASE64.encode(digest)
}

fn nonce() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    hex::encode(format!("{now}-{}-{counter}", std::process::id()).as_bytes())
        .chars()
        .take(32)
        .collect()
}

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 不引入 chrono：用 Unix 秒反推 UTC 日历时间，仅用于 API 签名。
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // Howard Hinnant's civil_from_days algorithm.
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

fn map_error(body: &Value) -> DnsError {
    let code = body
        .get("Code")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let message = body
        .get("Message")
        .and_then(|v| v.as_str())
        .unwrap_or("阿里云返回未知错误")
        .to_string();
    match code.as_str() {
        "InvalidAccessKeyId.NotFound"
        | "InvalidAccessKeyId"
        | "SignatureDoesNotMatch"
        | "Forbidden"
        | "Unauthorized" => DnsError::Auth(format!("阿里云凭据无效: {message}")),
        "Throttling" | "Throttling.User" | "Throttling.UserQos" | "FlowLimitExceeded" => {
            DnsError::RateLimited(format!("阿里云限流: {message}"))
        }
        "InvalidDomain.NotFound" | "InvalidRecord.NotFound" | "DomainNotExists" => {
            DnsError::NotFound(message)
        }
        _ => DnsError::Api { code, message },
    }
}

fn parse_domain(v: &Value) -> Option<ProviderDomain> {
    Some(ProviderDomain {
        name: v.get("DomainName")?.as_str()?.to_string(),
        provider_domain_id: String::new(),
        status: v
            .get("Status")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("active")
            .to_string(),
    })
}

fn parse_record(v: &Value) -> Option<ProviderRecord> {
    Some(ProviderRecord {
        provider_record_id: v.get("RecordId")?.as_str()?.to_string(),
        host: v.get("RR")?.as_str()?.to_string(),
        rtype: v.get("Type")?.as_str()?.to_string(),
        value: v.get("Value")?.as_str()?.to_string(),
        ttl: v.get("TTL").and_then(|t| t.as_u64()).unwrap_or(600),
        line: v.get("Line").and_then(|l| l.as_str()).map(str::to_string),
        proxied: None,
        enabled: v
            .get("Status")
            .and_then(|s| s.as_str())
            .map(|s| s.eq_ignore_ascii_case("Enable"))
            .unwrap_or(true),
        remark: v
            .get("Remark")
            .and_then(|r| r.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    })
}

pub struct AliyunProvider {
    access_key_id: String,
    access_key_secret: String,
    endpoint: String,
}

impl AliyunProvider {
    pub fn new(access_key_id: impl Into<String>, access_key_secret: impl Into<String>) -> Self {
        Self {
            access_key_id: access_key_id.into(),
            access_key_secret: access_key_secret.into(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
        }
    }

    pub fn with_endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = endpoint.trim_end_matches('/').to_string();
        self
    }

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .http_status_as_error(false)
            .build()
            .into()
    }

    fn call(&self, action: &str, mut params: BTreeMap<String, String>) -> Result<Value, DnsError> {
        params.insert("Action".into(), action.to_string());
        params.insert("Format".into(), "JSON".into());
        params.insert("Version".into(), API_VERSION.into());
        params.insert("AccessKeyId".into(), self.access_key_id.clone());
        params.insert("SignatureMethod".into(), "HMAC-SHA1".into());
        params.insert("SignatureVersion".into(), "1.0".into());
        params.insert("SignatureNonce".into(), nonce());
        params.insert("Timestamp".into(), timestamp());
        let signature = sign_pop(&self.access_key_secret, &params);
        params.insert("Signature".into(), signature);

        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", sign_percent_encode(k), sign_percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let url = format!("{}/?{}", self.endpoint, query);

        let response = Self::agent().get(&url).call().map_err(|e| match e {
            ureq::Error::StatusCode(code) => DnsError::Api {
                code: format!("HTTP{code}"),
                message: format!("阿里云返回 HTTP {code}"),
            },
            other => DnsError::Network(other.to_string()),
        })?;
        let status = response.status().as_u16();
        let mut body = response.into_body();
        let bytes = body
            .read_to_vec()
            .map_err(|e| DnsError::Network(format!("读取响应失败: {e}")))?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| DnsError::Api {
            code: "BadResponse".into(),
            message: format!("响应不是合法 JSON: {e}"),
        })?;
        if !(200..300).contains(&status) {
            return Err(map_error(&value));
        }
        if let Some(code) = value.get("Code").and_then(|v| v.as_str())
            && !code.eq_ignore_ascii_case("Success")
        {
            return Err(map_error(&value));
        }
        Ok(value)
    }

    fn set_remark(&self, record_id: &str, remark: &str) -> Result<(), DnsError> {
        let mut params = BTreeMap::new();
        params.insert("RecordId".into(), record_id.to_string());
        params.insert("Remark".into(), remark.to_string());
        self.call("UpdateDomainRecordRemark", params)?;
        Ok(())
    }
}

impl DnsProvider for AliyunProvider {
    fn verify(&self) -> Result<(), DnsError> {
        let mut params = BTreeMap::new();
        params.insert("PageSize".into(), "1".into());
        params.insert("PageNumber".into(), "1".into());
        self.call("DescribeDomains", params)?;
        Ok(())
    }

    fn list_domains(&self) -> Result<Vec<ProviderDomain>, DnsError> {
        let mut out = Vec::new();
        let mut page = 1u64;
        loop {
            let mut params = BTreeMap::new();
            params.insert("PageSize".into(), PAGE_SIZE.to_string());
            params.insert("PageNumber".into(), page.to_string());
            let body = self.call("DescribeDomains", params)?;
            let batch: Vec<ProviderDomain> = body
                .pointer("/Domains/Domain")
                .and_then(|v| v.as_array())
                .map(|items| items.iter().filter_map(parse_domain).collect())
                .unwrap_or_default();
            let done = (batch.len() as u64) < PAGE_SIZE;
            out.extend(batch);
            if done {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    fn list_records(&self, zone: &ProviderZone) -> Result<Vec<ProviderRecord>, DnsError> {
        let mut out = Vec::new();
        let mut page = 1u64;
        loop {
            let mut params = BTreeMap::new();
            params.insert("DomainName".into(), zone.name.clone());
            params.insert("PageSize".into(), PAGE_SIZE.to_string());
            params.insert("PageNumber".into(), page.to_string());
            let body = self.call("DescribeDomainRecords", params)?;
            let batch: Vec<ProviderRecord> = body
                .pointer("/DomainRecords/Record")
                .and_then(|v| v.as_array())
                .map(|items| items.iter().filter_map(parse_record).collect())
                .unwrap_or_default();
            let done = (batch.len() as u64) < PAGE_SIZE;
            out.extend(batch);
            if done {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    fn create_record(
        &self,
        zone: &ProviderZone,
        input: &RecordInput,
    ) -> Result<ProviderRecord, DnsError> {
        let line = input.line.clone().unwrap_or_else(|| "default".into());
        let mut params = BTreeMap::new();
        params.insert("DomainName".into(), zone.name.clone());
        params.insert("RR".into(), input.host.clone());
        params.insert("Type".into(), input.rtype.clone());
        params.insert("Value".into(), input.value.clone());
        params.insert("TTL".into(), input.ttl.to_string());
        params.insert("Line".into(), line.clone());
        let body = self.call("AddDomainRecord", params)?;
        let record_id = body
            .get("RecordId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DnsError::Api {
                code: "BadResponse".into(),
                message: "阿里云未返回 RecordId".into(),
            })?
            .to_string();
        if let Some(remark) = input.remark.as_deref().filter(|r| !r.is_empty()) {
            self.set_remark(&record_id, remark)?;
        }
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
            .or_else(|| record.line.clone())
            .unwrap_or_else(|| "default".into());
        let remark = update.remark.clone().or_else(|| record.remark.clone());

        let mut params = BTreeMap::new();
        params.insert("RecordId".into(), record.provider_record_id.clone());
        params.insert("RR".into(), host.clone());
        params.insert("Type".into(), rtype.clone());
        params.insert("Value".into(), value.clone());
        params.insert("TTL".into(), ttl.to_string());
        params.insert("Line".into(), line.clone());
        self.call("UpdateDomainRecord", params)?;
        if let Some(remark) = remark.as_deref() {
            self.set_remark(&record.provider_record_id, remark)?;
        }
        let _ = zone;
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

    fn delete_record(&self, _zone: &ProviderZone, record_id: &str) -> Result<(), DnsError> {
        let mut params = BTreeMap::new();
        params.insert("RecordId".into(), record_id.to_string());
        self.call("DeleteDomainRecord", params)?;
        Ok(())
    }

    fn set_record_status(
        &self,
        _zone: &ProviderZone,
        record_id: &str,
        enabled: bool,
    ) -> Result<(), DnsError> {
        let mut params = BTreeMap::new();
        params.insert("RecordId".into(), record_id.to_string());
        params.insert(
            "Status".into(),
            if enabled { "Enable" } else { "Disable" }.into(),
        );
        self.call("SetDomainRecordStatus", params)?;
        Ok(())
    }

    fn supports_record_remark(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DnsError;
    use crate::providers::test_server::{MockResponse, MockServer};
    use crate::types::{ProviderZone, RecordInput, RecordUpdate};
    use std::collections::BTreeMap;

    #[test]
    fn pop_signature_baseline() {
        let params: BTreeMap<String, String> = [
            ("AccessKeyId", "testkeyid"),
            ("Action", "DescribeDomains"),
            ("Format", "JSON"),
            ("PageNumber", "1"),
            ("PageSize", "20"),
            ("SignatureMethod", "HMAC-SHA1"),
            ("SignatureNonce", "nonce-abc"),
            ("SignatureVersion", "1.0"),
            ("Timestamp", "2026-01-01T00:00:00Z"),
            ("Version", "2015-01-09"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();
        assert_eq!(
            sign_pop("testsecret", &params),
            "UqACuUOUA1QlnatcSKhdUCNlDsA="
        );
    }

    #[test]
    fn verify_sends_signed_describe_domains() {
        let server = MockServer::new(vec![MockResponse::ok(
            r#"{"Domains":{"Domain":[]},"RequestId":"r1"}"#,
        )]);
        let provider = AliyunProvider::new("ak", "secret").with_endpoint(server.base_url());
        provider.verify().unwrap();

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].path.contains("Action=DescribeDomains"));
        assert!(requests[0].path.contains("Version=2015-01-09"));
        assert!(requests[0].path.contains("AccessKeyId=ak"));
        assert!(requests[0].path.contains("SignatureMethod=HMAC-SHA1"));
        assert!(requests[0].path.contains("Signature="));
    }

    #[test]
    fn maps_invalid_access_key_to_auth_error() {
        let server = MockServer::new(vec![MockResponse {
            status: 400,
            body: r#"{"Code":"InvalidAccessKeyId.NotFound","Message":"bad key","RequestId":"r1"}"#,
        }]);
        let provider = AliyunProvider::new("bad", "secret").with_endpoint(server.base_url());
        assert!(matches!(provider.verify(), Err(DnsError::Auth(_))));
    }

    #[test]
    fn maps_aliyun_record_fields() {
        let server = MockServer::new(vec![MockResponse::ok(
            r#"{
                "DomainRecords":{"Record":[{
                    "RecordId":"rec-1","RR":"www","Type":"A","Value":"1.2.3.4",
                    "TTL":600,"Line":"default","Status":"Enable","Remark":"web"
                }]},
                "RequestId":"r1"
            }"#,
        )]);
        let provider = AliyunProvider::new("ak", "secret").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        let records = provider.list_records(&zone).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provider_record_id, "rec-1");
        assert_eq!(records[0].host, "www");
        assert_eq!(records[0].rtype, "A");
        assert_eq!(records[0].value, "1.2.3.4");
        assert_eq!(records[0].line.as_deref(), Some("default"));
        assert!(records[0].enabled);
        assert_eq!(records[0].remark.as_deref(), Some("web"));
    }

    #[test]
    fn create_record_calls_add_then_remark() {
        let server = MockServer::new(vec![
            MockResponse::ok(r#"{"RecordId":"rec-2","RequestId":"r1"}"#),
            MockResponse::ok(r#"{"RequestId":"r2"}"#),
        ]);
        let provider = AliyunProvider::new("ak", "secret").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        let input = RecordInput {
            host: "api".into(),
            rtype: "A".into(),
            value: "5.6.7.8".into(),
            ttl: 300,
            line: Some("default".into()),
            proxied: None,
            remark: Some("api".into()),
        };
        let created = provider.create_record(&zone, &input).unwrap();
        assert_eq!(created.provider_record_id, "rec-2");

        let requests = server.requests();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].path.contains("Action=AddDomainRecord"));
        assert!(requests[0].path.contains("RR=api"));
        assert!(requests[0].path.contains("Line=default"));
        assert!(requests[1].path.contains("Action=UpdateDomainRecordRemark"));
        assert!(requests[1].path.contains("Remark=api"));
    }

    #[test]
    fn update_record_merges_fields_and_syncs_remark() {
        let server = MockServer::new(vec![
            MockResponse::ok(r#"{"RequestId":"r1"}"#),
            MockResponse::ok(r#"{"RequestId":"r2"}"#),
        ]);
        let provider = AliyunProvider::new("ak", "secret").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        let existing = ProviderRecord {
            provider_record_id: "rec-1".into(),
            host: "www".into(),
            rtype: "A".into(),
            value: "1.2.3.4".into(),
            ttl: 600,
            line: Some("default".into()),
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
        assert_eq!(updated.remark.as_deref(), Some("new"));

        let requests = server.requests();
        assert!(requests[0].path.contains("Action=UpdateDomainRecord"));
        assert!(requests[0].path.contains("Value=9.9.9.9"));
        assert!(requests[1].path.contains("Action=UpdateDomainRecordRemark"));
        assert!(requests[1].path.contains("Remark=new"));
    }

    #[test]
    fn set_record_status_maps_disable() {
        let server = MockServer::new(vec![MockResponse::ok(r#"{"RequestId":"r1"}"#)]);
        let provider = AliyunProvider::new("ak", "secret").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        provider.set_record_status(&zone, "rec-1", false).unwrap();
        let requests = server.requests();
        assert!(requests[0].path.contains("Action=SetDomainRecordStatus"));
        assert!(requests[0].path.contains("Status=Disable"));
    }

    #[test]
    fn delete_record_calls_delete_action() {
        let server = MockServer::new(vec![MockResponse::ok(r#"{"RequestId":"r1"}"#)]);
        let provider = AliyunProvider::new("ak", "secret").with_endpoint(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: String::new(),
        };
        provider.delete_record(&zone, "rec-1").unwrap();
        assert!(
            server.requests()[0]
                .path
                .contains("Action=DeleteDomainRecord")
        );
    }
}
