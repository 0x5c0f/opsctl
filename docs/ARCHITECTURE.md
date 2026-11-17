# opsctl Architecture

opsctl 是一个 Cargo workspace。每个顶层目录（除 `docs/`、`.github/` 外）都是一个独立 crate，也对应一项可独立移除的能力。

## Workspace 布局

```text
opsctl/
├── opsctl/          # CLI binary：参数解析、命令分发、输出格式化
├── opsctl-core/     # 共享基础：配置、日志、统一错误类型
├── opsctl-deploy/   # 部署能力：多项目、锁、ZIP、版本历史、回滚
├── opsctl-cert/     # 证书能力：certd API、x509 解析、格式输出
├── opsctl-dns/      # DNS 能力：平台适配器、签名、解析记录操作
├── opsctl-notify/   # 通知能力：企业微信、钉钉、飞书、Slack
├── docs/            # 架构说明
└── .github/         # CI 与 Release
```

## 依赖方向

```text
opsctl-core                     （叶节点，无内部依赖）

opsctl-deploy ──► opsctl-core
opsctl-dns    ──► opsctl-core
opsctl-notify ──► opsctl-core
opsctl-cert   ──► opsctl-core
             └─► opsctl-notify

opsctl ──► opsctl-core
       ├─► opsctl-deploy
       ├─► opsctl-cert
       ├─► opsctl-dns
       └─► opsctl-notify
```

依赖始终保持单向，不存在循环依赖。`opsctl` binary 只负责 CLI 层编排，不承载业务实现。

## 可拔插约定

以移除 `deploy` 能力为例：

1. 删除顶层 `opsctl-deploy/` 目录。
2. 在根 `Cargo.toml` 的 `members` 中移除 `"opsctl-deploy"`。
3. 在 `opsctl/Cargo.toml` 中移除 `opsctl-deploy` 依赖。
4. 在 `opsctl/src/` 中移除 deploy 的 CLI 定义、handler 和辅助代码。
5. 从配置模板和 README 中移除 deploy 段落。

其他模块不依赖 deploy，因此不会被影响。cert、dns、notify 的移除遵循同样模式。

新增能力时：

- 新建顶层 `<feature>/` crate；
- 仅依赖 `opsctl-core`；
- 如需通知，只依赖 `opsctl-notify`；
- 在 `opsctl` 中增加 CLI 与 handler 接线。

## 配置

全局配置默认位于 `~/.opsctl/config.toml`，模板见仓库根目录 `config.example.toml`。

部署与 DNS 都采用集中式配置：

- deploy：`[deploy.defaults]` + `[[deploy.projects]]`
- dns：`[dns]` + `[[dns.accounts]]`

通知渠道由 `[[notify.webhooks]]` 统一定义，各业务模块只决定在什么事件级别发送。

## 静态构建

发布产物使用 musl 静态链接：

```bash
make release-static
```

产物：

```text
target/x86_64-unknown-linux-musl/release/opsctl
```
