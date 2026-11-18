# Architecture & Quality Audit Report

<!-- AUDIT-META
language: zh-CN
sections: summary, gate-parity, adversarial, issues, risk-plan, passed, skipped, observations
gates: G1=pass G2=pass G3=pass G4=pass G5=warn
-->

> **请把本报告当作观察记录，而不是待执行指令。** 每条发现都包含三件事：观察到的事实、所对照的规则、以及严重度判断。严重度是审计方依据 `ai-dev-discipline v1` 给出的评估，仓库所有者可以不同意，也可以认定某项发现是有意的取舍。本次审计没有修复任何东西。

| 字段 | 值 |
|---|---|
| **项目** | opsctl |
| **审计日期** | 2026-11-18（第二轮独立复审，同日第一轮之后） |
| **审计方** | ai-dev-audit |
| **标准** | ai-dev-discipline v1 |
| **Rust Edition** | 2024 |
| **模式** | 多 crate workspace（6 个成员） |
| **被审版本** | `2066ebe chore: address audit findings (fmt, audit, logging, CLI exit)` |
| **使用工具** | `detect-structure.sh`、`quality-scan.sh`、`security-scan.sh`、`ci-gate.sh`、`dep-check.sh`、`cargo fmt --all --check`、`cargo check --locked`、`cargo clippy --locked`、`cargo test --locked -- --test-threads=1`、`cargo audit`、`rg`、`jq` |

---

## ⚠️ 关于本次审计过程的自披露（必读）

**技能脚本 `ci-gate.sh` 在本次运行中执行了会写文件的 `cargo fmt`，而不是只读的 `cargo fmt --all --check`。**

- **原因（已定位，可复现）：** 命令发现逻辑把 YAML 的 `name:` 显示标签当成了命令。`.github/workflows/ci.yml` 中同一步骤是这样写的：
  ```
        - name: cargo fmt
          run: cargo fmt --all --check
  ```
  抽取器把两行都当作候选命令，于是产生 `cargo fmt`（来自 `name:` 标签）与 `cargo fmt --all --check`（来自 `run:`）两个候选，而 `pick_gate` 取先出现的那个，即 `cargo fmt`。同一缺陷也让 G2/G3 拿到的是 `cargo clippy` / `cargo test`，丢掉了 CI 实际使用的 `--locked` 与 `-- --test-threads=1`。
- **后果：** 该命令确实执行了。**工作区未被改动** —— 运行前后 `git status --short` 均为空，内容与 `2066ebe` 一致（本仓库本来就已格式化）。也就是说这次写入是空操作，但"这次没事"不等于"这个流程安全"：若仓库当时存在未格式化文件，本次审计会静默改写 33 个文件，然后报告 G1 通过——这正是 `gotchas.md` 里 "Do Not Audit and Fix in the Same Run" 描述的事故形态。
- **处置：** 未产生任何需要回滚的改动（已用 `git status` 验证）；随后用只读命令 `cargo fmt --all --check` 独立复跑 G1，退出码 0，与脚本给出的 pass 一致。本报告的门禁结论以上述独立复跑为准，不以被污染的那次运行为准。
- **该缺陷已记入 `docs/AUDIT_LOG.md` 的 Gotcha Validation 段**，供该技能维护者修正（属于工具缺陷，不计入被审项目的发现）。

本次审计对仓库的唯一写入是本文档与 `docs/AUDIT_LOG.md`。

---

## Executive Summary

**整体健康度：** 🟢 健康（无 Critical / 无 High）

上一轮 6 项发现中 **4 项已解决**（G1 格式门禁、G4 工具链 pin、C5 main.rs 行数、S5 依赖 CVE 未扫描），**2 项按原严重度重复出现**（A3 角色映射、C9 原生输出，C9 的命中数从 81 降到 60），另有 1 项范围收窄后保留（G5：musl 发布构建仍未验证）。

判定为 🟢 的依据是技能的硬规则：无 Critical、无 High。同时必须说清它**不代表没有问题**：仍有 3 项 Medium，且 **B1 / C3 / C4 / D1–D5 因为角色映射为空而没有真正求值**，这部分覆盖是空白而非通过。变绿的主要原因是门禁与安全扫描变绿了，而不是架构层面的判断变充分了。

