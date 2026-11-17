// opsctl-dns: 多平台 DNS 统一管理（纯实时，无本地状态）

pub mod error;
pub mod operations;
pub mod providers;
pub mod signing;
pub mod types;
pub mod validation;

pub use error::DnsError;
pub use operations::{
    create_record, delete_record, find_zone, list_domains, list_records, provider_name,
    record_input, record_update, selected_accounts, set_record_status, update_record, verify,
};
pub use types::{
    DnsProvider, ProviderDomain, ProviderRecord, ProviderZone, RecordInput, RecordUpdate,
};
