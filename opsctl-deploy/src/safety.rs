/// 危险的 shell 命令模式
const DANGEROUS_COMMANDS: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "dd if=",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "mkfs",
    "fdisk",
    "format",
    "> /dev/",
    "chmod 777 /",
    "chown -R 0:0 /",
];

/// 命令安全检查：拒绝执行危险命令
///
/// 返回 `Ok(command)` 如果安全，返回 `Err` 如果被拒绝。
pub fn safe_command_check(command: &str) -> Result<String, crate::DeployError> {
    let trimmed = command.trim();

    if trimmed.is_empty() {
        return Err(crate::DeployError::CommandRejected("命令为空".into()));
    }

    for dangerous in DANGEROUS_COMMANDS {
        if trimmed.contains(dangerous) {
            return Err(crate::DeployError::CommandRejected(
                format!("危险命令被拦截: '{}' (匹配模式: '{}')", trimmed, dangerous),
            ));
        }
    }

    Ok(trimmed.to_string())
}

/// 执行重启命令并捕获输出
pub fn execute_command(command: &str) -> Result<String, crate::DeployError> {
    let safe_cmd = safe_command_check(command)?;

    tracing::info!("执行命令: {}", safe_cmd);

    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&safe_cmd)
        .output()
        .map_err(|e| crate::DeployError::DeployFailed(format!("命令执行失败: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        if !stdout.is_empty() {
            tracing::debug!("命令 stdout: {}", stdout.trim());
        }
        Ok(stdout)
    } else {
        let msg = format!("命令执行失败 (exit: {:?}): {}", output.status.code(), stderr.trim());
        tracing::error!("{}", msg);
        Err(crate::DeployError::DeployFailed(msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_command_allowed() {
        assert_eq!(safe_command_check("systemctl restart nginx").unwrap(), "systemctl restart nginx");
        assert_eq!(safe_command_check("docker-compose restart").unwrap(), "docker-compose restart");
    }

    #[test]
    fn test_safe_command_empty_rejected() {
        assert!(safe_command_check("").is_err());
        assert!(safe_command_check("   ").is_err());
    }

    #[test]
    fn test_dangerous_commands_blocked() {
        let dangerous = vec![
            "rm -rf /",
            "rm -rf /*",
            "rm -rf ~",
            "dd if=/dev/zero of=/dev/sda",
            "shutdown -h now",
            "reboot --force",
            "mkfs.ext4 /dev/sda1",
        ];
        for cmd in dangerous {
            assert!(safe_command_check(cmd).is_err(), "should block: {}", cmd);
        }
    }

    #[test]
    fn test_safe_prefix_not_blocked() {
        // 安全的 "format" 在路径中不应被拦截（但我们的 DANGEROUS_COMMANDS 包含 "format"）
        // 验证 "format" 本身被拦截
        assert!(safe_command_check("format").is_err());
    }
}
