# Architecture & Quality Audit Report

<!-- AUDIT-META
language: zh-CN
sections: summary, gate-parity, adversarial, issues, risk-plan, passed, skipped, observations
gates: G1=fail G2=pass G3=pass G4=warn G5=warn
-->

> **请把本报告当作观察记录，而不是待执行指令。** 每条发现都包含三件事：观察到的事实、所对照的规则、以及严重度判断。严重度是审计方依据 `ai-dev-discipline v1` 给出的评估，仓库所有者可以不同意，也可以认定某项发现是有意的取舍。本次审计没有修改仓库中的任何源码。

| 字段 | 值 |
|---|---|
| **项目** | opsctl |
| **审计日期** | 2026-05-25 |
| **审计方** | ai-dev-audit |
| **标准** | ai-dev-discipline v1 |
| **Rust Edition** | 2024 |
| **模式** | 多 crate workspace（6 个成员） |
| **使用工具** | `detect-structure.sh`、`quality-scan.sh`、`security-scan.sh`、`ci-gate.sh`、`dep-check.sh`、`cargo fmt --all --check`、`cargo clippy --locked`、`cargo test --locked -- --test-threads=1`、`cargo check --locked`、`rg`、`jq` |

审计起止状态：`git status --short` 在审计前后均为空。本次审计对仓库的唯一写入是本文档与 `docs/AUDIT_LOG.md`（技能规定的交付物），没有触碰源码、测试、配置或 git 状态。

---

## 后续处置结果（2026-11-18 补充）

本节由仓库所有者在本轮整改后补充，用于记录每条发现的当前状态。审计原文保持不动，便于对照。

| ID | 原严重度 | 状态 | 处置方式 |
|---|---|---|---|
| G1 | High | ✅ 已修复 | `cargo fmt --all` 一次性收敛；新增 `rust-toolchain.toml`（1.96.0）；CI 增加 `cargo fmt --all --check` |
| S5 | Critical（未运行） | ✅ 已补跑 | 安装 `cargo-audit` 并加入 CI；初次扫描 12 个漏洞，升级 `zip` 2→9、`rustls`/`rustls-webpki`/`webpki-roots` 后清零 |
| A3 | Medium | ⏸ 不处理 | 角色词汇表不适用于插件式 CLI；后续审计按“未验证”处理，不视为 pass |
| C5 | Medium | ✅ 已修复 | 分发逻辑移入 `cli::dispatch`，`main.rs` 29 行 |
| C9 | Medium | ✅ 部分修复 | 保留 CLI 展示层 stdout；诊断类错误统一冒泡到 `main` 一处出口（19 处 `exit` → 2 处）；顺带修复日志误写 stdout 污染 `--json` |
| G5 | Medium | ✅ 已覆盖 | 本地与 CI 均完成 musl 静态构建；`v0.3.0-rc.2` Release 实测通过 |
| G4 | Low | ✅ 已修复 | 新增 `rust-toolchain.toml` 固定 1.96.0 |

另外修复了审计未列出的两个问题：`--json` 序列化失败静默打印空行；无效 `--log-level` 生成 `target=trace` 指令而意外屏蔽其他日志。

---

## Executive Summary

**整体健康度：** 🟡 需要关注

判定依据：无 Critical；存在 1 项 High（格式门禁失败）。按技能规则，任何 High 都把结论压到 🟡，并且失败的门禁禁止给出 🟢。

真正值得注意的不是数量，而是三件事：**项目自身的 CI 并没有跑格式门禁**（所以 33 个文件的格式漂移一直没被发现）；**角色映射为空导致 B/C/D 三组的脚本检查集体空转**（本报告用人工核查补齐，并显式标注未验证项）；**依赖 CVE 检查（S5）因为没有 cargo-audit 而没跑**，这一项是本报告最大的盲区。

| 类别 | 状态 | 问题数 |
|---|---|---|
| MIGRATE. 架构模式 | N/A | 0（已是多 crate） |
| A. Workspace 结构 | 🟡 | 1（A3） |
| B. 依赖方向 | 🟡 | 0（脚本未验证，人工核查无违规） |
| S. 安全 | 🟡 | 0（S5 未运行） |
| G. CI 门禁一致性 | 🟡 | 3（G1 / G5 / G4） |
| C. 代码质量 | 🟡 | 2（C5 / C9） |
| D. 模块组织 | N/A | 0（角色不适用） |
| E. 前端（SvelteKit） | N/A | 0（无前端） |

