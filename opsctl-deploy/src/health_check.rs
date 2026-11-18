use sha2::{Digest, Sha256};
use std::time::Duration;

/// 健康检查：HTTP GET + SHA256 指纹，带重试
///
/// initial_delay: 首次等待秒数（默认 5s）
/// max_retries: 最大重试次数（默认 6）
/// retry_interval: 重试间隔秒数（默认 3s）
pub fn health_check_with_retry(
    url: &str,
    timeout_secs: u64,
    initial_delay: u64,
    max_retries: usize,
    retry_interval: u64,
) -> Result<(), crate::DeployError> {
    // 首次延迟：等待服务启动
    if initial_delay > 0 {
        tracing::info!("健康检查: 等待 {}s 后开始...", initial_delay);
        std::thread::sleep(Duration::from_secs(initial_delay));
    }

    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        match health_check_with_sha256(url, timeout_secs) {
            Ok(_) => {
                if attempt > 0 {
                    tracing::info!("健康检查通过 (重试 {}/{})", attempt, max_retries);
                }
                return Ok(());
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt < max_retries {
                    tracing::warn!(
                        "健康检查失败 ({}/{}): {}，{}s 后重试",
                        attempt + 1,
                        max_retries,
                        last_err,
                        retry_interval
                    );
                    std::thread::sleep(Duration::from_secs(retry_interval));
                }
            }
        }
    }

    Err(crate::DeployError::HealthCheck(format!(
        "健康检查失败 ({} 次重试后): {} — {}",
        max_retries, url, last_err
    )))
}

/// 单次健康检查
fn health_check_with_sha256(url: &str, timeout_secs: u64) -> Result<(), crate::DeployError> {
    let timeout = Duration::from_secs(timeout_secs);

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into();

    match agent.get(url).call() {
        Ok(resp) => {
            let status = resp.status();
            let status_code = status.as_u16();

            // 第一步：检查 HTTP 状态码
            if !(200..400).contains(&status_code) {
                return Err(crate::DeployError::HealthCheck(format!(
                    "HTTP {} — {}",
                    status_code, url
                )));
            }

            // 第二步：读取响应体并计算 SHA256
            let mut response_body = resp.into_body();
            let bytes = match response_body.read_to_vec() {
                Ok(b) => b,
                Err(e) => {
                    return Err(crate::DeployError::HealthCheck(format!(
                        "读取响应体失败 {}: {}",
                        url, e
                    )));
                }
            };

            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let _fingerprint = format!("{:x}", hasher.finalize());

            tracing::info!(
                "健康检查通过: {} (HTTP {}, SHA256 指纹已计算)",
                url,
                status_code
            );
            Ok(())
        }
        Err(ureq::Error::StatusCode(code)) => Err(crate::DeployError::HealthCheck(format!(
            "HTTP {} — {}",
            code, url
        ))),
        Err(e) => Err(crate::DeployError::HealthCheck(format!(
            "请求失败 {}: {}",
            url, e
        ))),
    }
}

/// 向后兼容：仅检查 HTTP 2xx/3xx 状态码
pub fn health_check_simple(url: &str, timeout_secs: u64) -> Result<(), crate::DeployError> {
    health_check_with_sha256(url, timeout_secs)
}