| 类别 | 状态 | 问题数 |
|---|---|---|
| MIGRATE. 架构模式 | N/A | 0（已是多 crate） |
| A. Workspace 结构 | 🟡 | 1（A3，重复） |
| B. 依赖方向 | 🟡 | 0（脚本仍未验证，人工核查无违规） |
| S. 安全 | 🟢 | 0（S5 已通过） |
| G. CI 门禁一致性 | 🟡 | 1（G5，范围已收窄） |
| C. 代码质量 | 🟡 | 1（C9，重复） |
| D. 模块组织 | N/A | 0（角色不适用） |
| E. 前端（SvelteKit） | N/A | 0（无前端） |

**问题计数**

| 严重度 | 数量 |
|---|---|
| 🔴 Critical | 0 |
| 🟠 High | 0 |
| 🟡 Medium | 3 |
| 🔵 Low | 0 |
| **合计** | **3** |

**与上一轮的对照**

| 检查项 | 上一轮 | 本轮 | 状态 |
|---|---|---|---|
| G1 格式门禁 | High，33 文件 / 172 hunk | pass | ✅ RESOLVED |
| G4 工具链 pin | Low，浮动 `stable` | pass，`1.96.0` 已 pin 且与本地一致 | ✅ RESOLVED |
| C5 main.rs 行数 | Medium，74 行 | pass，29 行（阈值 30） | ✅ RESOLVED |
| S5 依赖 CVE | Critical 级检查未运行 | pass，`cargo audit` 无已知漏洞（180 个依赖） | ✅ RESOLVED |
| A3 角色映射 | Medium | Medium | 🔁 RECURRING（未变化） |
| C9 原生输出 | Medium，81 处 | Medium，60 处 | 🔁 RECURRING（数量下降，定级不变） |
| G5 门禁覆盖度 | Medium，3 条未执行 | Medium，1 条（musl 发布构建）本轮仍未复现 | 🔁 RECURRING（范围收窄，定级不变） |

---

## CI Gate Parity

**门禁结果：** G1/G2/G3/G4 全部通过，G5 有 1 条命令未验证。三个主门禁（fmt / lint / test）在同一次本地执行中全部跑完，不存在"CI 停在前一步"的遮蔽。

**工具链：** 本地 `rustc 1.96.0 (ac68faa20 2026-05-25)` 与 `rust-toolchain.toml` 中 `channel = "1.96.0"` **完全一致**；`components = ["rustfmt", "clippy"]`，`profile = "minimal"`。edition = 2024。G4 因此从上一轮的浮动告警变为通过。

| ID | 门禁 | 命令（脚本实际执行） | 命令来源 | 结果 | 发现数 |
|---|---|---|---|---|---|
| G1 | 格式 | `cargo fmt`（**应为** `cargo fmt --all --check`，见上方自披露） | `.github/workflows/ci.yml`（`name:` 标签误选） | ✅ pass | 0 |
| G2 | Lint | `cargo clippy`（CI 原文为 `cargo clippy --locked`） | 同上 | ✅ pass | 0 |
| G3 | 测试 | `cargo test`（CI 原文为 `cargo test --locked -- --test-threads=1`） | 同上 | ✅ pass | 111 个测试通过 |
| G4 | 工具链一致性 | `rustc -V` vs `1.96.0` | `rust-toolchain.toml` | ✅ pass | 0 |
| G5 | 门禁覆盖度 | — | — | ⚠️ warn | 1（去重后） |

### 独立复跑（以只读命令为准）

由于 G1/G2/G3 的命令被 `name:` 标签替换，审计按 CI 原文逐条复跑，结果如下：

```
$ cargo fmt --all --check
    → 退出码 0（无任何文件需要重新格式化）

$ cargo check --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
    → 退出码 0

$ cargo clippy --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.10s
    → 退出码 0

$ cargo test --locked -- --test-threads=1
    passed: 111  failed: 0
    → 退出码 0

$ cargo audit
    Loaded 1527 security advisories
    Scanning Cargo.lock for vulnerabilities (180 crate dependencies)
    → 退出码 0（无 RUSTSEC 告警）
```

