use opsctl_core::GlobalConfig;

/// 处理 `opsctl cert update`
pub fn handle_update(global_cfg: &GlobalConfig, cert_id: Option<u32>) {
    if global_cfg.cert.certd_base_url.is_empty() {
        eprintln!("错误: 未配置 certd (请在 ~/.opsctl/config.toml 中设置 [cert] 段)");
        std::process::exit(1);
    }

    let mut cert_config = global_cfg.cert.clone();
    if let Some(id) = cert_id {
        cert_config.cert_ids = vec![id];
    }

    match opsctl_cert::update_certs(&cert_config) {
        Ok(results) => {
            let mut changed_lines = Vec::new();
            let mut failed_lines = Vec::new();

            for r in &results {
                if !r.success {
                    eprintln!("❌ [{}] {}", r.domain, r.message);
                    failed_lines.push(format!("❌ {} — {}", r.domain, r.message));
                    continue;
                }
                if r.changed {
                    println!("✅ [{}] {}", r.domain, r.message);
                    changed_lines.push(format!("✅ {} — {}", r.domain, r.message));
                    if let Some(ref cmd) = global_cfg.cert.domain_reload_commands.get(&r.domain) {
                        tracing::info!("执行重启命令: {} (domain={})", cmd, r.domain);
                        if let Err(e) = execute_restart_command(cmd) {
                            tracing::error!("重启命令执行失败 ({}): {}", r.domain, e);
                            eprintln!("  ⚠️ 重启命令失败: {}", e);
                            failed_lines.push(format!("⚠️ {} 重启命令失败 — {}", r.domain, e));
                        }
                    }
                } else {
                    println!("⏭️  [{}] {}", r.domain, r.message);
                }
            }

            if !changed_lines.is_empty() || !failed_lines.is_empty() {
                let title = cert_update_notification_title(
                    changed_lines.len(),
                    failed_lines.len(),
                );
                let mut lines = changed_lines;
                lines.extend(failed_lines);
                send_cert_notification(global_cfg, &title, &lines);
            }
        }
        Err(e) => {
            eprintln!("❌ 证书更新失败: {e}");
            send_cert_notification(
                global_cfg,
                "cert update failed",
                &[format!("❌ {e}")],
            );
            std::process::exit(1);
        }
    }
}

pub(crate) fn cert_update_notification_title(changed: usize, failed: usize) -> String {
    format!("cert update (changed={changed}, failed={failed})")
}

fn send_cert_notification(global_cfg: &GlobalConfig, title: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let notifier = opsctl_notify::NotifyManager::from_config(&global_cfg.notify.webhooks);
    if !notifier.is_empty() {
        notifier.send_all(title, &lines.join("\n"));
    }
}

fn execute_restart_command(command: &str) -> Result<(), String> {
    opsctl_deploy::safety::execute_command(command)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// 处理 `opsctl cert list`
pub fn handle_list(global_cfg: &GlobalConfig) {
    match opsctl_cert::list_certs(&global_cfg.cert) {
        Ok(entries) => {
            if entries.is_empty() {
                println!("暂无本地证书");
                return;
            }
            println!("{:<6}  {:<30}  {:<19}  状态", "ID", "域名", "过期时间");
            println!("{}", "-".repeat(72));
            for e in &entries {
                let id = e.cert_id.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
                let status = match e.remaining_days {
                    Some(days) if days > 30 => format!("✅ {} 天", days),
                    Some(days) if days > 0  => format!("⚠️ {} 天", days),
                    Some(_) => "❌ 已过期".to_string(),
                    None    => "❌ 已过期".to_string(),
                };
                println!("{:<6}  {:<30}  {:<19}  {}", id, e.domain, e.not_after, status);
            }
        }
        Err(e) => {
            eprintln!("错误: {e}");
            std::process::exit(1);
        }
    }
}

/// 处理 `opsctl cert check`
pub fn handle_check(global_cfg: &GlobalConfig, warn_days: u32) {
    let mut cert_config = global_cfg.cert.clone();
    cert_config.check.warn_days = warn_days;

    match opsctl_cert::check_certs(&cert_config) {
        Ok(result) => {
            let mut has_issues = false;
            if !result.expired.is_empty() {
                has_issues = true;
                println!("❌ {} 个证书已过期:", result.expired.len());
                for e in &result.expired {
                    println!("  - {} (过期于 {})", e.domain, e.not_after);
                }
            }
            if !result.expiring.is_empty() {
                has_issues = true;
                println!("⚠️ {} 个证书将在 {} 天内过期:", result.expiring.len(), warn_days);
                for e in &result.expiring {
                    let days = e.remaining_days.unwrap_or(0);
                    println!("  - {} (还剩 {} 天)", e.domain, days);
                }
            }
            if !has_issues {
                println!("✅ 所有证书正常");
            }
            if cert_config.check.notification && has_issues {
                let notifier = opsctl_notify::NotifyManager::from_config(&global_cfg.notify.webhooks);
                if !notifier.is_empty() {
                    let mut lines = Vec::new();
                    for e in &result.expired {
                        lines.push(format!("❌ {} 已过期", e.domain));
                    }
                    for e in &result.expiring {
                        let days = e.remaining_days.unwrap_or(0);
                        lines.push(format!("⚠️ {} 还剩 {} 天", e.domain, days));
                    }
                    notifier.send_all("cert check", &lines.join("\n"));
                }
            }
        }
        Err(e) => {
            eprintln!("错误: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_title_includes_changed_and_failed_counts() {
        assert_eq!(
            cert_update_notification_title(2, 1),
            "cert update (changed=2, failed=1)"
        );
    }
}
