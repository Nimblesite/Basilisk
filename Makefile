SHELL := /bin/bash
.SHELLFLAGS := -euo pipefail -c
.DEFAULT_GOAL := help

# ── Configuration ─────────────────────────────────────────────────────────────

EXTENSION_DIR := vscode-extension
ZED_DIR       := basilisk-zed
NVIM_DIR      := basilisk.nvim
OPEN          ?= 0
RULE          ?=

# Coverage thresholds (override via environment)
TEST_COVERAGE_BASILISK_CHECKER  ?= 92
TEST_COVERAGE_BASILISK_CLI      ?= 94
TEST_COVERAGE_BASILISK_DB       ?= 100
TEST_COVERAGE_BASILISK_LSP      ?= 74
TEST_COVERAGE_BASILISK_MOJO     ?= 91
TEST_COVERAGE_BASILISK_PARSER   ?= 100
TEST_COVERAGE_BASILISK_PLUGIN   ?= 100
TEST_COVERAGE_BASILISK_RESOLVER ?= 95
TEST_COVERAGE_BASILISK_STUBS    ?= 100
TEST_COVERAGE_BASILISK_CONFIG   ?= 92
TEST_COVERAGE_VSIX              ?= 60
TEST_COVERAGE_NVIM              ?= 30

# ── Build ─────────────────────────────────────────────────────────────────────

.PHONY: build build-rust build-vsix

build: build-rust build-vsix ## Build all artifacts

build-rust: ## Build Rust workspace (release)
	@echo -e '\033[1m\033[0;36m▶ Building Rust (release)\033[0m' && \
	cargo build --release && \
	echo -e '\033[0;32m✓ Rust build complete\033[0m'

build-vsix: ## Build VS Code extension
	@echo -e '\033[1m\033[0;36m▶ Building VS Code extension\033[0m' && \
	cd $(EXTENSION_DIR) && npm ci && npm run compile && \
	echo -e '\033[0;32m✓ VS Code extension compiled\033[0m'

# ── Lint ──────────────────────────────────────────────────────────────────────

.PHONY: lint lint-rust lint-vsix

lint: lint-rust lint-vsix ## Lint all languages

lint-rust: ## Lint Rust (clippy + fmt)
	@echo -e '\033[1m\033[0;36m▶ Linting Rust\033[0m' && \
	cargo clippy --workspace --all-targets -- -D warnings && \
	cargo fmt --all -- --check && \
	echo -e '\033[0;32m✓ Rust lint passed\033[0m'

lint-vsix: ## Lint VS Code extension (ESLint)
	@echo -e '\033[1m\033[0;36m▶ Linting VS Code extension\033[0m' && \
	cd $(EXTENSION_DIR) && npm run lint && \
	echo -e '\033[0;32m✓ VS Code lint passed\033[0m'

# ── Format ───────────────────────────────────────────────────────────────────

.PHONY: format format-rust format-python format-vsix

format: format-rust format-python format-vsix ## Format all code

format-rust: ## Format Rust code
	@echo -e '\033[1m\033[0;36m▶ Formatting Rust\033[0m' && \
	cargo fmt --all && \
	echo -e '\033[0;32m✓ Rust formatted\033[0m'

format-python: ## Format Python code (ruff)
	@echo -e '\033[1m\033[0;36m▶ Formatting Python\033[0m' && \
	ruff format --exclude '*/fixtures/*' . && \
	ruff check --fix --exclude '*/fixtures/*' . && \
	echo -e '\033[0;32m✓ Python formatted\033[0m'

format-vsix: ## Format VS Code extension (ESLint --fix)
	@echo -e '\033[1m\033[0;36m▶ Formatting VS Code extension\033[0m' && \
	cd $(EXTENSION_DIR) && npm run lint:fix && \
	echo -e '\033[0;32m✓ VS Code extension formatted\033[0m'

# ── Test ──────────────────────────────────────────────────────────────────────

