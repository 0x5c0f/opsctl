use serde_json::{Value, json};

use super::{fqdn_to_host, host_to_fqdn};
use crate::{
    DnsError, DnsProvider, ProviderDomain, ProviderRecord, ProviderZone, RecordInput, RecordUpdate,
};

const DEFAULT_BASE: &str = "https://api.cloudflare.com/client/v4";
const ZONE_PAGE_SIZE: u64 = 50;
const RECORD_PAGE_SIZE: u64 = 100;

pub struct CloudflareProvider {
    api_token: String,
    base_url: String,
}

impl CloudflareProvider {
    pub fn new(api_token: impl Into<String>) -> Self {
        Self {
            api_token: api_token.into(),
            base_url: DEFAULT_BASE.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.base_url = base_url.trim_end_matches('/').to_string();
        self
    }

    fn agent() -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .build()
            .into()
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, DnsError> {
        let url = format!("{}{}", self.base_url, path);
        let agent = Self::agent();
        let auth = format!("Bearer {}", self.api_token);
        let response = match method {
            "GET" => agent.get(&url).header("Authorization", &auth).call(),
            "POST" => agent
                .post(&url)
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .send_json(body.ok_or_else(|| DnsError::Validation("POST 缺少 JSON body".into()))?),
            "PATCH" => agent
                .patch(&url)
                .header("Authorization", &auth)
                .header("Content-Type", "application/json")
                .send_json(body.ok_or_else(|| DnsError::Validation("PATCH 缺少 JSON body".into()))?),
            "DELETE" => agent.delete(&url).header("Authorization", &auth).call(),
            other => {
                return Err(DnsError::Api {
                    code: "UnsupportedMethod".into(),
                    message: format!("不支持的 HTTP 方法: {other}"),
                })
            }
        };

        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(code)) => {
                return Err(match code {
                    401 | 403 => DnsError::Auth(format!("Cloudflare 返回 HTTP {code}")),
                    _ => DnsError::Api {
                        code: format!("HTTP{code}"),
                        message: "Cloudflare API 请求失败".into(),
                    },
                });
            }
            Err(e) => return Err(DnsError::Network(e.to_string())),
        };

        let status = response.status().as_u16();
        let mut resp_body = response.into_body();
        let bytes = resp_body
            .read_to_vec()
            .map_err(|e| DnsError::Network(format!("读取响应失败: {e}")))?;
        let text = String::from_utf8_lossy(&bytes).to_string();

        if status == 401 || status == 403 {
            return Err(DnsError::Auth("Cloudflare 凭据无效或权限不足".into()));
        }
        if !(200..300).contains(&status) {
            return Err(DnsError::Api {
                code: format!("HTTP{status}"),
                message: text.chars().take(300).collect(),
            });
        }

        let value: Value = serde_json::from_str(&text)
            .map_err(|e| DnsError::Api {
                code: "InvalidJson".into(),
                message: format!("响应解析失败: {e}"),
            })?;
        if value.get("success").and_then(|v| v.as_bool()) != Some(true) {
            let (code, message) = value
                .get("errors")
                .and_then(|e| e.get(0))
                .map(|e| {
                    (
                        e.get("code").and_then(|c| c.as_i64()).unwrap_or(0).to_string(),
                        e.get("message").and_then(|m| m.as_str()).unwrap_or("unknown").to_string(),
                    )
                })
                .unwrap_or_else(|| ("unknown".into(), "Cloudflare API 返回失败".into()));
            return Err(DnsError::Api { code, message });
        }
        Ok(value)
    }

    fn parse_record(value: &Value, zone: &str) -> Option<ProviderRecord> {
        Some(ProviderRecord {
            provider_record_id: value.get("id")?.as_str()?.to_string(),
            host: fqdn_to_host(value.get("name")?.as_str()?, zone),
            rtype: value.get("type")?.as_str()?.to_string(),
            value: value.get("content")?.as_str()?.to_string(),
            ttl: value.get("ttl").and_then(|v| v.as_u64()).unwrap_or(1),
            line: None,
            proxied: value.get("proxied").and_then(|v| v.as_bool()),
            enabled: true,
            remark: value
                .get("comment")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        })
    }
}

impl DnsProvider for CloudflareProvider {
    fn verify(&self) -> Result<(), DnsError> {
        self.request("GET", "/zones?per_page=1&page=1", None)?;
        Ok(())
    }

