# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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

### Changed

- 仓库整理为标准化 Rust workspace 结构，crate 上移到根目录
- 全局参数统一为根命令位置：`opsctl -c FILE dns ...`
- README 重写为正式项目文档

### Removed

- 设计阶段遗留 issue/handoff/superpowers 文档
- 历史 PRD/ADR/审计/术语表文档，保留精简版 `docs/ARCHITECTURE.md`
- 仓库内的临时构建产物和非项目配置