.PHONY: test test-rust test-vsix test-nvim test-zed test-compiler test-lsp audit

test: audit ## Run full test suite (Rust first, then extensions in parallel)
	@$(MAKE) --no-print-directory test-rust && \
	$(MAKE) --no-print-directory -j3 test-vsix test-nvim test-zed && \
	echo -e '\n\033[0;32m✓ All tests passed.\033[0m'

audit: ## Check all required build/test dependencies
	@bash scripts/audit.sh

test-rust: ## Run Rust tests with coverage + thresholds (OPEN=1 for report)
	@OPEN=$(OPEN) \
	TEST_COVERAGE_BASILISK_CHECKER=$(TEST_COVERAGE_BASILISK_CHECKER) \
	TEST_COVERAGE_BASILISK_CLI=$(TEST_COVERAGE_BASILISK_CLI) \
	TEST_COVERAGE_BASILISK_DB=$(TEST_COVERAGE_BASILISK_DB) \
	TEST_COVERAGE_BASILISK_LSP=$(TEST_COVERAGE_BASILISK_LSP) \
	TEST_COVERAGE_BASILISK_MOJO=$(TEST_COVERAGE_BASILISK_MOJO) \
	TEST_COVERAGE_BASILISK_PARSER=$(TEST_COVERAGE_BASILISK_PARSER) \
	TEST_COVERAGE_BASILISK_PLUGIN=$(TEST_COVERAGE_BASILISK_PLUGIN) \
	TEST_COVERAGE_BASILISK_RESOLVER=$(TEST_COVERAGE_BASILISK_RESOLVER) \
	TEST_COVERAGE_BASILISK_STUBS=$(TEST_COVERAGE_BASILISK_STUBS) \
	TEST_COVERAGE_BASILISK_CONFIG=$(TEST_COVERAGE_BASILISK_CONFIG) \
	bash scripts/test-rust.sh

test-vsix: ## Run VS Code extension tests + coverage threshold
	@TEST_COVERAGE_VSIX=$(TEST_COVERAGE_VSIX) bash scripts/test-vscode.sh

test-nvim: ## Run Neovim extension e2e + screenshot tests
	@TEST_COVERAGE_NVIM=$(TEST_COVERAGE_NVIM) bash scripts/test-nvim.sh

test-zed: ## Run Zed extension tests
	@bash scripts/test-zed.sh

test-compiler: ## Run compiler E2E tests
	@echo -e '\033[1m\033[0;36m▶ Running Basilisk compiler E2E tests\033[0m' && \
	cargo test --profile ci -p basilisk-compiler --test e2e_tests -- --nocapture && \
	echo -e '\033[0;32m✓ All compiler E2E tests passed\033[0m'

test-lsp: ## Run LSP integration tests (slow, not in main suite)
	@echo -e '\033[1m\033[0;36m▶ Running LSP stdio tests\033[0m' && \
	cargo test --profile ci -p basilisk-lsp --test lsp_stdio_tests && \
	echo -e '\033[0;32m✓ lsp_stdio_tests done\033[0m' && \
	echo -e '\033[1m\033[0;36m▶ Running workspace core tests\033[0m' && \
	cargo test --profile ci -p basilisk-lsp --test ws_core_tests && \
	echo -e '\033[0;32m✓ ws_core_tests done\033[0m' && \
	echo -e '\033[1m\033[0;36m▶ Running workspace features tests\033[0m' && \
	cargo test --profile ci -p basilisk-lsp --test ws_features_tests && \
	echo -e '\033[0;32m✓ ws_features_tests done\033[0m' && \
	echo -e '\033[1m\033[0;36m▶ Running workspace navigation tests\033[0m' && \
	cargo test --profile ci -p basilisk-lsp --test ws_navigation_tests && \
	echo -e '\033[0;32m✓ ws_navigation_tests done\033[0m' && \
	echo -e '\033[1m\033[0;36m▶ Running workspace cross-module tests\033[0m' && \
	cargo test --profile ci -p basilisk-lsp --test ws_test_cross_module && \
	echo -e '\033[0;32m✓ ws_test_cross_module done\033[0m' && \
	echo -e '\033[1m\033[0;36m▶ Running Zed extension tests\033[0m' && \
	cargo test --profile ci -p basilisk-lsp --test zed_tests && \
	echo -e '\033[0;32m✓ zed_tests done\033[0m'