结论：CI 在 `.github/workflows/ci.yml` 中声明的 **fmt / check / test / clippy / audit 五步，本地全部复现通过**，包括 CI 真实使用的 `--locked` 与单线程测试参数。

### CI 发现但本次未在本地执行的门禁（G5）

- `cargo build --release --locked --target x86_64-unknown-linux-musl`（`.github/workflows/release.yml`）— 需要本地安装 musl 目标与 `musl-tools`，本次未执行。**发布产物的静态链接构建路径本轮未被验证**，这是门禁一致性的唯一空白。
- 脚本另外把 `cargo check`、`cargo check --locked`、`cargo audit` 列为"未覆盖"，这三条都是 `name:` 标签误判产生的重复项；审计已逐条手工执行，均退出码 0，不再是空白。

---

## Adversarial Audit Pass

**复核输入：** `/tmp/q2.jsonl`（62 行：60 条 C9 + 2 条 info）、`/tmp/s2.jsonl`（1 行）、`/tmp/g2.jsonl`（8 行）、`/tmp/d2.jsonl`（1 行）、`git show --stat 2066ebe`（变更文件清单）、本报告初稿、`references/audit-checklist.md` 的相关段落、`references/gotchas.md`。

**结果：** 发现实质性缺口（Material gaps found）—— 缺口全部在"门禁证据的可信度"，不在"结论方向"。

### 增补或改判的发现

| ID | 动作 | 严重度 | 位置 | 证据 |
|---|---|---|---|---|
| G1 | 证据重建（不改结论） | — | `.github/workflows/ci.yml` | 脚本执行的是会写文件的 `cargo fmt`；审计另跑 `cargo fmt --all --check` 得退出码 0，据此判定 pass。若不重建，G1 的 pass 属于"审计自己修好了再报绿" |
| G2 / G3 | 证据重建（不改结论） | — | 同上 | 脚本丢掉了 `--locked` 与 `-- --test-threads=1`；按 CI 原文复跑得 `passed: 111 failed: 0`、clippy 退出码 0 |
| G4 | 复核确认 | — | `rust-toolchain.toml` | `channel = "1.96.0"` 与 `rustc 1.96.0` 精确匹配（版本号相同，非仅前缀匹配），上一轮的"浮动通道"告警不再成立 |
| S5 | 复核确认 | — | `Cargo.lock` | `cargo audit` 覆盖 180 个依赖、加载 1527 条公告，退出码 0。这是**本次**实测，不是采信提交信息里的说法 |
| C5 | 复核确认 RESOLVED | — | `opsctl/src/main.rs` | `wc -l` = 29 行，内容为解析参数 → 载入配置 → 初始化日志 → `cli::dispatch` → 单一错误出口，无业务逻辑。脚本仍输出 skipped（无 app 角色），故此项由人工判定 |
| C9 | 保留（重复） | Medium | `opsctl/src/handlers/*.rs` 等 6 文件 | 60 处，仍全部集中在 `opsctl` 二进制 crate；4 个库 crate 零命中原生输出 |
| A3 | 保留（重复） | Medium | `config.json`（`crate_roles: {}`） | `dep-check.sh` 再次输出 `"roles mapped: "`（空）却报 pass，属空转，不得作为结论 |
| B1 | 改判（pass 改为"未验证"） | — | `/tmp/d2.jsonl` | 与上一轮同一问题：角色为空 → 禁止边矩阵未求值。人工核对 6 个 manifest 确认星形单向图、无环 |
| 新增 | 复核无发现 | — | `opsctl-deploy/src/extractor.rs` | 检查了本轮新增的 ZIP 解压路径校验：`enclosed_name()` + `starts_with(target_dir)` 双重保护，并配有 `../evil.txt` 的负向测试（`extractor.rs:176`）。未发现绕过 |
| 新增 | 复核无发现 | — | `opsctl/src/handlers/dns.rs:509` | 上一轮指出的"`--json` 序列化失败静默打印空行"已改为返回 `Result`，并加了 `json_pretty_returns_error_instead_of_empty_string` 测试 |
| 新增 | 复核无发现 | — | `opsctl/src/handlers/*.rs`、`opsctl/src/main.rs` | handler 全部改为返回 `HandlerResult`；`process::exit` 从 19 处降到 2 处，且都在 `main` |