**问题计数**

| 严重度 | 数量 |
|---|---|
| 🔴 Critical | 0 |
| 🟠 High | 1 |
| 🟡 Medium | 4 |
| 🔵 Low | 1 |
| **合计** | **6** |

---

## CI Gate Parity

**门禁结果：** 1 个门禁失败（G1），G2/G3 通过，G4/G5 为告警。所有三个门禁（fmt / lint / test）在同一次本地执行中完成，不存在"CI 停在前一步"的遮蔽。

**工具链：** 本地 `rustc 1.96.0 (ac68faa20 2026-05-25)` / `cargo 1.96.0`，对比 CI 声明的 `dtolnay/rust-toolchain@stable`（来源 `.github/workflows/ci.yml`）。仓库没有 `rust-toolchain.toml` / `rust-toolchain`，因此该 pin 是浮动通道，本地与 CI 的工具链无法保证一致（edition = 2024 的格式化结果对 rustfmt 版本敏感）。

| ID | 门禁 | 实际执行命令 | 命令来源 | 结果 | 发现数 |
|---|---|---|---|---|---|
| G1 | 格式 | `cargo fmt --all --check` | default（CI 未声明 fmt 步骤） | ❌ fail | 33 个文件 / 172 个 hunk |
| G2 | Lint | `cargo clippy` | `.github/workflows/ci.yml` | ✅ pass | 0 |
| G3 | 测试 | `cargo test` | `.github/workflows/ci.yml` | ✅ pass | 99 个测试通过 |
| G4 | 工具链一致性 | `rustc -V` vs `stable` | `.github/workflows/ci.yml` | ⚠️ warn | 1 |
| G5 | 门禁覆盖度 | — | — | ⚠️ warn | 3 |

### 门禁命令与 CI 声明的差异（已人工补齐）

脚本发现的 G2/G3 命令丢掉了 CI 在 `run:` 里写的参数。本次审计额外按 CI 原文执行了对应命令，结果如下（同样是只读门禁，不修改源码）：

```
$ cargo check --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
    → 退出码 0

$ cargo clippy --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
    → 退出码 0

$ cargo test --locked -- --test-threads=1
    test result: ok. 99 passed; 0 failed ...
    → 退出码 0，99 个测试通过
```

结论：CI 的 `check` / `clippy` / `test` 三步在本地全部通过，包括 CI 真实使用的单线程测试参数。

### CI 发现但本次未在本地执行的门禁（G5）

- `cargo build --release --locked --target x86_64-unknown-linux-musl`（`.github/workflows/release.yml`）— 需要在本地安装 musl 目标与 `musl-tools`，本次未执行。**发布产物的静态链接构建未被验证**，这是本次门禁一致性的最后一块空白。
- `cargo check` 与 `cargo check --locked` 被脚本列为"未覆盖"，但审计已手工执行 `cargo check --locked`（退出码 0），这一项不再是空白。

### Gate failures

#### G1：格式门禁失败（33 个文件需要重新格式化）

- **观察：** `cargo fmt --all --check` 以非零退出码结束，输出 172 个 `Diff in` 块，覆盖 33 个不同文件；脚本按文件去重后列出前 20 个，并注明还有 12 个未列出（脚本自身计数为 32，本次直接测量为 33，以直接测量为准）。
  ```
  Diff in /home/cxd/Projects/aiediter/opsctl/opsctl/src/handlers/dns.rs:... （28 个 hunk）
  Diff in /home/cxd/Projects/aiediter/opsctl/opsctl-core/src/config.rs:... （15 个 hunk）
  Diff in /home/cxd/Projects/aiediter/opsctl/opsctl-deploy/src/history.rs:... （14 个 hunk）
  ... 共 33 个文件 / 172 个 hunk
  ```
  涉及全部 6 个 crate：`opsctl-cert`(4)、`opsctl-core`(2)、`opsctl-deploy`(10)、`opsctl-dns`(6)、`opsctl-notify`(5)、`opsctl`(6)。
