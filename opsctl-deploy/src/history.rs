use crate::DeployError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 版本历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRecord {
    pub id: String,
    pub project: String,
    pub timestamp: u64,
    pub ready_dir: String,
    pub status: VersionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "rolled_back")]
    RolledBack,
    #[serde(rename = "cleaned")]
    Cleaned,
}

// ─── 内部 TOML 包装结构 ──────────────────────────────────────
// 使用 wrapper 确保序列化/反序列化格式一致（TOML 不支持裸顶层数组）

#[derive(Debug, Serialize, Deserialize)]
struct HistoryFile {
    version_record: Vec<VersionRecord>,
}

/// 加载版本历史
pub fn load_history(basedir: &Path) -> Result<Vec<VersionRecord>, DeployError> {
    let path = basedir.join("enabled").join(".history.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| DeployError::Config(format!("读取历史记录失败: {e}")))?;
    let wrapper: HistoryFile = toml::from_str(&content)
        .map_err(|e| DeployError::Config(format!("解析历史记录失败: {e}")))?;
    Ok(wrapper.version_record)
}

/// 保存版本历史
pub fn save_history(basedir: &Path, records: &[VersionRecord]) -> Result<(), DeployError> {
    let path = basedir.join("enabled").join(".history.toml");
    // 确保 enabled 目录存在
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DeployError::Config(format!("创建 enabled 目录失败: {e}")))?;
    }
    let wrapper = HistoryFile {
        version_record: records.to_vec(),
    };
    let content = toml::to_string_pretty(&wrapper)
        .map_err(|e| DeployError::Config(format!("序列化历史记录失败: {e}")))?;
    std::fs::write(&path, content)?;
    Ok(())
}

/// 记录一次新部署（追加到历史记录中）
pub fn record_deployment(
    basedir: &Path,
    project: &str,
    version_id: &str,
    ready_dir: &Path,
) -> Result<(), DeployError> {
    let mut records = load_history(basedir)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 将之前的 active 版本标记为 rolled_back
    for r in &mut records {
        if r.project == project && matches!(r.status, VersionStatus::Active) {
            r.status = VersionStatus::RolledBack;
        }
    }

    records.push(VersionRecord {
        id: version_id.to_string(),
        project: project.to_string(),
        timestamp,
        ready_dir: ready_dir.to_string_lossy().to_string(),
        status: VersionStatus::Active,
    });

    save_history(basedir, &records)?;
    tracing::info!("已记录部署版本: {} ({})", version_id, project);
    Ok(())
}

/// 获取指定项目的历史记录（按时间倒序）
pub fn get_project_history(
    basedir: &Path,
    project: &str,
    limit: usize,
) -> Result<Vec<VersionRecord>, DeployError> {
    let records = load_history(basedir)?;
    let mut project_records: Vec<VersionRecord> = records
        .into_iter()
        .filter(|r| r.project == project)
        .collect();
    project_records.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
    project_records.truncate(limit);
    Ok(project_records)
}

/// 回滚到指定版本
pub fn rollback_to(basedir: &Path, version_id: &str) -> Result<(), DeployError> {
    let records = load_history(basedir)?;

    let target = records
        .iter()
        .find(|r| r.id == version_id)
        .ok_or_else(|| DeployError::DeployFailed(format!("未找到版本: {}", version_id)))?;

    let ready_dir = PathBuf::from(&target.ready_dir);
    if !ready_dir.exists() {
        return Err(DeployError::DeployFailed(format!(
            "版本目录不存在: {:?}",
            ready_dir
        )));
    }

    let enabled_dir = basedir.join("enabled");
    let enabled_link = enabled_dir.join(format!("{}_latest", target.project));

    // 删除当前软链接
    if enabled_link.is_symlink() {
        std::fs::remove_file(&enabled_link)?;
    } else if enabled_link.is_dir() {
        std::fs::remove_dir_all(&enabled_link)?;
    }

    // 创建新软链接指向目标版本
    #[cfg(unix)]
    std::os::unix::fs::symlink(&ready_dir, &enabled_link)?;
    #[cfg(not(unix))]
    crate::deploy::copy_dir(&ready_dir, &enabled_link)?;

    // 更新历史记录 — 收集索引独立处理，避免借用人问题
    let target_project = target.project.clone();
    let updated_records: Vec<VersionRecord> = records
        .into_iter()
        .map(|mut r| {
            if r.project == target_project && matches!(r.status, VersionStatus::Active) {
                r.status = VersionStatus::RolledBack;
            }
            if r.id == version_id {
                r.status = VersionStatus::Active;
            }
            r
        })
        .collect();

    save_history(basedir, &updated_records)?;
    tracing::info!("已回滚到版本: {}", version_id);
    Ok(())
}

