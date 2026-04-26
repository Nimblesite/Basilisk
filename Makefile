# agent-pmo:2efd847
# =============================================================================
# Standard Makefile — Basilisk
# Cross-platform: Linux, macOS, Windows (via GNU Make)
# Exactly 7 standard targets: build, test, lint, fmt, clean, ci, setup
# =============================================================================

.PHONY: build test lint fmt clean ci setup conformance package-vsix install-binaries
.PHONY: mutation-list mutation-list-working mutation-list-recorded-scores
.PHONY: mutation-run-group-fast mutation-run-group-small-crates mutation-run-group-all-crates
.PHONY: mutation-run-crate mutation-run-crate-stubs mutation-run-crate-db mutation-run-crate-config
.PHONY: mutation-run-crate-parser mutation-run-crate-mojo mutation-run-crate-checker mutation-run-crate-resolver
.PHONY: mutation-list-checker-groups mutation-run-checker-rule
.PHONY: mutation-run-checker-group-01 mutation-run-checker-group-02 mutation-run-checker-group-03
.PHONY: mutation-run-checker-group-04 mutation-run-checker-group-05 mutation-run-checker-group-06
.PHONY: mutation-run-checker-group-07 mutation-run-checker-group-08 mutation-run-checker-group-09
.PHONY: mutation-run-checker-group-10 mutation-run-checker-group-11 mutation-run-checker-group-12
.PHONY: mutation-run-checker-group-13 mutation-run-checker-group-14

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
MUTATION_DIR  := mutation_testing
MUTATION_CRATE ?=
MUTATION_RULE  ?= $(RULE)
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

package-vsix: _package_vsix ## Package the VS Code extension into a .vsix

install-binaries: ## Install all Basilisk binaries (basilisk, basilisk-profiler-helper, debugpy) to PATH
	@echo -e '\033[1m\033[0;36m▶ Installing basilisk binaries\033[0m' && \
	cargo install --path crates/basilisk-cli --force && \
	cargo install --path crates/basilisk-profiler-helper --force && \
	echo -e '\033[1m\033[0;36m▶ Installing debugpy\033[0m' && \
	python3 -m pip install --user --upgrade --break-system-packages debugpy && \
	echo -e "\033[0;32m✓ basilisk                 → $$(command -v basilisk)\033[0m" && \
	echo -e "\033[0;32m✓ basilisk-profiler-helper → $$(command -v basilisk-profiler-helper)\033[0m" && \
	echo -e "\033[0;32m✓ debugpy                  → $$(python3 -c 'import debugpy, os; print(os.path.dirname(debugpy.__file__))')\033[0m" && \
	basilisk --version

# =============================================================================
# Mutation Testing Targets
# =============================================================================

mutation-list: ## List mutation crates, groups, and per-crate options
	@bash $(MUTATION_DIR)/mutate.sh --list

mutation-list-working: ## Show verified mutation slices that can be expanded over time
	@sed -n '1,220p' $(MUTATION_DIR)/working_tests.md

mutation-list-recorded-scores: ## Show recorded mutation scores
	@column -s, -t < $(MUTATION_DIR)/mutation_scores.csv

mutation-run-group-fast: ## Run fast mutation crate targets only
	@$(MAKE) --no-print-directory mutation-run-crate-stubs
	@$(MAKE) --no-print-directory mutation-run-crate-db
	@$(MAKE) --no-print-directory mutation-run-crate-config
	@$(MAKE) --no-print-directory mutation-run-crate-parser
	@$(MAKE) --no-print-directory mutation-run-crate-mojo

mutation-run-group-small-crates: mutation-run-group-fast ## Run small mutation crate targets

mutation-run-group-all-crates: ## Run every mutation crate target; this is slow
	@$(MAKE) --no-print-directory mutation-run-group-fast
	@$(MAKE) --no-print-directory mutation-run-crate-checker
	@$(MAKE) --no-print-directory mutation-run-crate-resolver

mutation-run-crate: ## Run one mutation crate target (MUTATION_CRATE=checker|stubs|db|config|parser|mojo|resolver)
	$(if $(strip $(MUTATION_CRATE)),,$(error MUTATION_CRATE is required. Example: make mutation-run-crate MUTATION_CRATE=checker))
	@bash $(MUTATION_DIR)/mutate.sh --crate "$(MUTATION_CRATE)"

mutation-run-crate-stubs: ## Run basilisk-stubs mutation tests
	@bash $(MUTATION_DIR)/stubs.sh

mutation-run-crate-db: ## Run basilisk-db mutation tests
	@bash $(MUTATION_DIR)/db.sh

mutation-run-crate-config: ## Run basilisk-config mutation tests
	@bash $(MUTATION_DIR)/config.sh

mutation-run-crate-parser: ## Run basilisk-parser mutation tests
	@bash $(MUTATION_DIR)/parser.sh

mutation-run-crate-mojo: ## Run basilisk-mojo mutation tests
	@bash $(MUTATION_DIR)/mojo.sh

mutation-run-crate-checker: ## Run all basilisk-checker mutation groups
	@bash $(MUTATION_DIR)/checker.sh

mutation-run-crate-resolver: ## Run basilisk-resolver mutation tests; expensive
	@bash $(MUTATION_DIR)/resolver.sh

mutation-list-checker-groups: ## List checker mutation groups and their mutant counts
	@bash $(MUTATION_DIR)/checker.sh --list

mutation-run-checker-rule: ## Run one checker rule mutation slice (MUTATION_RULE=e0014 or RULE=e0014)
	$(if $(strip $(MUTATION_RULE)),,$(error MUTATION_RULE is required. Example: make mutation-run-checker-rule MUTATION_RULE=e0014))
	@bash $(MUTATION_DIR)/checker.sh --rule "$(MUTATION_RULE)"