- **标准：** `G1` — 项目自身的格式门禁必须通过；项目没有声明自己的 fmt 门禁时，以 `cargo fmt --all --check` 为准。
- **命令：** `cargo fmt --all --check`
- **评估：** High — 按检查表 G1 的定级。**需要说明的语境：** 本项目的 CI 并没有 fmt 步骤（`.github/workflows/ci.yml` 只有 `check` / `test` / `clippy`），所以这个失败不会让 CI 变红，它代表的是"仓库与 rustfmt 默认风格长期漂移"，而不是"CI 已坏"。这一点不改变 G1 的定级，但影响修复优先级。
- **原因：** 未确定。可观察到的事实是：仓库没有 `rustfmt.toml`/`.rustfmt.toml`（无 `style_edition` 覆盖），edition 为 2024，而本地 rustfmt 为 1.96.0；无法据此断定漂移是"从未跑过 fmt"还是"CI/本地 rustfmt 版本不同"造成的——两种假设都能解释现象，审计不做定论。
- **建议处置（供参考）：** 若要收敛，先确认以哪一版 rustfmt 为基准（建议在 `rust-toolchain.toml` 中固定版本），再一次性执行格式化并把 `cargo fmt --all --check` 加入 CI，否则会再次漂移。格式化只改空白，但会触及 33 个文件，建议独立提交以免污染语义 diff。
- **预估工作量：** S（不改变语义，但需要一次独立提交与回归）

---

## Adversarial Audit Pass

**复核输入：** `/tmp/quality.jsonl`（83 行：81 条 C9 + 2 条 info）、`/tmp/security.jsonl`（1 行）、`/tmp/gate.jsonl`（28 行）、`/tmp/dep.jsonl`（1 行）、本报告初稿、`references/audit-checklist.md` 的 §A/§B/§C/§D/§G/§S 与 `references/gotchas.md`。

**结果：** 发现实质性缺口（Material gaps found）— 缺口在"脚本空转"而非"结论方向"。

### 增补或改判的发现

| ID | 动作 | 严重度 | 位置 | 证据 |
|---|---|---|---|---|
| B1 | 改判（pass 改为"未验证"） | — | `dep-check.sh` 输出 | 输出为 `"roles mapped: "`（空），即没有任何 package 映射到角色，禁止边矩阵没有被真正求值。脚本的 `pass` 是空转，不能作为结论 |
| C5 | 新增（脚本跳过后人工核查） | Medium | `opsctl/src/main.rs:1` | 脚本因无 app 角色回退到不存在的 `src/main.rs` 而跳过；`wc -l` 实测 74 行 > 30 行阈值。按 gotcha 关于"纯接线 main 的可接受度"下调 High→Medium，理由写在发现内 |
| C4 | 复核为空转 | — | `quality-scan.sh` | 无 domain 角色 → 检查被跳过，`opsctl-dns/src/types.rs` 与 `opsctl-core/src/config.rs` 的 `Deserialize` 由人工判断，未作为发现上报（见观察） |
| C1 / C2 | 人工复核确认非假阴性 | — | 6 个 crate | 全部 81 处 `unwrap`/`expect` 均位于 `#[cfg(test)]` 之后或测试模块内；`opsctl-dns/src/providers/test_server.rs` 是 `#![cfg(test)]` + `#[cfg(test)] mod`，属测试代码，不构成 C1 |
| S1 | 复核确认为匹配噪声 | — | `opsctl-cert/src/lib.rs:306` | `rg 'unsafe'` 唯一命中是测试函数名 `sanitize_domain_replaces_unsafe_characters`，不存在 `unsafe {}` 块 |
| C9 | 保留但收窄结论 | Medium | `opsctl/src/handlers/*.rs` | 81 处 `println!`/`eprintln!` 全部集中在 CLI crate 的 6 个文件，4 个库 crate 零命中原生输出（全部用 `tracing`）。这不是"忘记用 tracing"，而是"展示层用 stdout" |

### 覆盖情况