    fn list_domains(&self) -> Result<Vec<ProviderDomain>, DnsError> {
        let mut out = Vec::new();
        let mut page = 1u64;
        loop {
            let value = self.request(
                "GET",
                &format!("/zones?page={page}&per_page={ZONE_PAGE_SIZE}"),
                None,
            )?;
            let result = value.get("result").and_then(|v| v.as_array()).ok_or_else(|| {
                DnsError::Api {
                    code: "InvalidResponse".into(),
                    message: "Cloudflare zones 响应缺少 result".into(),
                }
            })?;
            for item in result {
                let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                out.push(ProviderDomain {
                    name: name.to_string(),
                    provider_domain_id: item
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    status: item
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                });
            }
            let total_pages = value
                .get("result_info")
                .and_then(|v| v.get("total_pages"))
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            if page >= total_pages {
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
            let value = self.request(
                "GET",
                &format!(
                    "/zones/{}/dns_records?page={page}&per_page={RECORD_PAGE_SIZE}",
                    zone.provider_domain_id
                ),
                None,
            )?;
            let result = value.get("result").and_then(|v| v.as_array()).ok_or_else(|| {
                DnsError::Api {
                    code: "InvalidResponse".into(),
                    message: "Cloudflare dns_records 响应缺少 result".into(),
                }
            })?;
            for item in result {
                if let Some(record) = Self::parse_record(item, &zone.name) {
                    out.push(record);
                }
            }
            let total_pages = value
                .get("result_info")
                .and_then(|v| v.get("total_pages"))
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            if page >= total_pages {
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
        let body = json!({
            "name": host_to_fqdn(&input.host, &zone.name),
            "type": input.rtype,
            "content": input.value,
            "ttl": input.ttl,
            "comment": input.remark,
            "proxied": input.proxied,
        });
        let value = self.request(
            "POST",
            &format!("/zones/{}/dns_records", zone.provider_domain_id),
            Some(&body),
        )?;
        Self::parse_record(
            value.get("result").ok_or_else(|| DnsError::Api {
                code: "InvalidResponse".into(),
                message: "Cloudflare 创建记录响应缺少 result".into(),
            })?,
            &zone.name,
        )
        .ok_or_else(|| DnsError::Api {
            code: "InvalidResponse".into(),
            message: "Cloudflare 创建记录响应字段不完整".into(),
        })
    }

    fn update_record(
        &self,
        zone: &ProviderZone,
        record: &ProviderRecord,
        update: &RecordUpdate,
    ) -> Result<ProviderRecord, DnsError> {
        let body = json!({
            "name": host_to_fqdn(
                update.host.as_deref().unwrap_or(&record.host),
                &zone.name,
            ),
            "type": update.rtype.as_deref().unwrap_or(&record.rtype),
            "content": update.value.as_deref().unwrap_or(&record.value),
            "ttl": update.ttl.unwrap_or(record.ttl),
            "comment": update.remark.clone().or_else(|| record.remark.clone()),
            "proxied": update.proxied.or(record.proxied),
        });
        let value = self.request(
            "PATCH",
            &format!(
                "/zones/{}/dns_records/{}",
                zone.provider_domain_id, record.provider_record_id
            ),
            Some(&body),
        )?;
        Self::parse_record(
            value.get("result").ok_or_else(|| DnsError::Api {
                code: "InvalidResponse".into(),
                message: "Cloudflare 修改记录响应缺少 result".into(),
            })?,
            &zone.name,
        )
        .ok_or_else(|| DnsError::Api {
            code: "InvalidResponse".into(),
            message: "Cloudflare 修改记录响应字段不完整".into(),
        })
    }

    fn delete_record(&self, zone: &ProviderZone, record_id: &str) -> Result<(), DnsError> {
        self.request(
            "DELETE",
            &format!(
                "/zones/{}/dns_records/{record_id}",
                zone.provider_domain_id
            ),
            None,
        )?;
        Ok(())
    }

    fn set_record_status(
        &self,
        _zone: &ProviderZone,
        _record_id: &str,
        _enabled: bool,
    ) -> Result<(), DnsError> {
        Err(DnsError::Unsupported(
            "Cloudflare API 不支持记录启用/停用".into(),
        ))
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

    const ZONES: &str = r#"{
        "success": true,
        "result": [{"id":"zone-1","name":"example.com","status":"active"}],
        "result_info": {"page":1,"total_pages":1}
    }"#;

    const RECORDS: &str = r#"{
        "success": true,
        "result": [{
            "id":"rec-1","name":"www.example.com","type":"A",
            "content":"1.2.3.4","ttl":600,"proxied":true,"comment":"web"
        }],
        "result_info": {"page":1,"total_pages":1}
    }"#;

    const CREATED: &str = r#"{
        "success": true,
        "result": {
            "id":"rec-2","name":"api.example.com","type":"A",
            "content":"5.6.7.8","ttl":300,"proxied":false,"comment":"api"
        }
    }"#;

    #[test]
    fn sends_bearer_token_on_verify() {
        let server = MockServer::new(vec![MockResponse::ok(ZONES)]);
        let provider = CloudflareProvider::new("cf-token").with_base_url(server.base_url());
        provider.verify().unwrap();

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "GET");
        assert_eq!(requests[0].path, "/zones?per_page=1&page=1");
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer cf-token")
        );
    }

    #[test]
    fn maps_401_to_auth_error() {
        let server = MockServer::new(vec![MockResponse {
            status: 401,
            body: r#"{"success":false,"errors":[{"code":9109,"message":"Invalid token"}]}"#,
        }]);
        let provider = CloudflareProvider::new("bad").with_base_url(server.base_url());
        assert!(matches!(provider.verify(), Err(DnsError::Auth(_))));
    }

    #[test]
    fn maps_cloudflare_zone_and_record_fields() {
        let server = MockServer::new(vec![MockResponse::ok(ZONES), MockResponse::ok(RECORDS)]);
        let provider = CloudflareProvider::new("token").with_base_url(server.base_url());

        let domains = provider.list_domains().unwrap();
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].name, "example.com");
        assert_eq!(domains[0].provider_domain_id, "zone-1");

        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: "zone-1".into(),
        };
        let records = provider.list_records(&zone).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].host, "www");
        assert_eq!(records[0].rtype, "A");
        assert_eq!(records[0].value, "1.2.3.4");
        assert_eq!(records[0].ttl, 600);
        assert_eq!(records[0].proxied, Some(true));
        assert_eq!(records[0].remark.as_deref(), Some("web"));
    }

    #[test]
    fn create_record_sends_fqdn_and_comment() {
        let server = MockServer::new(vec![MockResponse::ok(CREATED)]);
        let provider = CloudflareProvider::new("token").with_base_url(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: "zone-1".into(),
        };
        let input = RecordInput {
            host: "api".into(),
            rtype: "A".into(),
            value: "5.6.7.8".into(),
            ttl: 300,
            line: None,
            proxied: Some(false),
            remark: Some("api".into()),
        };

        let created = provider.create_record(&zone, &input).unwrap();
        assert_eq!(created.provider_record_id, "rec-2");
        assert_eq!(created.host, "api");
        assert_eq!(created.remark.as_deref(), Some("api"));

        let requests = server.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
        assert_eq!(requests[0].path, "/zones/zone-1/dns_records");
        let sent: Value = serde_json::from_str(&requests[0].body).unwrap();
        assert_eq!(sent["name"], "api.example.com");
        assert_eq!(sent["comment"], "api");
    }

    #[test]
    fn update_record_sends_patch_to_record_id() {
        let server = MockServer::new(vec![MockResponse::ok(CREATED)]);
        let provider = CloudflareProvider::new("token").with_base_url(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: "zone-1".into(),
        };
        let existing = ProviderRecord {
            provider_record_id: "rec-2".into(),
            host: "api".into(),
            rtype: "A".into(),
            value: "5.6.7.8".into(),
            ttl: 300,
            line: None,
            proxied: Some(false),
            enabled: true,
            remark: Some("api".into()),
        };
        let update = RecordUpdate {
            value: Some("9.9.9.9".into()),
            ..Default::default()
        };
        provider.update_record(&zone, &existing, &update).unwrap();

        let requests = server.requests();
        assert_eq!(requests[0].method, "PATCH");
        assert_eq!(requests[0].path, "/zones/zone-1/dns_records/rec-2");
        let sent: Value = serde_json::from_str(&requests[0].body).unwrap();
        assert_eq!(sent["content"], "9.9.9.9");
    }

    #[test]
    fn delete_record_sends_delete() {
        let server = MockServer::new(vec![MockResponse::ok(
            r#"{"success":true,"result":{"id":"rec-1"}}"#,
        )]);
        let provider = CloudflareProvider::new("token").with_base_url(server.base_url());
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: "zone-1".into(),
        };
        provider.delete_record(&zone, "rec-1").unwrap();

        let requests = server.requests();
        assert_eq!(requests[0].method, "DELETE");
        assert_eq!(requests[0].path, "/zones/zone-1/dns_records/rec-1");
    }

    #[test]
    fn set_record_status_is_unsupported() {
        let provider = CloudflareProvider::new("token");
        let zone = ProviderZone {
            name: "example.com".into(),
            provider_domain_id: "zone-1".into(),
        };
        assert!(matches!(
            provider.set_record_status(&zone, "rec-1", false),
            Err(DnsError::Unsupported(_))
        ));
    }
}
