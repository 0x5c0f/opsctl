# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Security

- `zip` 2.4.2 → 9.0.5，修复 6 个 RUSTSEC 漏洞（SUID/SGID 保留、符号链接逃逸、AES 未校验、解压中止等）
- `rustls` / `rustls-webpki` / `webpki-roots` 升级到已修补版本，修复 6 个 TLS 相关 RUSTSEC 漏洞
- ZIP 解压改用 `enclosed_name()`，拒绝绝对路径与逃逸目标目录的条目
- `cargo audit` 清零，并加入 CI 门禁

### Fixed

- 日志改为写入 stderr，不再污染 `--json` 等 stdout 输出
- 无效 `--log-level` 不再生成 `target=trace` 指令而意外屏蔽其他日志
- `--json` 序列化失败时返回错误，不再打印空行静默降级
- handler 不再直接 `std::process::exit`，错误统一冒泡到 `main`（19 处 → 2 处）
- `main.rs` 收敛到 29 行，分发逻辑移入 `cli::dispatch`

### Added

- `rust-toolchain.toml` 固定工具链 1.96.0，本地与 CI 一致
- CI 新增 `cargo fmt --all --check` 与 `cargo audit` 门禁
- extractor / logging / handlers 新增测试（含 ZIP 逃逸、日志级别校验、配置加载）

## [0.3.0-rc.2] - 2026-11-17

### Fixed

- `dynamic` 部署改为覆盖合并，保留目标目录中未被本次 ZIP 覆盖的文件
- 回滚仅允许 `static` 项目，避免错误地将 dynamic/docker 目录替换为软链接
- 重启命令检查增加危险元字符、安全白名单与黑名单兜底
- `chown` 限制在 `/data` 目录下执行，避免误改系统目录权限
- certd token 固定为官方字段顺序
- pfx/der/jks 按官方 Base64 返回正确解码落盘，p7b 按文本/PEM 写入
- 证书主域名优先从 `detail.domains` 选择非通配符域名，并清洗目录名
- 证书获取、保存、重启命令失败均发送通知

### Changed

- README 去掉硬编码版本号，避免每次发版都需要修改文档

## [0.3.0-rc.1] - 2026-11-17

### Added

- DNS 多平台解析管理：阿里云、腾讯云 DNSPod（国内站）、Cloudflare
- Provider 抽象、统一错误类型、签名实现和记录字段校验
- DNS 账户验证、域名/记录查询、记录增删改查与启停
- DNS 写操作 `--dry-run`、删除交互确认 / `--yes`
- DNS 通知级别 `none | failure | all`
- CLI 全量短选项与详细帮助文本
- `config.example.toml` 配置模板
- musl 静态构建与 GitHub Actions tag 发布流程

### Fixed

- 证书测试改为内嵌 PEM，CI 不再依赖 `/tmp/test_cert.pem`
- 移除 `opsctl-cert` 对 `opsctl-notify` 的未使用依赖

### Changed

- 仓库整理为标准化 Rust workspace 结构，crate 上移到根目录
- 全局参数统一为根命令位置：`opsctl -c FILE dns ...`
- README 重写为正式项目文档

### Removed

- 设计阶段遗留 issue/handoff/superpowers 文档
- 历史 PRD/ADR/审计/术语表文档，保留精简版 `docs/ARCHITECTURE.md`
- 仓库内的临时构建产物和非项目配置