- A1/A2/A4/A5：读根 `Cargo.toml` 与 `cargo metadata`，成员、`resolver`、`[workspace.dependencies]` 均确认。
- B1：脚本未验证；人工读 5 个成员 manifest 的 `[dependencies]`，边集为 `opsctl → {cert,deploy,dns,notify,core}`、`{cert,deploy,dns,notify} → core`，`core` 无内部依赖，无环。
- C1/C2/C3/C6/C7/C8/C10：`rg` 全仓复核（见上表）。
- C4/C5/D1–D5：角色不适用，逐项标注为未验证或 N/A，未以脚本 pass 冒充覆盖。
- G1–G5：以 `ci-gate.sh` 原始输出为准，并对 G2/G3 的命令差异做了 CI 原文复跑。
- S1/S2/S3/S4/S5：S1–S3 全仓 `rg`；S4 因无 HTTP 服务层判 N/A；S5 未运行（工具缺失），在 Skipped 中声明。

### 假设

- 假设一：项目定位是"单机运维 CLI"，因此 `api/server/middleware/AppState` 一类检查项不适用，而不是"缺失"。依据：`README.md` 与 `docs/ARCHITECTURE.md` 均将产物描述为单文件静态二进制 CLI。
- 假设二：`opsctl-core` 承担了 `config` + `common` 两个角色的职责。这是审计方的归角色推断，仓库未声明角色词汇表，因此没有把它的混合职责作为违规上报。

---

## Issues

### 🟠 High

#### G1：格式门禁失败，33 个文件与 rustfmt 默认风格漂移

- **观察：** `cargo fmt --all --check` 退出码非零，172 个 hunk 分布在 33 个文件，覆盖全部 6 个 crate。详见上文 "Gate failures → G1"。
- **标准：** `G1` — 项目自身格式门禁（未声明时用 `cargo fmt --all --check`）必须通过。
- **评估：** High — 检查表 G1 定级。所有者可能认为"CI 本来也不查 fmt"因此不紧急，那是优先级判断，不改变门禁本身的红/绿事实。
- **建议处置（供参考）：** 固定 rustfmt 版本 → 一次性 `cargo fmt --all` → 把 `cargo fmt --all --check` 加进 `.github/workflows/ci.yml`，形成回归门禁。
- **预估工作量：** S
- **所有者备注：** 这是本仓库唯一会随时间自动恶化的门禁项：只要没有 fmt 检查，每次提交都会继续加大漂移面。

---

### 🟡 Medium

#### A3：6 个 package 全部无法映射到角色，B/C/D 三组脚本检查集体空转

- **观察：** `detect-structure.sh` 输出 `"standard_crates"` 全为 `false`、`"unmapped"` 列出全部 6 个 crate（`opsctl`、`opsctl-cert`、`opsctl-core`、`opsctl-deploy`、`opsctl-dns`、`opsctl-notify`）。`config.json` 的 `crate_roles` 为空。
  ```
  {"mode":"multi-crate","crate_count":6,"standard_crates":{"app":false,"server":false,"api":false,"domain":false,"infra":false,"config":false,"common":false},
   "role_packages":{"unknown":[...6 个...]},"unmapped":[...6 个...]}
  ```
- **标准：** `A3` — 每个 package 应能通过 `crate_roles` 映射到角色，或明确列为 unmapped。检查表注明这"从不是硬失败，只报告"。
- **评估：** Medium — 单个 crate 未映射只是观察；但这里是 6/6 全部未映射，直接后果是 **B1、C3、C4、C5、D1、D5 六项检查没有真正求值**（`dep-check.sh` 输出 `roles mapped: ` 为空却报 pass，`quality-scan.sh` 对 C4/C5 输出 skipped）。审计结论的可信度因此下降，这是"检查覆盖度"问题，不是"代码违规"。
- **建议处置（供参考）：** 所有者决定是否采用技能的默认角色词汇。若采用，一组可行的映射是：`opsctl` → `app`，`opsctl-core` → `common`（其 `config.rs` 亦可视为 `config` 角色），四个 feature crate 无现成角色，可保留为 unmapped 并在 `crate_roles` 中显式记录"feature 插件"语义。若认为该角色体系不适用于插件式 CLI，也可继续留空——但后续审计必须继续按"未验证"处理，不能报 pass。
- **预估工作量：** XS（改 `config.json`，不改代码）

