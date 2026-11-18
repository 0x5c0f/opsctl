use std::io::{self, IsTerminal, Write};

use opsctl_core::{
    GlobalConfig,
    config::{DnsAccount, DnsNotifyLevel},
};
use opsctl_dns::{DnsError, ProviderDomain, ProviderRecord, ProviderZone, selected_accounts};

use crate::cli::{
    DnsAccountSubcommands, DnsDomainSubcommands, DnsRecordSubcommands, DnsSubcommands,
};

pub fn handle_dns(config: &GlobalConfig, sub: &DnsSubcommands, json: bool) -> super::HandlerResult {
    let result = match sub {
        DnsSubcommands::Account { sub } => handle_account(config, sub, json),
        DnsSubcommands::Domain { sub } => handle_domain(config, sub, json),
        DnsSubcommands::Record { sub } => handle_record(config, sub, json),
    };
    result.map_err(|e| opsctl_core::OpsctlError::Other(format!("DNS 操作失败: {e}")))
}

fn handle_account(
    config: &GlobalConfig,
    sub: &DnsAccountSubcommands,
    json: bool,
) -> Result<(), DnsError> {
    match sub {
        DnsAccountSubcommands::List => {
            let accounts = selected_accounts(&config.dns, None)?;
            if json {
                let items: Vec<serde_json::Value> = accounts
                    .iter()
                    .map(|a| serde_json::json!({"name": a.name, "provider": a.provider.as_str()}))
                    .collect();
                println!("{}", json_pretty(&items)?);
            } else {
                println!(
                    "{}",
                    format_accounts_table(
                        &accounts.iter().map(|a| (*a).clone()).collect::<Vec<_>>()
                    )
                );
            }
            Ok(())
        }
        DnsAccountSubcommands::Verify { account } => {
            let accounts = selected_accounts(&config.dns, account.as_deref())?;
            let mut had_error = false;
            for acc in accounts {
                match opsctl_dns::verify(acc) {
                    Ok(()) => println!("✅ [{}] 凭据有效", acc.name),
                    Err(e) => {
                        eprintln!("❌ [{}] 凭据验证失败: {e}", acc.name);
                        had_error = true;
                    }
                }
            }
            if had_error {
                Err(DnsError::Auth("至少一个账户验证失败".into()))
            } else {
                Ok(())
            }
        }
    }
}