### 覆盖情况

- A1/A2/A4/A5：读根 `Cargo.toml`、`cargo metadata`、`docs/ARCHITECTURE.md`。
- B1：脚本未验证；人工读 6 个成员 manifest 的 `[dependencies]`，边集为 `opsctl → {cert,deploy,dns,notify,core}`、`{cert,deploy,dns,notify} → core`，`core` 无内部依赖，无环。
- C1/C2：`rg` 全仓复核，81 处 `unwrap`/`expect` 命中逐文件比对 `#[cfg(test)]` 起点，全部位于测试上下文；`test_server.rs` 为 `#![cfg(test)]` 模块。
- C2/C3/C7：`rg anyhow` 零命中，各 crate 统一 `thiserror`。
- C6/D1–D5：角色不适用，逐项标注 N/A，未以脚本 pass 冒充覆盖。
- G1–G5：脚本原始输出 + 只读命令独立复跑（见 CI Gate Parity）。
- S1：`rg 'unsafe\s*\{'` 零命中。S2：保守模式零命中。S3：无数据库。S4：无 HTTP 服务层，N/A。S5：实测通过。

### 假设

- 假设一：项目定位是单机运维 CLI，因此 `api/server/middleware/AppState` 类检查项不适用而非缺失。依据 `README.md` 与 `docs/ARCHITECTURE.md` 对产物的描述。
- 假设二：`opsctl-core` 同时承担 `config` 与 `common` 两个角色。这是审计方的归角色推断；仓库未声明角色词汇，故不把其混合职责作为违规上报。
- 假设三：本轮未执行 musl release 构建，因此对该构建路径的现状不下任何结论（既不说好也不说坏）。

---

## Issues

### 🟠 High

本轮无 High 级发现。

---

### 🟡 Medium

#### A3：6 个 package 仍全部无法映射到角色，B/C/D 三组脚本检查持续空转（重复发现）

- **观察：** `detect-structure.sh` 中 `standard_crates` 仍全为 `false`，`unmapped` 仍列出全部 6 个 crate，`config.json` 的 `crate_roles` 仍为空。`dep-check.sh` 输出 `no forbidden dependency directions found (roles mapped: )` —— 括号内为空，说明禁止边矩阵没有被求值，该 pass 是空转。`quality-scan.sh` 对 C4/C5 输出 `skipped`。
  ```
  {"check":"B1","severity":"info","status":"pass","finding":"no forbidden dependency directions found (roles mapped: )"}
  ```
- **标准：** `A3` — 每个 package 应能通过 `crate_roles` 映射到角色，或明确列为 unmapped（检查表注明这不是硬失败）。
- **评估：** Medium — 与上一轮**同一发现、同一严重度**，按规则重复上报而不升级。后果仍是覆盖度：B1 / C3 / C4 / D1–D5 六项没有真正求值，本轮已用人工核查补齐 B1 与 C3，但 C4 与 §D 仍是空白。
- **建议处置（供参考）：** 所有者决定是否采用技能的默认角色词汇。可行映射：`opsctl` → `app`，`opsctl-core` → `common`（其 `config.rs` 亦可视为 `config` 角色），四个 feature crate 无现成角色、可保留 unmapped 并显式记录。若认为该角色体系不适用于插件式 CLI，也可继续留空——后续审计会继续按"未验证"处理，不会按 pass 处理。
- **预估工作量：** XS（改 `config.json`，不改代码）
- **所有者备注：** 上一轮已记录同一建议，本轮无变化。重复出现不代表更严重，只代表仍未处理。

#### C9：CLI 层仍有 60 处 `println!`/`eprintln!`（重复发现，数量由 81 降至 60）

- **观察：** 60 处原生输出，全部位于 `opsctl`（二进制）crate：`handlers/dns.rs` (27)、`handlers/deploy.rs` (13)、`handlers/cert.rs` (13)、`handlers/init.rs` (4)、`helpers.rs` (2)、`main.rs` (1)。4 个库 crate 零命中，全部使用 `tracing`。日志层已改为写 stderr（`opsctl-core/src/logging.rs` 的 `with_writer(std::io::stderr)`），因此"日志污染 `--json` 的 stdout"这一具体问题已被解决。
  ```
  {"check":"C9","file":".../opsctl/src/main.rs","line":26,"context":"eprintln!(\"❌ {e}\");"}
  {"check":"C9","file":".../opsctl/src/handlers/dns.rs","line":402,"context":"println!(\"{}\", \"-\".repeat(120));"}
  ... 共 60 处
  ```
