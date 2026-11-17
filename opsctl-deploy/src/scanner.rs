use std::path::Path;

/// 扫描 upload_dir 下的 .zip 文件，匹配项目名
///
/// 匹配规则：扫描 upload_dir 下所有 .zip 文件，返回文件名列表。
/// 如果指定了 project_name，只返回包含项目名的 zip 文件。
pub fn scan_uploads(upload_dir: &Path, project_name: &str) -> Result<Vec<String>, crate::DeployError> {
    if !upload_dir.exists() {
        tracing::debug!("上传目录不存在: {:?}", upload_dir);
        return Ok(Vec::new());
    }

    let mut matches = Vec::new();
    for entry in std::fs::read_dir(upload_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "zip")
            && let Some(name) = path.file_stem().and_then(|n| n.to_str())
                && name.starts_with(project_name)
                    && let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        matches.push(file_name.to_string());
                    }
    }

    // 按文件名排序，取最新的
    matches.sort();
    matches.reverse();

    Ok(matches)
}

/// 获取 upload_dir 下第一个匹配的 zip 文件完整路径
pub fn find_first_upload(upload_dir: &Path, project_name: &str) -> Result<Option<std::path::PathBuf>, crate::DeployError> {
    let files = scan_uploads(upload_dir, project_name)?;
    if let Some(file_name) = files.first() {
        Ok(Some(upload_dir.join(file_name)))
    } else {
        Ok(None)
    }
}
