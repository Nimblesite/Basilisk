# agent-pmo:2efd847
# =============================================================================
# Standard Makefile — Basilisk
# Cross-platform: Linux, macOS, Windows (via GNU Make)
# Exactly 7 public targets: build, test, lint, fmt, clean, ci, setup
# =============================================================================

.PHONY: build test lint fmt clean ci setup

# ---------------------------------------------------------------------------
# OS Detection
# ---------------------------------------------------------------------------
ifeq ($(OS),Windows_NT)
  SHELL := powershell.exe
  .SHELLFLAGS := -NoProfile -Command
  RM = Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
  MKDIR = New-Item -ItemType Directory -Force
  HOME ?= $(USERPROFILE)
else
  RM = rm -rf
  MKDIR = mkdir -p
endif

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
EXTENSION_DIR := vscode-extension
ZED_DIR       := basilisk-zed
NVIM_DIR      := basilisk.nvim
OPEN          ?= 0
RULE          ?=
COVERAGE_THRESHOLDS_FILE := coverage-thresholds.json

# =============================================================================
# Standard Targets
# =============================================================================

## build: Compile/assemble all artifacts
build: _build_rust _build_vsix

## test: Fail-fast tests + coverage + threshold enforcement.
##       See REPO-STANDARDS-SPEC [TEST-RULES] and [COVERAGE-THRESHOLDS-JSON].
test: _audit
	@$(MAKE) --no-print-directory _test_rust && \
	$(MAKE) --no-print-directory -j3 _test_vsix _test_nvim _test_zed && \
	echo -e '\n\033[0;32m✓ All tests passed.\033[0m'

## lint: Run all linters/analyzers (read-only). Does NOT format.
lint: _lint_rust _lint_vsix

## fmt: Format all code in-place
fmt: _fmt_rust _fmt_python _fmt_vsix

## clean: Remove all build artifacts
clean:
	@echo -e '\033[1m\033[0;36m▶ Cleaning build artifacts\033[0m' && \
	cargo clean && \
	$(RM) $(EXTENSION_DIR)/out $(EXTENSION_DIR)/*.vsix && \
	$(RM) lcov.info && \
	echo -e '\033[0;32m✓ Clean complete\033[0m'

## ci: lint + test + build (full CI simulation)
ci: lint test build

## setup: Post-create dev environment setup
setup:
	@bash scripts/setup.sh

# =============================================================================
# Repo-Specific Targets
# =============================================================================

conformance: ## Run PEP conformance test suite (--fetch to re-download)
	@bash scripts/conformance.sh $(if $(FETCH),--fetch,)

benchmark: _build_rust ## Run benchmarks (RULE=e0034 to filter)
	@RULE='$(RULE)' bash scripts/benchmark.sh

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

package: _package_vsix _package_zed ## Package all extensions

install: _install_rust _install_vsix ## Build and install everything

# =============================================================================
# Internal Recipes (private — not in .PHONY)
# =============================================================================

_build_rust:
	@echo -e '\033[1m\033[0;36m▶ Building Rust (release)\033[0m' && \
	cargo build --release && \
	echo -e '\033[0;32m✓ Rust build complete\033[0m'

_build_vsix:
	@echo -e '\033[1m\033[0;36m▶ Building VS Code extension\033[0m' && \
	cd $(EXTENSION_DIR) && npm ci && npm run compile && \
	echo -e '\033[0;32m✓ VS Code extension compiled\033[0m'

_lint_rust:
	@echo -e '\033[1m\033[0;36m▶ Linting Rust\033[0m' && \
	cargo check --workspace --all-targets && \
	cargo clippy --workspace --all-targets -- -D warnings && \
	echo -e '\033[0;32m✓ Rust lint passed\033[0m'

_lint_vsix:
	@echo -e '\033[1m\033[0;36m▶ Linting VS Code extension\033[0m' && \
	cd $(EXTENSION_DIR) && npm run lint && \
	echo -e '\033[0;32m✓ VS Code lint passed\033[0m'

_fmt_rust:
	@echo -e '\033[1m\033[0;36m▶ Formatting Rust\033[0m' && \
	cargo fmt --all && \
	echo -e '\033[0;32m✓ Rust formatted\033[0m'

_fmt_python:
	@echo -e '\033[1m\033[0;36m▶ Formatting Python\033[0m' && \
	ruff format --exclude '*/fixtures/*' . && \
	ruff check --fix --exclude '*/fixtures/*' . && \
	echo -e '\033[0;32m✓ Python formatted\033[0m'

_fmt_vsix:
	@echo -e '\033[1m\033[0;36m▶ Formatting VS Code extension\033[0m' && \
	cd $(EXTENSION_DIR) && npm run lint:fix && \
	echo -e '\033[0;32m✓ VS Code extension formatted\033[0m'

_audit:
	@bash scripts/audit.sh

_test_rust:
	@OPEN=$(OPEN) bash scripts/test-rust.sh

_test_vsix:
	@bash scripts/test-vscode.sh

_test_nvim:
	@bash scripts/test-nvim.sh

_test_zed:
	@bash scripts/test-zed.sh

_package_vsix:
	@echo -e '\033[1m\033[0;36m▶ Packaging VSIX\033[0m' && \
	cd $(EXTENSION_DIR) && npm ci && npm run package && \
	echo -e '\033[0;32m✓ VSIX built\033[0m'

_package_zed:
	@echo -e '\033[1m\033[0;36m▶ Building basilisk CLI for Zed\033[0m' && \
	cargo install --path crates/basilisk-cli --force && \
	echo "$$(which basilisk) installed" && \
	echo "" && \
	echo "Now reinstall the dev extension in Zed:" && \
	echo "  Cmd+Shift+P -> 'zed: install dev extension'" && \
	echo "  Select: $(ZED_DIR)"

_install_rust: _build_rust
	@echo -e '\033[1m\033[0;36m▶ Installing basilisk\033[0m' && \
	cargo install --path crates/basilisk-cli --force && \
	echo -e "\033[0;32m✓ $$(which basilisk)\033[0m" && \
	basilisk --version

_install_vsix: _package_vsix
	@echo -e '\033[1m\033[0;36m▶ Installing VSIX into VS Code\033[0m' && \
	VSIX=$$(ls $(EXTENSION_DIR)/*.vsix | head -1) && \
	code --install-extension "$$VSIX" --force && \
	echo -e "\033[0;32m✓ $$VSIX\033[0m" && \
	echo "Reload VS Code (Cmd+Shift+P → Developer: Reload Window) to activate."
