# Audit Log Template

**This file is a template, not the log itself.** Copy it to the audited project's
`docs/AUDIT_LOG.md` before the first audit of that project, then append to that
copy. Per-project history must not be accumulated here: the skill's own directory
is shared by every project it audits.

Per-project audit history. **Read this file before auditing this project again** — it reveals patterns, recurring issues, and project evolution over time.

## Purpose

This file tracks the audit history of this specific project:
1. **Project evolution** — track whether the project is improving or degrading
2. **Recurring issues** — identify persistent problems
3. **Gotcha validation** — real audit findings feed back into `references/gotchas.md`

## Summary Table

| Date | Project | Mode | 🔴 | 🟠 | 🟡 | 🔵 | Health | Notes |
|------|---------|------|-----|-----|-----|-----|--------|-------|
| 2026-05-25 | opsctl | multi-crate (6 members) | 0 | 1 | 4 | 1 | 🟡 | G1 格式门禁失败（33 文件 / 172 hunk）；S5 cargo-audit 未安装未运行；A3 角色映射为空导致 B/C/D 脚本检查空转 |
| 2026-11-18 | opsctl | multi-crate (6 members) | 0 | 0 | 1 | 0 | 🟢 | 整改轮：G1/S5/C5/G4/G5 已修复（fmt 收敛 + CI 门禁 + zip 9 / rustls 升级后 audit 清零 + dispatch 抽取 + 工具链 pin）；A3 不处理（角色体系不适用）；C9 部分修复（保留展示层 stdout） |

## Findings Log

Each audit appends its findings here:

```
### <YYYY-MM-DD> — <mode>

**Health:** 🟢/🟡/🔴
**Key findings:**
- <CHECK-ID>: <severity> <status> -- <one-line summary> @ <file>
- <CHECK-ID>: <severity> <status> -- <one-line summary> @ <file>
**Gate parity (G1–G5):** <G1 fmt / G2 clippy / G3 test / G4 toolchain / G5 coverage — pass, fail, or skipped, with the command that was run>
**Recurring from last audit:** <none or list IDs>
**New this audit:** <list IDs>
**Resolved since last audit:** <list IDs>
**Adversarial pass:** <No material gaps / Material gaps found / Inconclusive>
```

Status values:
- `OPEN`: Evidence supports the finding in the current audit.
- `RECLASSIFIED`: Adversarial pass changed the severity or category.
- `FALSE-POSITIVE`: Evidence does not support reporting the finding.
- `RECURRING`: Same finding, same severity, seen in a previous audit. Re-reported
  deliberately. Repetition never raises the severity — see `references/gotchas.md`.
- `RESOLVED`: Finding no longer appears and the relevant check was covered.

There is intentionally no "accepted" or "waived" status. The audit cannot know
what the owner has decided, and re-reports findings either way. Whether a durable
finding still matters is the owner's call, not the audit's.

### 2026-05-25 — multi-crate

**Health:** 🟡
**Key findings:**
- G1: High OPEN -- `cargo fmt --all --check` 非零退出，172 个 hunk / 33 个文件，覆盖全部 6 个 crate（CI 本身无 fmt 步骤，故不会让 CI 变红） @ `opsctl-cert/src/certd.rs`、`opsctl-core/src/config.rs`、`opsctl-deploy/*`、`opsctl-dns/*`、`opsctl-notify/*`、`opsctl/src/*`
- A3: Medium OPEN -- 6/6 package 未映射到角色，`dep-check.sh` 输出 `roles mapped: ` 为空却报 pass（空转），C4/C5 被跳过 @ `config.json`（`crate_roles: {}`）
- C5: Medium OPEN -- `main.rs` 74 行（阈值 30），按 gotcha 从 High 下调：内容全为纯接线与分发表 @ `opsctl/src/main.rs:1`
- C9: Medium OPEN -- 81 处 `println!`/`eprintln!` 全部集中在 CLI crate 的 6 个文件；库 crate 零命中原生输出 @ `opsctl/src/handlers/{dns,deploy,cert,init}.rs`、`helpers.rs`、`main.rs`
- G5: Medium OPEN -- CI 的 `cargo build --release --locked --target x86_64-unknown-linux-musl` 未被本轮门禁执行（`cargo check --locked` 已手工补跑并退出 0） @ `.github/workflows/release.yml`
- G4: Low OPEN -- 工具链 pin 为浮动 `stable`（来自 ci.yml），仓库无 `rust-toolchain.toml` @ `.github/workflows/ci.yml`
- S5: Critical 级别检查 SKIPPED -- 本机未安装 cargo-audit，依赖 CVE 状态本轮未知