#### C5：`main.rs` 74 行，超出 30 行阈值（按 getcha 下调为 Medium）

- **观察：** `opsctl/src/main.rs` 共 74 行，内容为：解析 CLI、加载全局配置、初始化日志、一个两层 `match` 分发到 8 个 handler。
  ```
  $ wc -l opsctl/src/main.rs
  74 opsctl/src/main.rs
  ```
- **标准：** `C5` — app 角色的 `main.rs` 不超过 30 行，只做接线。检查表定级 High。
- **评估：** **Medium（对检查表 High 的显式下调）** — 依据 `gotchas.md` 的 "app/main.rs Line Count"：判定要看行里装了什么。这里 74 行全部是纯接线与分发表，没有业务逻辑、没有 inline handler，也没有结构体定义；分发表更适合放在 `cli.rs` 里与 `Commands` 枚举相邻。下调是对规则的校准，不是给事实打折。
- **建议处置（供参考）：** 把 `match &cli.command { ... }` 抽成 `cli::dispatch(cfg, cli)`，`main` 收敛到约 25 行；顺带把 `eprintln!` + `process::exit(1)` 的错误出口集中到一处（见观察）。
- **预估工作量：** S

#### C9：CLI 层 81 处 `println!`/`eprintln!`，其中存在与 `tracing` 重复的错误出口

- **观察：** 全仓 81 处原生输出，全部位于 `opsctl`（二进制）crate 的 6 个文件：`handlers/dns.rs` (28)、`handlers/deploy.rs` (20)、`handlers/cert.rs` (17)、`handlers/init.rs` (12)、`helpers.rs` (3)、`main.rs` (1)。4 个库 crate 零命中，全部使用 `tracing`。
  ```
  {"check":"C9","file":".../opsctl/src/main.rs","line":27,"context":"eprintln!(\"错误: {e}\");"}
  {"check":"C9","file":".../opsctl/src/helpers.rs","line":47,"context":"println!(\"⏭️  {} (version={}): {}\", ...);"}
  ... 共 81 处
  ```
- **标准：** `C9` — 测试之外不得出现 `println!`/`dbg!`/`eprintln!`，应使用 `tracing` 宏。
- **评估：** Medium — 按检查表原定级保留，但结论需要收窄：这是一个 CLI，**stdout 就是产品的输出通道**（表格、`--json` 机器可读输出），把这些 `println!` 换成 `tracing` 会破坏管道语义（`tracing` 默认写 stderr，且带时间戳前缀）。真正值得处理的是其中**与 tracing 重复的诊断路径**：`main.rs:26-27` 先 `tracing::error!` 再 `eprintln!` 同一条错误，同一条信息走了两条通道。
- **建议处置（供参考）：** 不要把展示层输出改成 tracing。可考虑：(1) 只把诊断类输出（错误、告警）收敛到一条通道；(2) 展示层输出集中到一个 `output` 模块，让"用户可见输出"与"日志"在代码上可区分；(3) 若想消除检查表告警，可在文档中显式登记"CLI 展示层允许 stdout"的例外，让后续审计有据可依。
- **预估工作量：** M

#### G5：CI 的两条构建命令未被本次门禁执行覆盖

- **观察：** `ci-gate.sh` 报出 3 条 "CI build/test command not verified"：`cargo check`、`cargo check --locked`（`.github/workflows/ci.yml`）、`cargo build --release --locked --target "$TARGET"`（`.github/workflows/release.yml`）。审计已手工执行 `cargo check --locked`（退出码 0），musl release 构建仍未执行。
  ```
  {"check":"G5","severity":"medium","status":"warn","command":"cargo build --release --locked --target \"$TARGET","source":".github/workflows/release.yml"}
  ```
- **标准：** `G5` — CI 里发现的每条构建/测试命令都应被本次门禁覆盖；未执行的必须显式列出，不得沉默。
- **评估：** Medium — 这不是代码缺陷，是**结论边界的声明**：发布路径（musl 静态链接、`crt-static`、`Cargo` 版本与 tag 一致性校验）本轮完全没被验证，报告不能暗示有完整 CI 一致性。
- **建议处置（供参考）：** 所有者决定是否在本地补跑 `rustup target add x86_64-unknown-linux-musl` + `cargo build --release --locked --target x86_64-unknown-linux-musl`；若不补跑，请在发布前单独验证。
- **预估工作量：** XS（补跑）／M（若要修复构建失败）

