# opsctl

[![Release](https://github.com/0x5c0f/opsctl/actions/workflows/release.yml/badge.svg)](https://github.com/0x5c0f/opsctl/actions/workflows/release.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

`opsctl` 是一个用 Rust 编写的单文件运维 CLI，把部署、证书管理和多平台 DNS 解析管理统一到一个二进制中，面向 Linux 服务器运维场景，零运行时依赖。


## 特性

- **部署管理**：集中式多项目配置、持久化锁、ZIP 扫描、三种部署模式、版本历史与回滚
- **证书管理**：从 certd 拉取证书、x509 解析、8 种格式输出、过期检查与变更通知
- **DNS 管理**：集中管理阿里云、腾讯云 DNSPod（国内站）、Cloudflare 的解析记录
- **通知**：企业微信 / 钉钉 / 飞书 / Slack webhook，支持按事件级别发送
- **单文件交付**：musl 静态编译，可直接放到任意 Linux x86_64 机器运行

## 安装

### 从 Release 安装

从 [Releases](https://github.com/0x5c0f/opsctl/releases) 下载静态二进制：

```bash
curl -L -o opsctl <release-asset-url>
chmod +x opsctl
sudo mv opsctl /usr/local/bin/opsctl
opsctl --version
```

### 从源码构建

```bash
git clone https://github.com/0x5c0f/opsctl.git
cd opsctl
cargo build --release
sudo cp target/release/opsctl /usr/local/bin/
```

静态构建：

```bash
make release-static
# 产物：target/x86_64-unknown-linux-musl/release/opsctl
```

## 配置

复制仓库根目录的 [`config.example.toml`](config.example.toml)：

```bash
mkdir -p ~/.opsctl
cp config.example.toml ~/.opsctl/config.toml
chmod 600 ~/.opsctl/config.toml
```

也可以用工具生成模板：

```bash
opsctl init
```

> 全局参数必须写在子命令之前，例如：`opsctl -c ~/.opsctl/config.toml dns ...`

## 命令概览

```text
opsctl deploy   部署管理
opsctl cert     SSL 证书管理
opsctl dns      多平台 DNS 解析管理
opsctl init     生成配置模板
```

所有命令支持 `-h / --help` 查看详细说明。

### 部署

```bash
opsctl deploy run [-p myapp]
opsctl deploy daemon [-i 10]
opsctl deploy history [-p myapp] [-l 20]
opsctl deploy rollback <version_id>
opsctl deploy init [-p myapp]
```

部署采用共享目录模型：

```text
/data/autoDeploy/
├── uploads/   开发上传 ZIP 与持久化锁
├── ready/     解压后的历史版本
└── enabled/   {project}_latest 软链接与 .history.toml
```

锁文件为 `uploads/.{project}.lock`，部署完成后不自动释放；手动删除锁才会触发下一次部署。

三种部署模式：

- `static`：切换 `{project}_latest` 软链接，应用目录始终是 `ready/{version}`，支持回滚
- `dynamic`：把 ZIP 内容覆盖合并到 `enabled/{project}_latest`，保留目标目录中不属于本次 ZIP 的文件，适合只上传部分 DLL/配置的项目
- `docker`：不解压 ZIP，只执行重启命令与健康检查

> `dynamic` 与 `docker` 不提供回滚；`opsctl deploy rollback` 仅接受 `static` 项目。命令执行前会进行误操作防护检查，`chown` 只允许在 `/data` 下递归执行。

### 证书

```bash
opsctl cert update [-i 1]
opsctl cert list
opsctl cert check [-w 30]
```

默认写入适合 Nginx 的 PEM 文件：`fullchain.cer`、`{domain}.key`、`{domain}.ic`、`{domain}.cer`、`{domain}.one`。

可选格式：`pfx / der / jks / p7b`。`pfx/der/jks` 由服务端返回 Base64 内容，opsctl 解码后写入二进制文件；`p7b` 按服务端返回的文本/PEM 内容写入。默认不下载这些格式。`pfx` 专为后续 Windows/IIS 场景预留。

### DNS

第一版支持 `aliyun`、`dnspod`（国内站）、`cloudflare`，纯实时调用平台 API，无本地缓存与对账。

```bash
opsctl dns account list
opsctl dns account verify [-a aliyun-prod]

opsctl dns domain list [-a cf-prod] [-k example]

opsctl dns record list -d example.com [-t A] [-k www]
opsctl dns record create -d example.com -H www -t A -v 1.2.3.4 [-T 600] [-l default] [-p true] [-r web] [-a PROFILE] [-n]
opsctl dns record update <record-id> -d example.com [-v 2.3.4.5] [-n]
opsctl dns record delete <record-id> -d example.com [-y] [-n]
opsctl dns record enable|disable <record-id> -d example.com [-n]
```

DNS 安全约定：

- 写操作必须指定 `--domain`
- `--account` 不指定时遍历所有已配置账户
- `delete` 在 TTY 下交互确认，非 TTY 脚本必须显式 `--yes`
- 所有写操作支持 `--dry-run`
- Cloudflare 不支持记录启停，会返回明确的 `Unsupported`

## 架构

Workspace 结构：

```text
opsctl/
├── opsctl/           # CLI binary：参数解析、handlers
├── opsctl-core/      # 配置、日志、错误类型
├── opsctl-deploy/    # 部署能力
├── opsctl-cert/      # 证书能力
├── opsctl-notify/    # 通知能力
├── opsctl-dns/       # DNS 平台适配器与操作编排
├── docs/             # 架构说明与审计记录
├── .github/workflows/
│   ├── ci.yml
│   └── release.yml
├── config.example.toml
└── Makefile
```

每个顶层 `opsctl-*` 目录都是独立 crate，对应一项可拔插能力。依赖方向单向，无循环依赖：

```text
opsctl ──► opsctl-core
  ├──────► opsctl-deploy ──► opsctl-core
  ├──────► opsctl-cert ────► opsctl-core / opsctl-notify
  ├──────► opsctl-dns ─────► opsctl-core
  └──────► opsctl-notify ──► opsctl-core
```

`opsctl-core` 是叶节点；`opsctl` binary 只做参数解析与分发，业务逻辑在各 lib crate 中。如何移除/新增一个能力，见 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)。

## 开发

```bash
cargo fmt --all --check
cargo check --locked
cargo test --locked -- --test-threads=1
cargo clippy --locked
cargo audit
make release-static
```

工具链由 `rust-toolchain.toml` 固定，CI 与本地保持一致。

## Release 规则

发布由 tag 推送触发，只有符合以下规范的 tag 会构建并发布：

| Tag 格式 | Release 类型 |
|---|---|
| `vX.Y.Z-rc.N` | 预发布（Prerelease） |
| `vX.Y.Z` | 正式发布 |
| 其他 | 忽略，不构建不发布 |

示例：

```bash
git tag v0.3.0-rc.1
git push origin v0.3.0-rc.1
```

构建产物：

```text
opsctl-x86_64-unknown-linux-musl
opsctl-x86_64-unknown-linux-musl.sha256
```

## License

Apache-2.0，详见 [LICENSE](LICENSE)。