**Gate parity (G1–G5):** G1 `cargo fmt --all --check` ❌ fail（default，CI 未声明 fmt）／G2 `cargo clippy` ✅ pass（`.github/workflows/ci.yml`；另复跑 `cargo clippy --locked` 退出 0）／G3 `cargo test` ✅ pass 99 tests（`.github/workflows/ci.yml`；另复跑 CI 原文 `cargo test --locked -- --test-threads=1` 同样 99 passed）／G4 ⚠️ warn 浮动 `stable`／G5 ⚠️ warn，3 条构建命令未执行，其中 `cargo check --locked` 已手工补跑退出 0

**Recurring from last audit:** none（本项目首次审计）
**New this audit:** G1, A3, C5, C9, G5, G4
**Resolved since last audit:** n/a
**Adversarial pass:** Material gaps found — 修正点：B1 的脚本 pass 属于角色为空导致的空转，改判为“未验证”；C5 从脚本 skipped 补为人工实测发现；C4/C1/C2/S1 的脚本结果经人工复核后确认或标为噪声

## Gotcha Validation Log


When an audit discovers a new false positive pattern or a gotcha that was missing, log it here. Periodically review this section to update `references/gotchas.md`.

| Date | Gotcha ID | Discovery Context | Action |
|------|-----------|-------------------|--------|
| 2026-05-25 | role-map-vacuous-pass | `crate_roles` 为空时，`dep-check.sh` 仍输出 `{"check":"B1","status":"pass","finding":"no forbidden dependency directions found (roles mapped: )"}` —— 禁止边矩阵根本没被求值，pass 是空转。`gotchas.md` 的 "Dep-Check Works on Roles, Not Directories" 只讲了角色映射错了怎么办，没讲映射为空时不得视为 pass。按同样形式，`quality-scan.sh` 的 C4/C5 在无对应角色时输出 `skipped`（这是正确行为），两组脚本行为不一致 | 建议加入 `references/gotchas.md`：空角色映射下 B1 必须按“未验证”读，不得按 pass 读。本次审计只记录，未修改技能自身的 `gotchas.md`（技能变更按“Changing This Skill”需单独跑 `check-docs.sh` 与 `selftest.sh`，不属于项目审计范围） |
| 2026-05-25 | fmt-diff-per-hunk | `cargo fmt --all --check` 的 `Diff in` 输出是**每个 hunk 一行**（本例 172 行），不是每个文件一行；按文件去重后为 33 个，而 `ci-gate.sh` 报出 20 个文件 + “12 further”（合计 32）。`gotchas.md` 已提醒“不要把 hunk 当文件”，但未提醒脚本自身的截断计数可能与直接测量不一致 | 报告以直接测量的 33 为准，并写明差异。建议脚本后续对齐 `n` 的计算与 `shown` 的截断 |

## How to Use This File

**Before auditing:** Read this file to see what was flagged before and whether it was resolved.

**After auditing:** Append findings. If you discovered a new false positive or a gotcha that was missing from `references/gotchas.md`, log it in the Gotcha Validation section AND update gotchas.md.

**Recurring findings:** mark them `RECURRING` and re-report them at the same
severity. Never escalate because a finding is old — see `references/gotchas.md` →
Recurring Issues. There is no "owner accepted" status by design.

### 2026-11-18 — 整改轮

**Health:** 🟢
**Key findings:**
- G1: RESOLVED -- `cargo fmt --all` 收敛 33 个文件；`rust-toolchain.toml` 固定 1.96.0；CI 增加 `cargo fmt --all --check`
- S5: RESOLVED -- 安装 cargo-audit 并加入 CI；12 个漏洞（zip 6 / rustls 4 / rustls-webpki 1 / webpki-roots 1）经升级后清零
- C5: RESOLVED -- 分发移入 `cli::dispatch`，`main.rs` 29 行
- G4: RESOLVED -- 新增 `rust-toolchain.toml`
- G5: RESOLVED -- 本地与 CI 均完成 musl 静态构建，`v0.3.0-rc.2` Release 实测通过
- C9: RECLASSIFIED（部分修复）-- 保留 CLI 展示层 stdout；19 处 `process::exit` 收敛为 `main` 一处出口
- A3: OPEN（不处理）-- 角色体系不适用于插件式 CLI，后续审计按“未验证”读
**Gate parity (G1–G5):** G1 `cargo fmt --all --check` ✅ / G2 `cargo clippy --locked` ✅ 0 诊断 / G3 `cargo test --locked -- --test-threads=1` ✅ 111 passed / G4 工具链已 pin / G5 发布构建已实测
**Recurring from last audit:** none
**New this audit:** 日志误写 stdout 污染 `--json`；无效 `--log-level` 生成 `target=trace` 指令；ZIP 解压改用 `enclosed_name()`
**Resolved since last audit:** G1, S5, C5, G4, G5
**Adversarial pass:** Material gaps found — 审计未覆盖 stdout/stderr 通道分离，本轮补测发现并修复