mutation-run-checker-group-01: ## Run checker mutation group 01
	@bash $(MUTATION_DIR)/checker.sh --group 1

mutation-run-checker-group-02: ## Run checker mutation group 02
	@bash $(MUTATION_DIR)/checker.sh --group 2

mutation-run-checker-group-03: ## Run checker mutation group 03
	@bash $(MUTATION_DIR)/checker.sh --group 3

mutation-run-checker-group-04: ## Run checker mutation group 04
	@bash $(MUTATION_DIR)/checker.sh --group 4

mutation-run-checker-group-05: ## Run checker mutation group 05
	@bash $(MUTATION_DIR)/checker.sh --group 5

mutation-run-checker-group-06: ## Run checker mutation group 06
	@bash $(MUTATION_DIR)/checker.sh --group 6

mutation-run-checker-group-07: ## Run checker mutation group 07
	@bash $(MUTATION_DIR)/checker.sh --group 7

mutation-run-checker-group-08: ## Run checker mutation group 08
	@bash $(MUTATION_DIR)/checker.sh --group 8

mutation-run-checker-group-09: ## Run checker mutation group 09
	@bash $(MUTATION_DIR)/checker.sh --group 9

mutation-run-checker-group-10: ## Run checker mutation group 10
	@bash $(MUTATION_DIR)/checker.sh --group 10

mutation-run-checker-group-11: ## Run checker mutation group 11
	@bash $(MUTATION_DIR)/checker.sh --group 11

mutation-run-checker-group-12: ## Run checker mutation group 12
	@bash $(MUTATION_DIR)/checker.sh --group 12

mutation-run-checker-group-13: ## Run checker mutation group 13
	@bash $(MUTATION_DIR)/checker.sh --group 13

mutation-run-checker-group-14: ## Run checker mutation group 14
	@bash $(MUTATION_DIR)/checker.sh --group 14

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
	cd $(EXTENSION_DIR) && npm ci --silent && npm run lint && \
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

## test-vsix: Compile, lint, E2E-test, and coverage-gate the VS Code extension.
_test_vsix:
	@set -e; \
	REPO_ROOT="$$(pwd)"; \
	BASILISK_BIN=""; \
	for c in "$$REPO_ROOT/target/llvm-cov-target/ci/basilisk" \
	         "$$REPO_ROOT/target/ci/basilisk" \
	         "$$REPO_ROOT/target/llvm-cov-target/release/basilisk" \
	         "$$REPO_ROOT/target/release/basilisk" \
	         "$$REPO_ROOT/target/debug/basilisk"; do \
	    if [ -x "$$c" ]; then BASILISK_BIN="$$c"; break; fi; \
	done; \
	if [ -z "$$BASILISK_BIN" ]; then \
	    echo -e '\033[1m\033[0;36m▶ Building basilisk binary\033[0m'; \
	    cargo build --profile ci; \
	    BASILISK_BIN="$$REPO_ROOT/target/ci/basilisk"; \
	fi; \
	[ -x "$$BASILISK_BIN" ] || { echo -e '\033[0;31m✗ basilisk binary not found\033[0m'; exit 1; }; \
	echo -e "\033[0;32m✓ basilisk binary: $$BASILISK_BIN\033[0m"; \
	echo -e '\033[1m\033[0;36m▶ VS Code extension — compile\033[0m'; \
	cd $(EXTENSION_DIR) && npm ci && npm run compile; \
	echo -e '\033[1m\033[0;36m▶ VS Code extension — ESLint\033[0m'; \
	npm run lint; \
	echo -e '\033[1m\033[0;36m▶ VS Code E2E tests\033[0m'; \
	VSCODE_TEST_CMD="npm test -- --coverage"; \
	if [ -z "$${DISPLAY:-}" ] && command -v xvfb-run >/dev/null 2>&1; then \
	    VSCODE_TEST_CMD="xvfb-run -a $$VSCODE_TEST_CMD"; \
	fi; \
	BASILISK_EXECUTABLE_PATH="$$BASILISK_BIN" $$VSCODE_TEST_CMD; \
	echo -e '\033[1m\033[0;36m▶ VS Code extension — coverage threshold\033[0m'; \
	VSIX_LCOV="$$REPO_ROOT/$(EXTENSION_DIR)/coverage/lcov.info"; \
	VSIX_THRESHOLD=$$(python3 -c 'import json; print(json.load(open("'"$$REPO_ROOT"'/$(COVERAGE_THRESHOLDS_FILE)"))["projects"]["vsix"]["threshold"])'); \
	if [ ! -f "$$VSIX_LCOV" ]; then echo -e '\033[0;31m✗ vscode-extension: no LCOV data — coverage collection broken\033[0m'; exit 1; fi; \
	VSIX_TOTAL=$$(grep -c '^DA:' "$$VSIX_LCOV" || true); \
	if [ "$$VSIX_TOTAL" -eq 0 ]; then echo -e '\033[0;31m✗ vscode-extension: no LCOV data\033[0m'; exit 1; fi; \
	VSIX_COVERED=$$(grep -c '^DA:[^,]*,[^0]' "$$VSIX_LCOV" || true); \
	VSIX_PCT=$$(( VSIX_COVERED * 100 / VSIX_TOTAL )); \
	if [ "$$VSIX_PCT" -lt "$$VSIX_THRESHOLD" ]; then \
	    echo -e "\033[0;31m✗ vscode-extension: $$VSIX_PCT%% < $$VSIX_THRESHOLD%% threshold — FAIL\033[0m"; exit 1; \
	fi; \
	echo -e "\033[0;32m✓ vscode-extension: $$VSIX_PCT%% ≥ $$VSIX_THRESHOLD%% threshold\033[0m"

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