- **标准：** `C9` — 测试之外不得出现 `println!`/`dbg!`/`eprintln!`，应使用 `tracing` 宏。
- **评估：** Medium — 与上一轮**同一发现、同一严重度**，重复上报而不升级。结论与上一轮相同并已部分被本轮整改印证：这是 CLI 的展示层，stdout 是产品输出通道，把这些 `println!` 全部换成 `tracing` 会破坏管道语义。仍成立的那一点是 `main.rs:26-27`：`tracing::error!` 与 `eprintln!` 输出同一条错误，信息走两条通道（虽已集中到单一出口，重复仍未消除）。
- **建议处置（供参考）：** 维持展示层输出；只把诊断类输出收敛到一条通道；若要在检查表层面消除告警，可在文档中显式登记"CLI 展示层允许 stdout"的例外。
- **预估工作量：** M

#### G5：musl 发布构建仍未在本地验证（重复发现，范围已收窄）

- **观察：** `ci-gate.sh` 报出 4 条未覆盖命令，去重并扣除 `name:` 标签误判后，真正未执行的是 1 条：`cargo build --release --locked --target x86_64-unknown-linux-musl`（`.github/workflows/release.yml`）。其余三条（`cargo check`、`cargo check --locked`、`cargo audit`）已手工执行，退出码 0。
  ```
  {"check":"G5","severity":"medium","status":"warn","command":"cargo build --release --locked --target \"$TARGET","source":".github/workflows/release.yml"}
  ```
- **标准：** `G5` — CI 中发现的每条构建/测试命令都应被本轮门禁覆盖；未执行的必须显式列出，不得沉默。
- **评估：** Medium — 与上一轮**同一发现、同一严重度**。这是结论边界的声明，不是代码缺陷：`crt-static` 静态链接、musl 工具链、以及 release.yml 里"Cargo 版本与 tag 一致"的校验，本轮都没有被验证。`cargo audit` 本轮已通过，依赖侧的空白已消除，发布构建侧的空白仍在。
- **建议处置（供参考）：** 所有者决定是否在本地补跑（`rustup target add x86_64-unknown-linux-musl` + 对应 `cargo build`）；若不补跑，请在发布前单独验证。
- **所有者申报（不是本次观测）：** `docs/AUDIT_LOG.md` 中所有者自己写的一条记录称"本地与 CI 均完成 musl 静态构建，`v0.3.0-rc.2` Release 实测通过"。这是一条**申报**，本次审计**没有复现**，因此不据此把 G5 记为 resolved——技能规则要求门禁结论必须来自本次实际执行的命令与退出码。
- **预估工作量：** XS（补跑）／M（若暴露构建失败）

---

## Risk Priority Plan

按严重度优先排序。**仅为建议**——是否处理、以什么顺序处理，均由所有者决定。本轮无 Critical / High，因此没有"必须先处理"的项。

| 优先级 | ID | 标题 | 建议处置 | 备注 |
|---|---|---|---|---|
| 1 | G5 | musl 发布构建未验证 | 发布前本地补跑一次 release 构建 | 唯一仍会直接影响交付产物的空白 |
| 2 | A3 | 角色映射为空 | 在 `config.json` 声明 `crate_roles`，或显式接受"未验证" | 决定后续审计能否自动覆盖 B/C/D |
| 3 | C9 | 60 处原生输出 | 只收敛 `main.rs` 的重复错误出口，不动展示层 | 数量已从 81 降到 60，剩余部分多为表格排版 |

### 建议的分流安排

以下分配仅供参考，所有者可接受、修改或忽略：

**需要所有者立即判断：**
- G5：发布前是否补跑 musl 构建（这一项与"交付物是否可用"直接相关）。

**计划内整改：**
- 无 High 级项需要排期。