# ── Package ───────────────────────────────────────────────────────────────────

.PHONY: package package-vsix package-zed

package: package-vsix package-zed ## Package all extensions

package-vsix: ## Package VS Code extension as VSIX
	@echo -e '\033[1m\033[0;36m▶ Packaging VSIX\033[0m' && \
	cd $(EXTENSION_DIR) && npm ci && npm run package && \
	echo -e '\033[0;32m✓ VSIX built\033[0m'

package-zed: ## Build CLI binary for Zed extension
	@echo -e '\033[1m\033[0;36m▶ Building basilisk CLI for Zed\033[0m' && \
	cargo install --path crates/basilisk-cli --force && \
	echo "$$(which basilisk) installed" && \
	echo "" && \
	echo "Now reinstall the dev extension in Zed:" && \
	echo "  Cmd+Shift+P -> 'zed: install dev extension'" && \
	echo "  Select: $(ZED_DIR)"

# ── Install ───────────────────────────────────────────────────────────────────

.PHONY: install install-rust install-vsix

install: install-rust install-vsix ## Build and install everything

install-rust: build-rust ## Install basilisk binary to ~/.cargo/bin
	@echo -e '\033[1m\033[0;36m▶ Installing basilisk\033[0m' && \
	cargo install --path crates/basilisk-cli --force && \
	echo -e "\033[0;32m✓ $$(which basilisk)\033[0m" && \
	basilisk --version

install-vsix: package-vsix ## Install VSIX into VS Code
	@echo -e '\033[1m\033[0;36m▶ Installing VSIX into VS Code\033[0m' && \
	VSIX=$$(ls $(EXTENSION_DIR)/*.vsix | head -1) && \
	code --install-extension "$$VSIX" --force && \
	echo -e "\033[0;32m✓ $$VSIX\033[0m" && \
	echo "Reload VS Code (Cmd+Shift+P → Developer: Reload Window) to activate."

# ── Setup ─────────────────────────────────────────────────────────────────────

.PHONY: setup

setup: ## Install all build/test dependencies
	@bash scripts/setup.sh

# ── Benchmark ─────────────────────────────────────────────────────────────────

.PHONY: benchmark

benchmark: build-rust ## Run benchmarks (RULE=e0034 to filter)
	@RULE='$(RULE)' bash scripts/benchmark.sh

# ── Examples ──────────────────────────────────────────────────────────────────

.PHONY: example-all example-good example-bad example-mixed

example-all: ## Check all example files
	@cargo run -- check examples/

example-good: ## Check examples/good.py (expects clean pass)
	@cargo run -- check examples/good.py

example-bad: ## Check examples/bad.py (expects errors)
	@cargo run -- check examples/bad.py

example-mixed: ## Check examples/mixed.py (expects some errors)
	@cargo run -- check examples/mixed.py

# ── Clean ─────────────────────────────────────────────────────────────────────

.PHONY: clean

clean: ## Remove build artifacts
	@echo -e '\033[1m\033[0;36m▶ Cleaning build artifacts\033[0m' && \
	cargo clean && \
	rm -rf $(EXTENSION_DIR)/out $(EXTENSION_DIR)/*.vsix && \
	rm -f lcov.info && \
	echo -e '\033[0;32m✓ Clean complete\033[0m'

# ── Help ──────────────────────────────────────────────────────────────────────

.PHONY: help

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'