---

### 🔵 Low

#### G4：工具链 pin 为浮动通道 `stable`，格式与 lint 结果不可比

- **观察：** 仓库无 `rust-toolchain.toml` / `rust-toolchain` / `.tool-versions` / `mise.toml`；CI 使用 `dtolnay/rust-toolchain@stable`。本地为 `rustc 1.96.0`，`rustup` 活动工具链为 `stable-x86_64-unknown-linux-gnu`。edition = 2024。
  ```
  {"check":"G4","severity":"low","status":"warn","finding":"pinned channel is floating ('stable' via .github/workflows/ci.yml) — local and CI toolchains are not guaranteed identical (edition=2024)"}
  ```
- **标准：** `G4` — 项目为 CI pin 了工具链时，本地门禁结果只在该版本上可信；浮动通道无法断言一致性。
- **评估：** Low — 后果是"本地绿不等于 CI 绿"，而当前 G2/G3 恰好都通过，所以风险主要是**未来**的（新 clippy lint 或 rustfmt 风格变更会突然让 CI 变红，且与 G1 的漂移互相掩盖）。
- **建议处置（供参考）：** 增加 `rust-toolchain.toml` 固定版本（例如 `channel = "1.96.0"`），CI 与本地共用同一 pin。
- **预估工作量：** XS

---

## Risk Priority Plan

按严重度优先排序，其次考虑把握度与影响面。**仅为建议**——是否处理、以什么顺序处理、是否处理，均由所有者决定。

| 优先级 | ID | 标题 | 建议处置 | 备注 |
|---|---|---|---|---|
| 1 | G1 | 33 个文件格式漂移 | 固定 rustfmt 版本 → 一次性格式化 → 把 fmt 加入 CI | 唯一会随每次提交自动恶化的一项 |
| 2 | S5 | 依赖 CVE 未扫描 | `cargo install cargo-audit` 后重跑审计 | 本报告最大盲区；在补跑前不宜宣称依赖安全 |
| 3 | A3 | 6/6 crate 无角色映射 | 在 `config.json` 声明 `crate_roles`，或显式接受"未验证" | 决定后续审计能否自动覆盖 B/C/D |
| 4 | G5 | musl 发布构建未验证 | 本地补跑一次 release 构建，或接受边界声明 | 发布前必做 |
| 5 | C9 | 81 处原生输出 | 收窄到"诊断类输出统一走一条通道"，不动展示层 | 建议连同 C5 一起整改 |
| 6 | C5 | main.rs 74 行 | 抽出 `cli::dispatch` | 与 C9 的整改同属 CLI 层收敛 |
| 7 | G4 | 浮动工具链 | 加 `rust-toolchain.toml` | 一行配置，能让前 6 项的结论稳定 |

### 建议的分流安排

以下分配仅供参考，所有者可接受、修改或忽略：

**需要所有者立即判断：**
- S5 未运行 —— 是否接受"本轮无依赖 CVE 结论"，以及何时补跑。

**计划内整改：**
- G1 格式门禁（一次性 + 加 CI）
- G5 musl 发布构建验证

**跟踪观察：**
- A3 角色映射
- C9 / C5 CLI 层收敛

**待办池：**
- G4 工具链 pin

---

## Passed Checks