**跟踪观察：**
- A3 角色映射（重复项，不催办）
- C9 重复错误出口

---

## Passed Checks

✅ **A1**：根 `Cargo.toml` 含 `[workspace]`、`resolver = "2"`、6 个 `members`，`cargo metadata` 可解析
✅ **A2**：成员声明完整可判定
✅ **A4**：`[workspace.dependencies]` 与 `[workspace.package]` 集中管理外部依赖与元数据
✅ **A5**：6 个 crate 在 `docs/ARCHITECTURE.md` 中均有职责说明
✅ **B1（人工）**：脚本未求值；人工核对依赖图为星形单向（feature → core，bin → 全部 feature），无环、无反向边
✅ **C1**：81 处 `unwrap`/`expect` 命中全部位于 `#[cfg(test)]` 之后或 `#![cfg(test)]` 模块内；生产代码零 `unwrap`/`expect`
✅ **C2**：同上
✅ **C3（人工）**：全仓无 `anyhow`，统一使用 `thiserror`
✅ **C4（人工）**：`Deserialize` 仅用于配置解析（`opsctl-core/src/config.rs`）与平台响应解析（DNS provider types），未出现 HTTP 请求 DTO 污染领域模型
✅ **C5**：`opsctl/src/main.rs` = 29 行（阈值 30），纯接线，错误出口集中在 `main`（**上一轮 MEDIUM → 本轮 RESOLVED**）
✅ **C6**：N/A（无 `AppState`）
✅ **C7**：全仓不使用 `anyhow`，无作用域违规
✅ **C8**：111 个测试全绿（`cargo test --locked -- --test-threads=1` 实测 passed 111 / failed 0）
✅ **C10**：`TODO|FIXME|HACK|XXX` 在 `*.rs` 中零命中
✅ **S1**：零 `unsafe {}` 块
✅ **S2**：无硬编码凭据
✅ **S3**：无数据库，无 `format!` 拼接 SQL
✅ **S5**：`cargo audit` 覆盖 180 个依赖、加载 1527 条公告，退出码 0，无已知漏洞（**上一轮 SKIPPED → 本轮 PASS**）
✅ **G1（格式门禁）**：`cargo fmt --all --check` 退出码 0，无文件需要重新格式化（**上一轮 High → 本轮 RESOLVED**）
✅ **G2（lint 门禁）**：`cargo clippy --locked` 退出码 0，零诊断
✅ **G3（测试门禁）**：`cargo test --locked -- --test-threads=1` 退出码 0，111 passed / 0 failed
✅ **G4（工具链一致性）**：`rust-toolchain.toml` 固定 `1.96.0`，与本地 `rustc 1.96.0` 一致，edition 2024（**上一轮 Low → 本轮 RESOLVED**）
✅ **本轮新增代码复核**：ZIP 解压路径校验（`enclosed_name()` + `starts_with`）与负向测试到位；`--json` 序列化失败改为返回错误并有测试覆盖；`--log-level` 非法值被拒绝而非生成伪 target 指令；日志固定写 stderr

---

## Skipped / Not Applicable

- **C5 / C4（脚本层面）：** `quality-scan.sh` 对二者输出 `skipped`（无 app / domain 角色）。本报告对 C5 用人工实测补齐（29 行，pass）；C4 以人工观察记录，未作为发现。
- **B1（脚本层面）：** `dep-check.sh` 输出 pass，但因角色为空属空转——本报告按"人工核查通过、脚本未验证"记录，不按脚本 pass 记录。
- **D1–D5（模块组织）：** 依赖 `domain`/`api`/`server`/`infra` 角色，本项目为插件式 feature crate 布局，角色体系不适用。
- **S4（认证中间件）：** 无 HTTP 服务层，N/A；访问控制以配置文件权限（README 建议 `chmod 600`）形式存在，属部署实践而非代码门禁。
- **MIGRATE-1/2/3：** 已是 6 成员 workspace，非单 crate。
- **E1–E6（前端）：** 无前端（`has_frontend = false`，0 个 `.svelte` 文件）。
- **G5（musl 发布构建）：** 未执行，见 Issues → G5。发布路径的 CI 一致性**未验证**，本报告不对其下结论。

---

