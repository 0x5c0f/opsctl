use serde::Serialize;

use crate::error::DnsError;

#[derive(Debug, Clone, Serialize)]
pub struct ProviderDomain {
    pub name: String,
    pub provider_domain_id: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ProviderZone {
    pub name: String,
    pub provider_domain_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderRecord {
    pub provider_record_id: String,
    pub host: String,
    pub rtype: String,
    pub value: String,
    pub ttl: u64,
    pub line: Option<String>,
    pub proxied: Option<bool>,
    pub enabled: bool,
    pub remark: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RecordInput {
    pub host: String,
    pub rtype: String,
    pub value: String,
    pub ttl: u64,
    pub line: Option<String>,
    pub proxied: Option<bool>,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RecordUpdate {
    pub host: Option<String>,
    pub rtype: Option<String>,
    pub value: Option<String>,
    pub ttl: Option<u64>,
    pub line: Option<String>,
    pub proxied: Option<bool>,
    pub remark: Option<String>,
}

/// 平台适配器：opsctl 是同步 CLI，因此 trait 保持同步，不引入 async runtime。
pub trait DnsProvider {
    fn verify(&self) -> Result<(), DnsError>;
    fn list_domains(&self) -> Result<Vec<ProviderDomain>, DnsError>;
    fn list_records(&self, zone: &ProviderZone) -> Result<Vec<ProviderRecord>, DnsError>;
    fn create_record(
        &self,
        zone: &ProviderZone,
        input: &RecordInput,
    ) -> Result<ProviderRecord, DnsError>;
    fn update_record(
        &self,
        zone: &ProviderZone,
        record: &ProviderRecord,
        update: &RecordUpdate,
    ) -> Result<ProviderRecord, DnsError>;
    fn delete_record(&self, zone: &ProviderZone, record_id: &str) -> Result<(), DnsError>;
    fn set_record_status(
        &self,
        zone: &ProviderZone,
        record_id: &str,
        enabled: bool,
    ) -> Result<(), DnsError>;
    fn supports_record_remark(&self) -> bool;
}