✅ **A1**：根 `Cargo.toml` 含 `[workspace]`，`resolver = "2"`，`members` 列出 6 个成员，`cargo metadata` 可解析成员与角色
✅ **A2**：成员声明完整可判定（`cargo metadata` 成功返回 6 个 package）
✅ **A4**：`[workspace.dependencies]` 存在并集中管理 serde/serde_json/tracing/thiserror/ureq/toml/sha2/sha1/hex/base64/dirs，`[workspace.package]` 统一 edition/version/license
✅ **A5**：6 个 crate 全部在 `docs/ARCHITECTURE.md` 中有一句话职责说明，无"来历不明"的 crate
✅ **B1（人工）**：脚本因角色为空未求值；人工核对 5 个成员 manifest 的 `[dependencies]`，依赖图是单向下行星形（feature → core，bin → 全部），无环、无反向边
✅ **C1**：`rg '\.unwrap\(\)'` 全仓 81 处命中全部位于 `#[cfg(test)]` 之后或 `#![cfg(test)]` 模块内；生产代码零 `unwrap`
✅ **C2**：同上，`expect()` 无生产代码命中
✅ **C3（人工）**：全仓无 `anyhow`（`rg anyhow` 零命中），统一的 `OpsctlError`/`DnsError`/`CertError`/`DeployError`/`NotifyError` 均基于 `thiserror`
✅ **C7**：`anyhow` 作用域检查不适用且无违规——整个 workspace 不使用 `anyhow`
✅ **C8**：21 个文件含 `#[cfg(test)]`/`#[test]`，`cargo test` 实跑 99 个测试全绿
✅ **C10**：`rg '\b(TODO|FIXME|HACK|XXX)\b'` 在 `*.rs` 中零命中
✅ **S1**：零 `unsafe {}` 块（唯一 `unsafe` 命中是测试函数名 `sanitize_domain_replaces_unsafe_characters`）
✅ **S2**：无硬编码凭据；`rg` 保守模式零命中，`config.example.toml` 中均为占位说明
✅ **S3**：项目不使用数据库，无 `format!` 拼接 SQL
✅ **G2（lint 门禁）**：`cargo clippy` 退出码 0，零诊断；额外按 CI 原文复跑 `cargo clippy --locked` 亦退出码 0
✅ **G3（测试门禁）**：`cargo test` 退出码 0，99 个测试通过；额外按 CI 原文复跑 `cargo test --locked -- --test-threads=1` 同样 99 个通过
✅ **G2/G3 补充**：`cargo check --locked`（CI 的第一步）退出码 0

---

## Skipped / Not Applicable

- **S5（Critical 级别检查）：未运行** — 本机未安装 `cargo-audit`（`security-scan.sh` 输出 `{"check":"S5","status":"skipped","finding":"cargo-audit not installed"}`）。这意味着**依赖 CVE 状态本轮完全未知**，报告不能给出"依赖安全"方面的任何正面结论。补跑方式：`cargo install cargo-audit && cargo audit`。
- **S4（认证中间件）：N/A** — 项目不提供 HTTP 服务层，无路由与中间件；"认证"以配置文件权限（README 建议 `chmod 600 ~/.opsctl/config.toml`）形式存在，属部署实践而非代码门禁。
- **C4（DTO 泄漏）：未验证** — 无 domain 角色 crate，`quality-scan.sh` 主动跳过。人工观察：`opsctl-core/src/config.rs` 的 `Deserialize` 用于解析 TOML 配置（存储/配置反序列化），`opsctl-dns/src/types.rs` 用于解析各家 DNS 平台返回的 JSON，均不属于"HTTP 请求 DTO 污染领域模型"的形态，故未作为发现上报。
- **C6（AppState 用 trait object）：N/A** — 无 `AppState`、无 axum/Axum 状态注入。
- **D1–D5（模块组织）：N/A** — 依赖 `domain`/`api`/`server`/`infra` 角色，本项目为插件式 feature crate 布局，角色体系不适用。人工观察见下节（`types.rs` 命名）。
- **MIGRATE-1/2/3：N/A** — 已是 6 成员 workspace，非单 crate。
- **E1–E6（前端）：N/A** — 仓库无前端（`has_frontend = false`，0 个 `.svelte` 文件）。
- **G1 的 CI 归属说明** — G1 使用了 cargo 默认命令（`default`），因为 `.github/workflows/ci.yml` 未声明 fmt 步骤。这是"项目自身门禁缺失"，本报告按检查表 G1 记为失败项，同时在上文明确标注它不会让当前 CI 变红。

---

## Observations

以下条目没有对应检查项编号，属于值得记录但不由检查表裁定的内容。