## Observations

以下条目没有对应检查项编号，属于值得记录但不由检查表裁定的内容。

- **文档与 manifest 漂移仍在（依赖方向）：** `docs/ARCHITECTURE.md:28` 仍写着 `opsctl-cert ──► opsctl-notify`，但 `opsctl-cert/Cargo.toml` 无 `opsctl-notify` 依赖，`rg notify opsctl-cert/src` 零命中。实际通知编排在二进制层（`opsctl/src/handlers/cert.rs`、`helpers.rs`）。上一轮已记录，本轮未变。文档描述的是"能力供给关系"而非编译依赖边，新读者据文档会得出错误结论。

- **业务策略仍落在二进制层：** `docs/ARCHITECTURE.md` 声明"`opsctl` binary 只负责 CLI 层编排，不承载业务实现"，而"哪些事件、何种级别发送通知"仍写在 `opsctl/src/handlers/*.rs` 与 `helpers.rs` 中，`opsctl-notify` 只是传输层。本轮 handler 改为返回 `Result` 之后，这一层的可测试性明显改善，但职责归属未变。

- **`main.rs` 的两处 `process::exit` 是刻意的：** 第一处用于"配置加载失败且日志尚未初始化"，第二处是 `dispatch` 错误的唯一出口。两处都合理；与上一轮的 19 处相比，这是本轮质量改善最明显的一项。

- **测试覆盖分布仍不均：** 111 个测试集中在算法与协议层（`safety`、`history`、`signing`、`validation`、各 DNS provider、`formats`、`messages`、`extractor`、`logging`、`cli`）。仍无 `#[cfg(test)]` 的实现文件：`opsctl-deploy/src/{daemon,health_check,scanner,static_deploy}.rs`、`opsctl-notify/src/{dingtalk,feishu,slack,wework}.rs`。其中 `extractor.rs` 本轮已从"无测试"变为"有测试"，方向正确；`health_check` / `static_deploy` 仍是部署落地的关键路径且无测试覆盖。C8 只要求"存在测试"，故记为 pass，覆盖度不足单列于此。

- **测试非并行安全（隐含约束）：** `Makefile` 的 `test` 目标仍是 `cargo test -- --test-threads=1`，注释写明"避免临时目录冲突"，CI 也使用单线程。测试共享文件系统状态，`make test-fast`（多线程）存在偶发失败的潜在风险。本轮未对多线程路径下结论。

- **两处 lint 抑制保留：** `opsctl-dns/src/operations.rs:90` 与 `:119` 使用 `#[allow(clippy::too_many_arguments)]`。clippy 当前零诊断，抑制本身提示这两个函数参数偏多，属可选重构点。

- **clippy 门禁强度仍偏弱：** CI 的 lint 步骤是 `cargo clippy --locked`，无 `-D warnings`、无 `--all-targets`。即新增 warning 不会让 CI 变红，测试目标也不会被 lint。本轮 clippy 确实零诊断（因此 G2 记 pass），但门禁强度不足以防止退化。

- **新增了 `cargo audit` 独立 job：** `.github/workflows/ci.yml` 现在有一个 `audit` job（`taiki-e/install-action@cargo-audit` + `cargo audit`）。这是上一轮 S5 空白的直接补齐，比"本地偶尔跑一次"更持久。

- **`opsctl-core` 混合职责未变：** 该 crate 仍同时提供 `config`（配置加载）、`logging`（tracing 初始化）与 `error`（统一错误类型），在技能的角色体系里横跨 `config` 与 `common`。以当前规模看是合理的最小共享层，记录以备后续拆分判断。

- **依赖升级的安全含义（观察，非结论）：** `opsctl-deploy` 的 `zip` 从 2 升到 9，`Cargo.lock` 相应更新（`cargo audit` 现在覆盖 180 个依赖且无告警）。审计只确认当前 `Cargo.lock` 在 `cargo audit` 下干净，不对"曾经的风险是否已被完全消除"下结论——那需要看具体 RUSTSEC 条目与被移除的版本，超出本轮工具输出范围。

---

*本报告由 ai-dev-audit 生成。标准：ai-dev-discipline v1。*
*文件：docs/AUDIT_REPORT.md*