fn handle_domain(
    config: &GlobalConfig,
    sub: &DnsDomainSubcommands,
    json: bool,
) -> Result<(), DnsError> {
    let DnsDomainSubcommands::List { account, keyword } = sub;
    let accounts = selected_accounts(&config.dns, account.as_deref())?;
    let mut rows: Vec<(String, ProviderDomain)> = Vec::new();
    let mut had_error = false;

    for acc in accounts {
        match opsctl_dns::list_domains(acc) {
            Ok(domains) => {
                for domain in domains {
                    if keyword
                        .as_deref()
                        .map(|k| domain.name.to_lowercase().contains(&k.to_lowercase()))
                        .unwrap_or(true)
                    {
                        rows.push((acc.name.clone(), domain));
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ [{}] 拉取域名失败: {e}", acc.name);
                had_error = true;
            }
        }
    }

    if json {
        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|(name, d)| {
                serde_json::json!({
                    "account": name,
                    "name": d.name,
                    "provider_domain_id": d.provider_domain_id,
                    "status": d.status,
                })
            })
            .collect();
        println!("{}", json_pretty(&items)?);
    } else {
        println!("{:<20} {:<35} {:<20} 状态", "账户", "域名", "平台ID");
        println!("{}", "-".repeat(90));
        for (name, d) in &rows {
            println!(
                "{:<20} {:<35} {:<20} {}",
                name, d.name, d.provider_domain_id, d.status
            );
        }
    }
    if had_error {
        Err(DnsError::Api {
            code: "PartialFailure".into(),
            message: "部分账户拉取失败".into(),
        })
    } else {
        Ok(())
    }
}

fn handle_record(
    config: &GlobalConfig,
    sub: &DnsRecordSubcommands,
    json: bool,
) -> Result<(), DnsError> {
    match sub {
        DnsRecordSubcommands::List {
            domain,
            account,
            r#type,
            keyword,
        } => list_records(
            config,
            domain,
            account.as_deref(),
            r#type.as_deref(),
            keyword.as_deref(),
            json,
        ),
        DnsRecordSubcommands::Create {
            domain,
            host,
            r#type,
            value,
            ttl,
            line,
            proxied,
            remark,
            account,
            dry_run,
        } => {
            let accounts = selected_accounts(&config.dns, account.as_deref())?;
            if *dry_run {
                println!(
                    "DRY-RUN: 在 {} 个账户下创建记录 {host}.{domain} ({type}) → {value}",
                    accounts.len()
                );
                return Ok(());
            }
            let mut had_error = false;
            for acc in accounts {
                let input = match opsctl_dns::record_input(
                    acc,
                    host,
                    r#type,
                    value,
                    *ttl,
                    line.as_deref(),
                    *proxied,
                    remark.as_deref(),
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("❌ [{}] 参数错误: {e}", acc.name);
                        had_error = true;
                        continue;
                    }
                };
                let result = opsctl_dns::find_zone(acc, domain)
                    .and_then(|zone| opsctl_dns::create_record(acc, &zone, &input));
                report_write(config, &acc.name, "create", result, &mut had_error);
            }
            finish(had_error)
        }
        DnsRecordSubcommands::Update {
            record_id,
            domain,
            host,
            r#type,
            value,
            ttl,
            line,
            proxied,
            remark,
            account,
            dry_run,
        } => {
            let accounts = selected_accounts(&config.dns, account.as_deref())?;
            if *dry_run {
                println!(
                    "DRY-RUN: 在 {} 个账户下修改记录 {record_id} ({domain})",
                    accounts.len()
                );
                return Ok(());
            }
            let mut had_error = false;
            for acc in accounts {
                let result = (|| -> Result<(), DnsError> {
                    let zone = opsctl_dns::find_zone(acc, domain)?;
                    let record = find_record(acc, &zone, record_id)?;
                    let update = opsctl_dns::record_update(
                        acc,
                        host.as_deref(),
                        r#type.as_deref(),
                        value.as_deref(),
                        *ttl,
                        line.as_deref(),
                        *proxied,
                        remark.as_deref(),
                    )?;
                    opsctl_dns::update_record(acc, &zone, &record, &update)?;
                    Ok(())
                })();
                report_write(config, &acc.name, "update", result, &mut had_error);
            }
            finish(had_error)
        }
        DnsRecordSubcommands::Delete {
            record_id,
            domain,
            account,
            dry_run,
            yes,
        } => {
            let accounts = selected_accounts(&config.dns, account.as_deref())?;
            let mut targets = Vec::new();
            for acc in accounts {
                match opsctl_dns::find_zone(acc, domain).and_then(|zone| {
                    let record = find_record(acc, &zone, record_id)?;
                    Ok((acc, zone, record))
                }) {
                    Ok(target) => targets.push(target),
                    Err(e) => {
                        eprintln!("❌ [{}] 定位记录失败: {e}", acc.name);
                    }
                }
            }
            if targets.is_empty() {
                return Err(DnsError::NotFound(format!("未找到记录 {record_id}")));
            }
            if *dry_run {
                for (acc, zone, record) in &targets {
                    println!(
                        "DRY-RUN: 删除 [{}] {}/{} ({}) → {}",
                        acc.name, zone.name, record.host, record.rtype, record.value
                    );
                }
                return Ok(());
            }
            let details: Vec<String> = targets
                .iter()
                .map(|(acc, zone, record)| {
                    format!(
                        "[{}] {}/{} ({}) → {}",
                        acc.name, zone.name, record.host, record.rtype, record.value
                    )
                })
                .collect();
            if !confirm_delete(&details, *yes) {
                println!("已取消");
                return Ok(());
            }
            let mut had_error = false;
            for (acc, zone, record) in targets {
                let result = opsctl_dns::delete_record(acc, &zone, &record.provider_record_id);
                report_write(config, &acc.name, "delete", result, &mut had_error);
            }
            finish(had_error)
        }
        DnsRecordSubcommands::Enable {
            record_id,
            domain,
            account,
            dry_run,
        }
        | DnsRecordSubcommands::Disable {
            record_id,
            domain,
            account,
            dry_run,
        } => {
            let enabled = matches!(sub, DnsRecordSubcommands::Enable { .. });
            let accounts = selected_accounts(&config.dns, account.as_deref())?;
            if *dry_run {
                println!(
                    "DRY-RUN: 在 {} 个账户下将记录 {record_id} 设为 {}",
                    accounts.len(),
                    if enabled { "启用" } else { "停用" }
                );
                return Ok(());
            }
            let mut had_error = false;
            for acc in accounts {
                let result = opsctl_dns::find_zone(acc, domain)
                    .and_then(|zone| opsctl_dns::set_record_status(acc, &zone, record_id, enabled));
                match result {
                    Err(DnsError::Unsupported(msg)) => println!("⚠️  [{}] {msg}", acc.name),
                    other => report_write(
                        config,
                        &acc.name,
                        if enabled { "enable" } else { "disable" },
                        other,
                        &mut had_error,
                    ),
                }
            }
            finish(had_error)
        }
    }
}

