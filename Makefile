.PHONY: build release release-static test check clippy clean install run help

# ─── 默认目标 ────────────────────────────────────────────────
.DEFAULT_GOAL := help

# ─── 配置 ────────────────────────────────────────────────────
CARGO          := cargo
TARGET          := target/release/opsctl
TARGET_STATIC   := target/x86_64-unknown-linux-musl/release/opsctl
DEBUG_TARGET    := target/debug/opsctl
BIN_DIR         := /usr/local/bin

# 静态链接：musl libc → 零运行时依赖，可在任意 Linux 发行版运行
MUSL_TARGET     := x86_64-unknown-linux-musl
MUSL_RUSTFLAGS  := -C target-feature=+crt-static

# ─── 构建 ────────────────────────────────────────────────────
build: ## 编译 debug 版本
	$(CARGO) build

release: ## 编译 release 版本（glibc，快速）
	$(CARGO) build --release

release-static: ## 编译静态链接 release 版本（musl，可移植）
	RUSTFLAGS="$(MUSL_RUSTFLAGS)" $(CARGO) build --release --target $(MUSL_TARGET)
	@echo "Static binary: $(TARGET_STATIC)"
	@ls -lh $(TARGET_STATIC)

# ─── 测试 ────────────────────────────────────────────────────
test: ## 运行全部测试（单线程，避免临时目录冲突）
	$(CARGO) test -- --test-threads=1

test-fast: ## 运行测试（多线程）
	$(CARGO) test

# ─── 代码质量 ────────────────────────────────────────────────
check: ## 语法检查（快）
	$(CARGO) check

clippy: ## clippy 静态分析
	$(CARGO) clippy

lint: check clippy ## 运行全部静态检查

audit: ## 依赖安全扫描（需安装 cargo-audit）
	cargo audit || echo "提示: cargo install cargo-audit"

# ─── 代码检查（一键） ────────────────────────────────────────
ci: check clippy test ## CI 门禁 → check + clippy + test
	@echo "=== CI checks passed ==="

# ─── 清理 ────────────────────────────────────────────────────
clean: ## 清理 build 产物
	$(CARGO) clean

# ─── 安装 / 卸载 ─────────────────────────────────────────────
install: release-static ## 安装静态二进制到 $(BIN_DIR)
	@if [ -f $(TARGET_STATIC) ]; then \
		sudo cp $(TARGET_STATIC) $(BIN_DIR)/opsctl; \
		sudo chmod 755 $(BIN_DIR)/opsctl; \
		echo "opsctl → $(BIN_DIR)/opsctl"; \
		$(BIN_DIR)/opsctl --version; \
	else \
		echo "Error: $(TARGET_STATIC) not found — run 'make release-static' first"; \
		exit 1; \
	fi

uninstall: ## 卸载
	sudo rm -f $(BIN_DIR)/opsctl
	@echo "opsctl uninstalled"

# ─── 运行 ────────────────────────────────────────────────────
run: build ## 编译并运行（debug 模式，显示 --help）
	$(DEBUG_TARGET) --help

# ─── 帮助 ────────────────────────────────────────────────────
help: ## 显示帮助
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-18s\033[0m %s\n", $$1, $$2}'