- **文档与 manifest 漂移（依赖方向）：** `docs/ARCHITECTURE.md` 的依赖图写着 `opsctl-cert ──► opsctl-notify`，但 `opsctl-cert/Cargo.toml` 中并无 `opsctl-notify` 依赖，`rg 'notify' opsctl-cert/src` 零命中。实际的证书通知编排发生在二进制层：`opsctl/src/handlers/cert.rs:72` 与 `:139`、`helpers.rs:26` 直接构造 `opsctl_notify::NotifyManager`。文档描述的是"能力供给关系"，与 Cargo 的"编译依赖边"不一致，新读者据文档会得出错误结论。建议要么修正文档措辞，要么把通知编排下移到 cert crate 以匹配文档。

- **业务策略落在二进制层：** `docs/ARCHITECTURE.md` 声明 "`opsctl` binary 只负责 CLI 层编排，不承载业务实现"，但"哪些事件、以何种级别发送通知"这一策略目前写在 `opsctl/src/handlers/*.rs` 与 `helpers.rs` 中，`opsctl-notify` 只是传输层。这是有意的分工还是尚未下移，审计无法判断，仅记录观察到的分布。

- **handler 无返回值 + 19 处 `std::process::exit`：** 8 个 `handle_*` 函数签名全部返回 `()`，错误路径通过 `eprintln!` + `std::process::exit(1)` 终止进程（`handlers/cert.rs`、`handlers/deploy.rs`、`handlers/init.rs`、`handlers/dns.rs`、`helpers.rs`、`main.rs` 共 19 处）。后果是 CLI 层无法在不终止进程的前提下被测试，也无法复用与组合；这是本次唯一"架构味道"较明确的观察点。改造方向通常是让 handler 返回 `Result<(), OpsctlError>`，只在 `main` 一处调用 `exit`。

- **`--json` 输出的静默降级：** `handlers/dns.rs:41`、`:104`、`:282`、`:400` 使用 `serde_json::to_string_pretty(&x).unwrap_or_default()`，序列化失败时会打印空行而不是报错。对 `--json` 消费者而言这是静默失败而非显式错误。同类 `unwrap_or_default()` 在 HTTP 响应体读取处（`notify/*.rs`、DNS providers）出现较多，那里通常是"响应体为空"的合理兜底，风险低于输出路径。

- **测试覆盖分布不均：** 99 个测试集中在算法与协议层（`safety`、`history`、`signing`、`validation`、各 DNS provider、`formats`、`messages`），但 `opsctl-deploy` 的 `static_deploy.rs`、`extractor.rs`、`health_check.rs`、`daemon.rs` 四个文件没有任何 `#[cfg(test)]`。这四项恰好是"部署实际落地"的部分。C8 只要求"存在测试"，故记为 pass；覆盖度不足单列于此。

- **测试非并行安全（隐含约束）：** `Makefile` 的 `test` 目标是 `cargo test -- --test-threads=1`，注释写明"避免临时目录冲突"，CI 也使用单线程。这意味着测试共享文件系统状态，`make test-fast`（多线程）存在偶发失败的潜在风险。本次审计以 CI 原文命令（单线程）验证通过，未对多线程路径下结论。

- **两处 lint 抑制：** `opsctl-dns/src/operations.rs:91` 与 `:120` 使用 `#[allow(clippy::too_many_arguments)]`。clippy 当前不报错，但抑制本身说明这两个函数参数偏多，属于可选的后续重构点。

- **clippy 门禁强度：** CI 的 lint 步骤是 `cargo clippy --locked`，既没有 `-D warnings`，也没有 `--all-targets`。也就是说当前 lint 门禁**不会**因为新增 warning 失败，也不会 lint 测试目标。本轮 clippy 确实零诊断（因此 G2 记 pass），但门禁强度足够弱，值得所有者知情。

- **`opsctl-core` 混合职责：** 该 crate 同时提供 `config`（配置加载）、`logging`（tracing 初始化）与 `error`（统一错误类型），在技能的角色体系里横跨 `config` 与 `common` 两个角色。以插件式 CLI 的规模看这是合理的最小共享层，记录以备后续拆分判断。

---

*本报告由 ai-dev-audit 生成。标准：ai-dev-discipline v1。*
*文件：docs/AUDIT_REPORT.md*