/// 自动清理旧版本（保留最近的 N 个版本）
pub fn cleanup_old_versions(basedir: &Path, project: &str, keep: usize) -> Result<(), DeployError> {
    let records = load_history(basedir)?;

    // 收集该项目的索引和引用，按时间排序
    let mut project_indices: Vec<(usize, &VersionRecord)> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.project == project)
        .collect();
    project_indices.sort_by_key(|(_, b)| std::cmp::Reverse(b.timestamp));

    // 确定需清理的索引（超出 keep 且不是 active 或已 cleaned）
    let to_clean: Vec<usize> = project_indices
        .iter()
        .enumerate()
        .filter(|(idx, (_, r))| {
            *idx >= keep
                && !matches!(r.status, VersionStatus::Active)
                && !matches!(r.status, VersionStatus::Cleaned)
        })
        .map(|(_, (i, _))| *i)
        .collect();

    if to_clean.is_empty() {
        return Ok(());
    }

    // 分开两步：先删目录，再更新记录
    for idx in &to_clean {
        let ready_path = PathBuf::from(&records[*idx].ready_dir);
        if ready_path.exists() {
            std::fs::remove_dir_all(&ready_path)
                .map_err(|e| DeployError::DeployFailed(format!("清理旧版本失败: {e}")))?;
            tracing::debug!("已清理旧版本目录: {:?}", ready_path);
        }
    }

    // 现在更新 records（重新加载避免借用冲突）
    let mut updated_records = load_history(basedir)?;
    for idx in &to_clean {
        if *idx < updated_records.len() {
            updated_records[*idx].status = VersionStatus::Cleaned;
        }
    }

    save_history(basedir, &updated_records)?;
    tracing::info!("已清理 {} 个旧版本 (保留 {})", to_clean.len(), keep);
    Ok(())
}

/// 列出版本历史（人类可读格式）
pub fn format_history(records: &[VersionRecord]) -> Vec<String> {
    records
        .iter()
        .map(|r| {
            let time_str = format_timestamp(r.timestamp);
            let status_str = match r.status {
                VersionStatus::Active => "✅ active",
                VersionStatus::RolledBack => "↩️ rolled_back",
                VersionStatus::Cleaned => "🗑️ cleaned",
            };
            format!("{:<25} {}  {}  {}", r.id, time_str, status_str, r.project)
        })
        .collect()
}

fn format_timestamp(ts: u64) -> String {
    let days = ts / 86400;
    let time_secs = ts % 86400;
    let hours = time_secs / 3600;
    let mins = (time_secs % 3600) / 60;
    let secs = time_secs % 60;
    let (year, month, day) = days_to_date(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hours, mins, secs
    )
}

fn days_to_date(days: u64) -> (u64, u64, u64) {
    let mut y = 1970i64;
    let mut d = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0u64;
    for (i, &md) in month_days.iter().enumerate() {
        if d < md as i64 {
            m = i as u64;
            break;
        }
        d -= md as i64;
    }
    (y as u64, m + 1, (d + 1) as u64)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_history_empty() {
        let lines = format_history(&[]);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_format_history_one_record() {
        let records = vec![VersionRecord {
            id: "test_v1".into(),
            project: "myapp".into(),
            timestamp: 1700000000,
            ready_dir: "/tmp/myapp_v1".into(),
            status: VersionStatus::Active,
        }];
        let lines = format_history(&records);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("test_v1"));
        assert!(lines[0].contains("✅"));
    }

    #[test]
    fn test_version_status_deserialize() {
        // 在结构体上下文中测试 enum 反序列化
        #[derive(Deserialize)]
        struct Wrapper {
            status: VersionStatus,
        }
        let w: Wrapper = toml::from_str("status = \"active\"").unwrap();
        assert!(matches!(w.status, VersionStatus::Active));

        let w: Wrapper = toml::from_str("status = \"rolled_back\"").unwrap();
        assert!(matches!(w.status, VersionStatus::RolledBack));

        let w: Wrapper = toml::from_str("status = \"cleaned\"").unwrap();
        assert!(matches!(w.status, VersionStatus::Cleaned));
    }

    #[test]
    fn test_save_and_load_history() {
        let dir = std::env::temp_dir().join("opsctl_test_history");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir.join("enabled")).unwrap();

        // 使用 save_history / record_deployment 写入，确保序列化格式一致
        let records = vec![
            VersionRecord {
                id: "v1".into(),
                project: "test".into(),
                timestamp: 1000,
                ready_dir: "/tmp/ready_v1".into(),
                status: VersionStatus::Active,
            },
            VersionRecord {
                id: "v2".into(),
                project: "test".into(),
                timestamp: 2000,
                ready_dir: "/tmp/ready_v2".into(),
                status: VersionStatus::RolledBack,
            },
        ];
        save_history(&dir, &records).unwrap();

        let loaded = load_history(&dir).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].id, "v1");
        assert!(matches!(loaded[0].status, VersionStatus::Active));
        assert_eq!(loaded[1].id, "v2");
        assert!(matches!(loaded[1].status, VersionStatus::RolledBack));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