fn list_records(
    config: &GlobalConfig,
    domain: &str,
    account: Option<&str>,
    rtype: Option<&str>,
    keyword: Option<&str>,
    json: bool,
) -> Result<(), DnsError> {
    let accounts = selected_accounts(&config.dns, account)?;
    let mut rows: Vec<(String, String, ProviderRecord)> = Vec::new();
    let mut had_error = false;
    for acc in accounts {
        match opsctl_dns::find_zone(acc, domain)
            .and_then(|zone| opsctl_dns::list_records(acc, &zone))
        {
            Ok(records) => {
                for record in records {
                    let type_ok = rtype
                        .map(|t| record.rtype.eq_ignore_ascii_case(t))
                        .unwrap_or(true);
                    let keyword_ok = keyword
                        .map(|k| {
                            let k = k.to_lowercase();
                            record.host.to_lowercase().contains(&k)
                                || record.value.to_lowercase().contains(&k)
                                || record
                                    .remark
                                    .as_deref()
                                    .unwrap_or("")
                                    .to_lowercase()
                                    .contains(&k)
                        })
                        .unwrap_or(true);
                    if type_ok && keyword_ok {
                        rows.push((acc.name.clone(), domain.to_string(), record));
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ [{}] 拉取记录失败: {e}", acc.name);
                had_error = true;
            }
        }
    }

    if json {
        let mut grouped: std::collections::BTreeMap<String, Vec<ProviderRecord>> =
            Default::default();
        for (account_name, domain_name, record) in &rows {
            grouped
                .entry(format!("{account_name}/{domain_name}"))
                .or_default()
                .push(record.clone());
        }
        let mut output = serde_json::Map::new();
        for (key, records) in grouped {
            let (account_name, domain_name) = key.split_once('/').unwrap_or((&key, ""));
            let items: serde_json::Value =
                serde_json::from_str(&format_records_json(account_name, domain_name, &records)?)
                    .map_err(|e| DnsError::Api {
                        code: "JsonSerialize".into(),
                        message: format!("JSON 输出序列化失败: {e}"),
                    })?;
            output.insert(key, items);
        }
        println!("{}", json_pretty(&output)?);
    } else {
        println!(
            "{:<18} {:<20} {:<8} {:<32} {:<6} {:<10} 备注",
            "账户", "主机", "类型", "记录值", "TTL", "状态"
        );
        println!("{}", "-".repeat(120));
        for (account_name, _, record) in &rows {
            println!(
                "{:<18} {:<20} {:<8} {:<32} {:<6} {:<10} {}",
                account_name,
                record.host,
                record.rtype,
                record.value,
                record.ttl,
                if record.enabled { "启用" } else { "停用" },
                record.remark.as_deref().unwrap_or("")
            );
        }
    }
    finish(had_error)
}

fn find_record(
    acc: &DnsAccount,
    zone: &ProviderZone,
    record_id: &str,
) -> Result<ProviderRecord, DnsError> {
    opsctl_dns::list_records(acc, zone)?
        .into_iter()
        .find(|r| r.provider_record_id == record_id)
        .ok_or_else(|| DnsError::NotFound(format!("账户 {} 下未找到记录 {record_id}", acc.name)))
}

fn report_write<T>(
    config: &GlobalConfig,
    account: &str,
    action: &str,
    result: Result<T, DnsError>,
    had_error: &mut bool,
) {
    match result {
        Ok(_) => {
            println!("✅ [{}] {} 成功", account, action);
            notify(
                config,
                true,
                action,
                &format!("账户 {account} {action} 成功"),
            );
        }
        Err(e) => {
            eprintln!("❌ [{}] {} 失败: {e}", account, action);
            *had_error = true;
            notify(
                config,
                false,
                action,
                &format!("账户 {account} {action} 失败: {e}"),
            );
        }
    }
}

fn notify(config: &GlobalConfig, success: bool, action: &str, detail: &str) {
    let should = match config.dns.notify {
        DnsNotifyLevel::None => false,
        DnsNotifyLevel::Failure => !success,
        DnsNotifyLevel::All => true,
    };
    if !should {
        return;
    }
    let notifier = opsctl_notify::NotifyManager::from_config(&config.notify.webhooks);
    if !notifier.is_empty() {
        let title = format!("opsctl dns: {action}");
        let message = format!("{title}\n{detail}");
        let _ = notifier.send_all(&title, &message);
    }
}

fn confirm_delete(details: &[String], yes: bool) -> bool {
    if yes {
        return true;
    }
    println!("即将删除以下 DNS 记录：");
    for line in details {
        println!("  - {line}");
    }
    if !io::stdin().is_terminal() {
        eprintln!("非交互终端：请显式添加 --yes 确认删除");
        return false;
    }
    print!("输入 yes 确认删除: ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    answer.trim().eq_ignore_ascii_case("yes")
}

fn finish(had_error: bool) -> Result<(), DnsError> {
    if had_error {
        Err(DnsError::Api {
            code: "PartialFailure".into(),
            message: "部分操作失败".into(),
        })
    } else {
        Ok(())
    }
}

fn json_pretty<T: serde::Serialize>(value: &T) -> Result<String, DnsError> {
    serde_json::to_string_pretty(value).map_err(|e| DnsError::Api {
        code: "JsonSerialize".into(),
        message: format!("JSON 输出序列化失败: {e}"),
    })
}

pub(crate) fn format_accounts_table(accounts: &[DnsAccount]) -> String {
    let mut out = String::from("账户名                平台\n");
    out.push_str(&"-".repeat(40));
    out.push('\n');
    for acc in accounts {
        out.push_str(&format!("{:<20} {}\n", acc.name, acc.provider.as_str()));
    }
    out
}

pub(crate) fn format_records_json(
    account: &str,
    domain: &str,
    records: &[ProviderRecord],
) -> Result<String, DnsError> {
    let items: Vec<serde_json::Value> = records
        .iter()
        .map(|r| {
            serde_json::json!({
                "account": account,
                "domain": domain,
                "id": r.provider_record_id,
                "host": r.host,
                "type": r.rtype,
                "value": r.value,
                "ttl": r.ttl,
                "line": r.line,
                "proxied": r.proxied,
                "status": if r.enabled { "enabled" } else { "disabled" },
                "remark": r.remark,
            })
        })
        .collect();
    json_pretty(&items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opsctl_core::config::{DnsAccount, DnsProviderKind};
    use opsctl_dns::ProviderRecord;

    fn account() -> DnsAccount {
        DnsAccount {
            name: "aliyun-prod".into(),
            provider: DnsProviderKind::Aliyun,
            access_key_id: Some("ak".into()),
            access_key_secret: Some("sk".into()),
            secret_id: None,
            secret_key: None,
            api_token: None,
        }
    }

    #[test]
    fn accounts_table_contains_name_and_provider() {
        let table = format_accounts_table(&[account()]);
        assert!(table.contains("aliyun-prod"));
        assert!(table.contains("aliyun"));
    }

    #[test]
    fn records_json_contains_record_id_and_fields() {
        let record = ProviderRecord {
            provider_record_id: "rec-1".into(),
            host: "www".into(),
            rtype: "A".into(),
            value: "1.2.3.4".into(),
            ttl: 600,
            line: Some("default".into()),
            proxied: None,
            enabled: true,
            remark: Some("web".into()),
        };
        let json = format_records_json("aliyun-prod", "example.com", &[record]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value[0]["account"], "aliyun-prod");
        assert_eq!(value[0]["domain"], "example.com");
        assert_eq!(value[0]["id"], "rec-1");
        assert_eq!(value[0]["value"], "1.2.3.4");
    }
}

#[cfg(test)]
mod json_error_tests {
    use super::*;

    struct Failing;
    impl serde::Serialize for Failing {
        fn serialize<S: serde::Serializer>(&self, _s: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("boom"))
        }
    }

    #[test]
    fn json_pretty_returns_error_instead_of_empty_string() {
        let result = json_pretty(&Failing);
        assert!(result.is_err(), "序列化失败必须返回错误而不是空字符串");
    }

    #[test]
    fn json_pretty_serializes_normally() {
        let value = serde_json::json!({"a": 1});
        assert!(json_pretty(&value).unwrap().contains("\"a\""));
    }
}
