use std::fs::File;
use std::path::Path;
use zip::ZipArchive;

/// 检查 ZIP 文件完整性
pub fn check_zip_integrity(zip_path: &Path) -> Result<(), crate::DeployError> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    // 尝试读取所有文件来验证完整性
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let _size = entry.size();

        // 尝试读取文件内容来验证完整性
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buffer).map_err(|e| {
            crate::DeployError::Zip(format!("ZIP 文件损坏 ({}): {e}", zip_path.display()))
        })?;
    }

    tracing::debug!(
        "ZIP 完整性检查通过: {:?} ({} 个文件)",
        zip_path,
        archive.len()
    );
    Ok(())
}

/// 解压 ZIP 到目标目录
///
/// 如果目标目录已存在，则先删除再重新解压（确保干净的版本目录）。
pub fn extract_zip(zip_path: &Path, target_dir: &Path) -> Result<(), crate::DeployError> {
    // 如果目标目录已存在，删除重建（保证版本纯净）
    if target_dir.exists() {
        std::fs::remove_dir_all(target_dir)?;
    }
    std::fs::create_dir_all(target_dir)?;

    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;

        // zip 9 的 enclosed_name() 拒绝绝对路径与逃逸出目标目录的条目
        let Some(relative) = entry.enclosed_name() else {
            tracing::warn!("跳过不安全条目: {:?}", entry.name().map(|n| n.to_string()));
            continue;
        };
        let safe_path = target_dir.join(&relative);
        if !safe_path.starts_with(target_dir) {
            tracing::warn!("跳过路径穿越条目: {}", relative.display());
            continue;
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&safe_path)?;
        } else {
            if let Some(parent) = safe_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut output = File::create(&safe_path)?;
            std::io::copy(&mut entry, &mut output)?;
        }
    }

    // 如果只有一个顶层目录，提层（strip wrapper directory）
    flatten_single_dir(target_dir)?;

    tracing::info!("ZIP 解压完成: {:?} → {:?}", zip_path, target_dir);
    Ok(())
}

/// 如果 target_dir 下只有一个子目录（且无文件），将该子目录内容提升一级
fn flatten_single_dir(target_dir: &Path) -> std::io::Result<()> {
    let entries: Vec<_> = std::fs::read_dir(target_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .collect();

    // 恰好一个条目且是目录 → 提层
    if entries.len() != 1 || !entries[0].path().is_dir() {
        return Ok(());
    }

    let wrapper = entries[0].path();
    let wrapper_name = wrapper.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "包装目录缺少文件名")
    })?;

    // 将 wrapper 内的所有条目移到 target_dir
    for item in std::fs::read_dir(&wrapper)? {
        let item = item?;
        let target = target_dir.join(item.file_name());
        std::fs::rename(item.path(), &target)?;
    }
    std::fs::remove_dir(&wrapper)?;

    tracing::debug!(
        "ZIP 提层: 移除了包装目录 '{}'",
        wrapper_name.to_string_lossy()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "opsctl_zip_{name}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default();
        for (name, content) in entries {
            writer.start_file(*name, opts).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn flattens_single_top_level_directory() {
        let dir = temp_dir("flatten");
        let zip_path = dir.join("a.zip");
        write_zip(&zip_path, &[("app/index.html", "hello")]);

        let target = dir.join("out");
        extract_zip(&zip_path, &target).unwrap();
        assert_eq!(
            std::fs::read_to_string(target.join("index.html")).unwrap(),
            "hello"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keeps_layout_when_multiple_top_level_entries() {
        let dir = temp_dir("multi");
        let zip_path = dir.join("a.zip");
        write_zip(
            &zip_path,
            &[("app/index.html", "hello"), ("readme.txt", "hi")],
        );

        let target = dir.join("out");
        extract_zip(&zip_path, &target).unwrap();
        assert_eq!(
            std::fs::read_to_string(target.join("app/index.html")).unwrap(),
            "hello"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("readme.txt")).unwrap(),
            "hi"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_entries_escaping_target_directory() {
        let dir = temp_dir("escape");
        let zip_path = dir.join("evil.zip");
        write_zip(&zip_path, &[("../evil.txt", "pwned")]);

        let target = dir.join("out");
        extract_zip(&zip_path, &target).unwrap();

        assert!(!target.join("../evil.txt").exists());
        assert!(!dir.join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detects_corrupt_zip() {
        let dir = temp_dir("corrupt");
        let zip_path = dir.join("bad.zip");
        std::fs::write(&zip_path, b"not a zip").unwrap();
        assert!(check_zip_integrity(&zip_path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
