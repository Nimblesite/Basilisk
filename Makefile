SHELL := /bin/bash
.SHELLFLAGS := -euo pipefail -c
.ONESHELL:
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

# Terminal colors
RED    := \033[0;31m
GREEN  := \033[0;32m
YELLOW := \033[1;33m
CYAN   := \033[0;36m
BOLD   := \033[1m
RESET  := \033[0m

# Locate the basilisk binary across known build paths.
define find-or-build-basilisk
if [[ -z "$${BASILISK_BIN:-}" ]] || [[ ! -x "$${BASILISK_BIN:-}" ]]; then
	BASILISK_BIN=""
	for c in target/llvm-cov-target/ci/basilisk target/ci/basilisk \
		target/llvm-cov-target/release/basilisk target/release/basilisk \
		target/debug/basilisk; do
		if [[ -x "$$c" ]]; then BASILISK_BIN="$$c"; break; fi
	done
fi
if [[ -z "$${BASILISK_BIN:-}" ]]; then
	echo -e '$(BOLD)$(CYAN)▶ Building basilisk binary$(RESET)'
	cargo build --profile ci
	BASILISK_BIN="target/ci/basilisk"
fi
if [[ ! -x "$$BASILISK_BIN" ]]; then
	echo -e '$(RED)$(BOLD)FATAL: basilisk binary not found.$(RESET)'
	exit 1
fi
echo -e '$(GREEN)✓ basilisk binary: '"$$BASILISK_BIN"'$(RESET)'
endef

# ── Build ─────────────────────────────────────────────────────────────────────

.PHONY: build build-rust build-vsix

build: build-rust build-vsix ## Build all artifacts

build-rust: ## Build Rust workspace (release)
	@echo -e '$(BOLD)$(CYAN)▶ Building Rust (release)$(RESET)'
	cargo build --release
	echo -e '$(GREEN)✓ Rust build complete$(RESET)'

build-vsix: ## Build VS Code extension
	@echo -e '$(BOLD)$(CYAN)▶ Building VS Code extension$(RESET)'
	cd $(EXTENSION_DIR) && npm ci && npm run compile
	echo -e '$(GREEN)✓ VS Code extension compiled$(RESET)'

# ── Lint ──────────────────────────────────────────────────────────────────────

.PHONY: lint lint-rust lint-vsix

lint: lint-rust lint-vsix ## Lint all languages

lint-rust: ## Lint Rust (clippy + fmt)
	@echo -e '$(BOLD)$(CYAN)▶ Linting Rust$(RESET)'
	cargo clippy --workspace --all-targets -- -D warnings
	cargo fmt --all -- --check
	echo -e '$(GREEN)✓ Rust lint passed$(RESET)'

lint-vsix: ## Lint VS Code extension (ESLint)
	@echo -e '$(BOLD)$(CYAN)▶ Linting VS Code extension$(RESET)'
	cd $(EXTENSION_DIR) && npm run lint
	echo -e '$(GREEN)✓ VS Code lint passed$(RESET)'

# ── Format ───────────────────────────────────────────────────────────────────

.PHONY: format format-rust format-python format-vsix

format: format-rust format-python format-vsix ## Format all code

format-rust: ## Format Rust code
	@echo -e '$(BOLD)$(CYAN)▶ Formatting Rust$(RESET)'
	cargo fmt --all
	echo -e '$(GREEN)✓ Rust formatted$(RESET)'

format-python: ## Format Python code (ruff)
	@echo -e '$(BOLD)$(CYAN)▶ Formatting Python$(RESET)'
	ruff format --exclude '*/fixtures/*' .
	ruff check --fix --exclude '*/fixtures/*' .
	echo -e '$(GREEN)✓ Python formatted$(RESET)'

format-vsix: ## Format VS Code extension (ESLint --fix)
	@echo -e '$(BOLD)$(CYAN)▶ Formatting VS Code extension$(RESET)'
	cd $(EXTENSION_DIR) && npm run lint:fix
	echo -e '$(GREEN)✓ VS Code extension formatted$(RESET)'

# ── Test ──────────────────────────────────────────────────────────────────────

.PHONY: test test-rust test-vsix test-nvim test-zed test-compiler test-lsp audit

test: audit ## Run full test suite (Rust first, then extensions in parallel)
	@$(MAKE) --no-print-directory test-rust
	$(MAKE) --no-print-directory -j3 test-vsix test-nvim test-zed
	echo -e '\n$(GREEN)✓ All tests passed.$(RESET)'

audit: ## Check all required build/test dependencies
	@echo -e '$(BOLD)$(CYAN)▶ Auditing dependencies$(RESET)'
	MISSING=0
	require_cmd() {
		if ! command -v "$$1" &>/dev/null; then
			echo -e "  $(RED)✗ MISSING: $$1 — $$2$(RESET)"; MISSING=1
		else
			echo -e "  $(GREEN)✓ $$1$(RESET)"
		fi
	}
	require_py() {
		if ! python3 -c "import $$1" 2>/dev/null; then
			echo -e "  $(RED)✗ MISSING: Python module '$$1' — $$2$(RESET)"; MISSING=1
		else
			echo -e "  $(GREEN)✓ python3 -c 'import $$1'$(RESET)"
		fi
	}
	require_cmd cargo          "Install Rust: https://rustup.rs"
	require_cmd cargo-llvm-cov "Install: cargo install cargo-llvm-cov"
	require_cmd node           "Install Node.js 20+: https://nodejs.org"
	require_cmd npm            "Bundled with Node.js"
	require_cmd python3        "Install Python 3.12: https://python.org"
	require_cmd ruff           "Install: pip install ruff"
	require_cmd nvim           "Install Neovim 0.10+: https://neovim.io"
	require_py  debugpy        "Install: pip install debugpy"
	if [[ "$$MISSING" -ne 0 ]]; then
		echo -e '\n$(RED)$(BOLD)FATAL: Missing dependencies above.$(RESET)'
		exit 1
	fi
	echo -e '$(GREEN)✓ All dependencies present$(RESET)'

test-rust: ## Run Rust tests with coverage + thresholds (OPEN=1 for report)
	@echo -e '\n$(BOLD)$(CYAN)▶ Running tests with coverage instrumentation$(RESET)'
	rustup component add llvm-tools-preview 2>/dev/null || true
	set +e
	cargo llvm-cov --profile ci --workspace --exclude basilisk-compiler \
		--all-targets --lcov --output-path lcov.info
	TESTS_EXIT=$$?
	set -e
	echo -e '$(GREEN)✓ lcov.info written$(RESET)'
	if [[ "$$TESTS_EXIT" -ne 0 ]]; then
		echo -e '\n$(RED)$(BOLD)TESTS FAILED (exit '"$$TESTS_EXIT"').$(RESET)'
		echo -e '$(RED)Fix every failure. Nothing else runs until tests pass.$(RESET)'
		exit "$$TESTS_EXIT"
	fi
	echo -e '$(GREEN)✓ All workspace tests passed$(RESET)'
	# Locate binary
	$(find-or-build-basilisk)
	cargo llvm-cov report --profile ci --html --output-dir target/llvm-cov/html
	echo -e '$(GREEN)✓ HTML report → target/llvm-cov/html/index.html$(RESET)'
	# Summary
	echo -e '\n$(BOLD)$(CYAN)▶ Coverage summary$(RESET)'
	REPORT=$$(cargo llvm-cov report --profile ci 2>&1)
	echo "$$REPORT"
	echo -e '\n$(BOLD)VSCode:$(RESET) install Coverage Gutters, then $(CYAN)Coverage Gutters: Watch$(RESET).\n'
	if [[ "$(OPEN)" == "1" ]]; then
		open target/llvm-cov/html/index.html 2>/dev/null \
			|| xdg-open target/llvm-cov/html/index.html 2>/dev/null || true
	fi
	# Per-crate thresholds
	echo -e '$(BOLD)$(CYAN)▶ Enforcing per-project coverage thresholds$(RESET)'
	COV_FAILED=0
	HTML_ROWS=""
	check_crate() {
		local crate="$$1" threshold="$$2" totals total_lines missed_lines covered pct
		totals=$$(echo "$$REPORT" | grep "/$$crate/" \
			| awk '{total+=$$8; missed+=$$9} END {print total, missed}')
		total_lines=$$(echo "$$totals" | awk '{print $$1}')
		missed_lines=$$(echo "$$totals" | awk '{print $$2}')
		if [[ -z "$$total_lines" ]] || [[ "$$total_lines" -eq 0 ]]; then
			echo -e "  $(RED)✗ $$crate: NO COVERAGE DATA — FAIL$(RESET)"
			COV_FAILED=1
			HTML_ROWS+="<tr class='fail'><td>$$crate</td><td>NO DATA</td><td>$$threshold%</td><td>FAIL</td></tr>"
			return
		fi
		covered=$$((total_lines - missed_lines))
		pct=$$((covered * 100 / total_lines))
		if [[ "$$pct" -lt "$$threshold" ]]; then
			echo -e "  $(RED)✗ $$crate: $${pct}% < $${threshold}% — FAIL$(RESET)"
			COV_FAILED=1
			HTML_ROWS+="<tr class='fail'><td>$$crate</td><td>$${pct}%</td><td>$$threshold%</td><td>FAIL</td></tr>"
		else
			echo -e "  $(GREEN)✓ $$crate: $${pct}% ≥ $${threshold}%$(RESET)"
			HTML_ROWS+="<tr class='pass'><td>$$crate</td><td>$${pct}%</td><td>$$threshold%</td><td>PASS</td></tr>"
		fi
	}
	check_crate basilisk-checker  $(TEST_COVERAGE_BASILISK_CHECKER)
	check_crate basilisk-cli      $(TEST_COVERAGE_BASILISK_CLI)
	check_crate basilisk-db       $(TEST_COVERAGE_BASILISK_DB)
	check_crate basilisk-lsp      $(TEST_COVERAGE_BASILISK_LSP)
	check_crate basilisk-mojo     $(TEST_COVERAGE_BASILISK_MOJO)
	check_crate basilisk-parser   $(TEST_COVERAGE_BASILISK_PARSER)
	check_crate basilisk-plugin   $(TEST_COVERAGE_BASILISK_PLUGIN)
	check_crate basilisk-resolver $(TEST_COVERAGE_BASILISK_RESOLVER)
	check_crate basilisk-stubs    $(TEST_COVERAGE_BASILISK_STUBS)
	check_crate basilisk-config   $(TEST_COVERAGE_BASILISK_CONFIG)
	mkdir -p target/llvm-cov/html/html
	cat > target/llvm-cov/html/html/crates.html <<-CRATE_HTML
	<!DOCTYPE html><html><head><meta charset="utf-8"><title>Basilisk Crate Coverage</title>
	<style>body{font-family:monospace;background:#1e1e1e;color:#d4d4d4;padding:2rem}
	h1{color:#fff}table{border-collapse:collapse;width:100%;margin-top:1rem}
	th{background:#2d2d2d;color:#9cdcfe;padding:.5rem 1rem;text-align:left;border-bottom:2px solid #444}
	td{padding:.4rem 1rem;border-bottom:1px solid #333}
	tr.pass td:last-child{color:#4ec9b0;font-weight:bold}
	tr.fail td:last-child{color:#f44747;font-weight:bold}</style></head>
	<body><h1>Crate Coverage Summary</h1>
	<p>Generated: $$(date '+%Y-%m-%d %H:%M') | <a href="index.html">Full report</a></p>
	<table><thead><tr><th>Crate</th><th>Coverage</th><th>Threshold</th><th>Status</th></tr></thead>
	<tbody>$${HTML_ROWS}</tbody></table></body></html>
	CRATE_HTML
	echo -e '$(GREEN)✓ Crate summary generated$(RESET)'
	if [[ "$$COV_FAILED" -ne 0 ]]; then
		echo -e '\n$(RED)Coverage regression detected.$(RESET)'
		exit 1
	fi
	echo -e '\n$(GREEN)✓ All projects meet their coverage thresholds.$(RESET)'

test-vsix: ## Run VS Code extension tests + coverage threshold
	@$(find-or-build-basilisk)
	echo -e '\n$(BOLD)$(CYAN)▶ VS Code extension — compile + test$(RESET)'
	cd $(EXTENSION_DIR)
	npm ci
	npm run compile
	echo -e '$(GREEN)✓ TypeScript compiled$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ VS Code extension — ESLint$(RESET)'
	npm run lint
	echo -e '$(GREEN)✓ ESLint passed$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ VS Code E2E tests$(RESET)'
	VSCODE_TEST_CMD="npm test -- --coverage"
	if [[ -z "$${DISPLAY:-}" ]] && command -v xvfb-run &>/dev/null; then
		VSCODE_TEST_CMD="xvfb-run -a npm test -- --coverage"
	fi
	BASILISK_EXECUTABLE_PATH="$$BASILISK_BIN" MOCHA_TIMEOUT="120000" $$VSCODE_TEST_CMD
	echo -e '$(GREEN)✓ VS Code E2E tests done$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ VS Code extension — coverage threshold$(RESET)'
	VSIX_LCOV="coverage/lcov.info"
	if [[ -f "$$VSIX_LCOV" ]]; then
		vsix_total=$$(grep -c "^DA:" "$$VSIX_LCOV" || true)
	else
		vsix_total=0
	fi
	if [[ "$$vsix_total" -eq 0 ]]; then
		echo -e "  $(RED)$(BOLD)✗ vscode-extension: no LCOV data — FAIL$(RESET)"
		exit 1
	fi
	vsix_covered=$$(grep -c "^DA:[^,]*,[^0]" "$$VSIX_LCOV" || true)
	vsix_pct=$$((vsix_covered * 100 / vsix_total))
	if [[ "$$vsix_pct" -lt "$(TEST_COVERAGE_VSIX)" ]]; then
		echo -e "  $(RED)✗ vscode-extension: $${vsix_pct}% < $(TEST_COVERAGE_VSIX)% — FAIL$(RESET)"
		exit 1
	fi
	echo -e "  $(GREEN)✓ vscode-extension: $${vsix_pct}% ≥ $(TEST_COVERAGE_VSIX)%$(RESET)"

test-nvim: ## Run Neovim extension e2e + screenshot tests
	@$(find-or-build-basilisk)
	export BASILISK_EXECUTABLE_PATH="$$BASILISK_BIN"
	echo -e '$(BOLD)$(CYAN)▶ Checking Neovim dependencies$(RESET)'
	command -v pytest &>/dev/null || { echo -e '$(RED)FATAL: pytest not found$(RESET)'; exit 1; }
	echo -e '$(GREEN)✓ pytest$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ Neovim extension — real LSP e2e tests$(RESET)'
	cd $(NVIM_DIR)
	[[ -d /tmp/plenary.nvim ]] || git clone --depth 1 https://github.com/nvim-lua/plenary.nvim /tmp/plenary.nvim
	[[ -d /tmp/nvim-dap ]]     || git clone --depth 1 https://github.com/mfussenegger/nvim-dap /tmp/nvim-dap
	[[ -d /tmp/mini.nvim ]]    || git clone --depth 1 https://github.com/echasnovski/mini.nvim /tmp/mini.nvim
	if command -v nvim &>/dev/null; then
		rm -f luacov.stats.out luacov.report.out
		NVIM_LOG=$$(mktemp)
		LUACOV=1 nvim --headless -u tests/minimal_init.lua \
			-c "PlenaryBustedDirectory tests/lsp {minimal_init = 'tests/minimal_init.lua', sequential = true}" 2>&1 \
			| tee "$$NVIM_LOG" || true
		if grep -q "^Failed .*[1-9]" "$$NVIM_LOG" || grep -q "^Errors .*[1-9]" "$$NVIM_LOG"; then \
			echo -e '$(RED)$(BOLD)✗ Neovim LSP e2e tests FAILED$(RESET)'; rm -f "$$NVIM_LOG"; exit 1; \
		fi
		rm -f "$$NVIM_LOG"
		echo -e '$(GREEN)✓ Neovim LSP e2e tests passed$(RESET)'
		LUACOV=1 nvim --headless -u tests/minimal_init.lua \
			-l tests/ui/run_screenshots.lua 2>&1
		echo -e '$(GREEN)✓ Neovim screenshot regression tests passed$(RESET)'
	else
		echo -e '$(YELLOW)⚠ nvim not found — skipping$(RESET)'
	fi
	# Coverage (local only)
	if [[ -n "$${CI:-}" ]]; then
		echo -e '  $(YELLOW)⊘ neovim: coverage check skipped on CI$(RESET)'
	else
		echo -e '$(BOLD)$(CYAN)▶ Neovim extension — coverage threshold$(RESET)'
		if [[ ! -f luacov.stats.out ]]; then
			echo -e "  $(RED)$(BOLD)✗ neovim: no luacov stats — FAIL$(RESET)"; exit 1
		fi
		nvim --headless --noplugin -l tests/generate_report.lua 2>&1
		if [[ ! -f luacov.report.out ]]; then
			echo -e "  $(RED)$(BOLD)✗ neovim: report generation failed — FAIL$(RESET)"; exit 1
		fi
		echo "  luacov report summary:"
		awk '/^=+$$/{s=1} s{print "    "$$0}' luacov.report.out | tail -20
		nvim_pct=$$(awk '/^Total/ { gsub(/%/, "", $$NF); printf "%d", $$NF }' luacov.report.out)
		if [[ -z "$$nvim_pct" ]] || [[ "$$nvim_pct" -eq 0 ]]; then
			echo -e "  $(RED)$(BOLD)✗ neovim: could not parse coverage — FAIL$(RESET)"; exit 1
		fi
		if [[ "$$nvim_pct" -lt "$(TEST_COVERAGE_NVIM)" ]]; then
			echo -e "  $(RED)✗ neovim: $${nvim_pct}% < $(TEST_COVERAGE_NVIM)% — FAIL$(RESET)"; exit 1
		fi
		echo -e "  $(GREEN)✓ neovim: $${nvim_pct}% ≥ $(TEST_COVERAGE_NVIM)%$(RESET)"
	fi

test-zed: ## Run Zed extension tests
	@echo -e '$(BOLD)$(CYAN)▶ Zed extension — tests$(RESET)'
	cd $(ZED_DIR) && cargo test --profile ci --all-targets
	echo -e '$(GREEN)✓ Zed extension done$(RESET)'

test-compiler: ## Run compiler E2E tests
	@echo -e '$(BOLD)$(CYAN)▶ Running Basilisk compiler E2E tests$(RESET)'
	cargo test --profile ci -p basilisk-compiler --test e2e_tests -- --nocapture
	echo -e '$(GREEN)✓ All compiler E2E tests passed$(RESET)'

test-lsp: ## Run LSP integration tests (slow, not in main suite)
	@echo -e '$(BOLD)$(CYAN)▶ Running LSP stdio tests$(RESET)'
	cargo test --profile ci -p basilisk-lsp --test lsp_stdio_tests
	echo -e '$(GREEN)✓ lsp_stdio_tests done$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ Running workspace core tests$(RESET)'
	cargo test --profile ci -p basilisk-lsp --test ws_core_tests
	echo -e '$(GREEN)✓ ws_core_tests done$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ Running workspace features tests$(RESET)'
	cargo test --profile ci -p basilisk-lsp --test ws_features_tests
	echo -e '$(GREEN)✓ ws_features_tests done$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ Running workspace navigation tests$(RESET)'
	cargo test --profile ci -p basilisk-lsp --test ws_navigation_tests
	echo -e '$(GREEN)✓ ws_navigation_tests done$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ Running workspace cross-module tests$(RESET)'
	cargo test --profile ci -p basilisk-lsp --test ws_test_cross_module
	echo -e '$(GREEN)✓ ws_test_cross_module done$(RESET)'
	echo -e '$(BOLD)$(CYAN)▶ Running Zed extension tests$(RESET)'
	cargo test --profile ci -p basilisk-lsp --test zed_tests
	echo -e '$(GREEN)✓ zed_tests done$(RESET)'

# ── Package ───────────────────────────────────────────────────────────────────

.PHONY: package package-vsix package-zed

package: package-vsix package-zed ## Package all extensions

package-vsix: ## Package VS Code extension as VSIX
	@echo -e '$(BOLD)$(CYAN)▶ Packaging VSIX$(RESET)'
	cd $(EXTENSION_DIR) && npm ci && npm run package
	echo -e '$(GREEN)✓ VSIX built$(RESET)'

package-zed: ## Build CLI binary for Zed extension
	@echo -e '$(BOLD)$(CYAN)▶ Building basilisk CLI for Zed$(RESET)'
	cargo install --path crates/basilisk-cli --force
	echo -e "$$(which basilisk) installed"
	echo ""
	echo "Now reinstall the dev extension in Zed:"
	echo "  Cmd+Shift+P -> 'zed: install dev extension'"
	echo "  Select: $(ZED_DIR)"

# ── Install ───────────────────────────────────────────────────────────────────

.PHONY: install install-rust install-vsix

install: install-rust install-vsix ## Build and install everything

install-rust: build-rust ## Install basilisk binary to ~/.cargo/bin
	@echo -e '$(BOLD)$(CYAN)▶ Installing basilisk$(RESET)'
	cargo install --path crates/basilisk-cli --force
	echo -e "$(GREEN)✓ $$(which basilisk)$(RESET)"
	basilisk --version

install-vsix: package-vsix ## Install VSIX into VS Code
	@echo -e '$(BOLD)$(CYAN)▶ Installing VSIX into VS Code$(RESET)'
	VSIX=$$(ls $(EXTENSION_DIR)/*.vsix | head -1)
	code --install-extension "$$VSIX" --force
	echo -e "$(GREEN)✓ $$VSIX$(RESET)"
	echo "Reload VS Code (Cmd+Shift+P → Developer: Reload Window) to activate."

# ── Setup ─────────────────────────────────────────────────────────────────────

.PHONY: setup

setup: ## Install all build/test dependencies
	@echo -e '$(BOLD)$(CYAN)▶ Checking required tools$(RESET)'
	command -v python3 &>/dev/null || { echo -e '$(RED)✗ python3 not found$(RESET)'; exit 1; }
	echo -e '$(GREEN)✓ python3$(RESET)'
	command -v rustup &>/dev/null || { echo -e '$(RED)✗ rustup not found$(RESET)'; exit 1; }
	echo -e '$(GREEN)✓ rustup$(RESET)'
	command -v cargo &>/dev/null || { echo -e '$(RED)✗ cargo not found$(RESET)'; exit 1; }
	echo -e '$(GREEN)✓ cargo$(RESET)'
	echo -e '\n$(BOLD)$(CYAN)▶ Installing Rust toolchain components$(RESET)'
	rustup component list --installed | grep -q llvm-tools || rustup component add llvm-tools
	echo -e '$(GREEN)✓ llvm-tools$(RESET)'
	cargo llvm-cov --version &>/dev/null || cargo install cargo-llvm-cov --locked
	echo -e '$(GREEN)✓ cargo-llvm-cov$(RESET)'
	echo -e '\n$(BOLD)$(CYAN)▶ Installing Python packages$(RESET)'
	python3 -c 'import debugpy' &>/dev/null \
		|| python3 -m pip install --quiet --break-system-packages debugpy
	echo -e '$(GREEN)✓ debugpy$(RESET)'
	echo -e '\n$(BOLD)$(CYAN)▶ Installing Node dependencies$(RESET)'
	if command -v node &>/dev/null && command -v npm &>/dev/null; then
		cd $(EXTENSION_DIR) && npm ci
		echo -e '$(GREEN)✓ vscode-extension node_modules$(RESET)'
	else
		echo -e '$(YELLOW)⚠ node/npm not found — skipping VS Code extension deps$(RESET)'
	fi
	echo -e '\n$(GREEN)✓ All dependencies installed.$(RESET)'

# ── Benchmark ─────────────────────────────────────────────────────────────────

.PHONY: benchmark

benchmark: build-rust ## Run benchmarks (RULE=e0034 to filter)
	@[[ -x target/release/basilisk ]] || { echo "Binary not found" >&2; exit 1; }
	command -v hyperfine &>/dev/null || { echo "hyperfine required (brew install hyperfine)" >&2; exit 1; }
	mkdir -p benchmarks/results
	RULE_FILTER="$$(echo '$(RULE)' | tr '[:upper:]' '[:lower:]')"
	FIXTURES=(
		"e0002_missing_return.py:E0002 Missing return annotations"
		"e0016_incompatible_override.py:E0016 Incompatible override"
		"e0022_unhashable_dict_key.py:E0022 Unhashable dict key"
		"e0023_nonexhaustive_match.py:E0023 Non-exhaustive match"
		"e0026_typevar_single_constraint.py:E0026 TypeVar single constraint"
		"e0054_final_reassignment.py:E0054 Final reassignment"
	)
	echo "Running benchmarks..."
	for entry in "$${FIXTURES[@]}"; do
		FILE="$${entry%%:*}"; LABEL="$${entry##*:}"
		if [[ -n "$$RULE_FILTER" ]]; then
			LF="$$(echo "$$FILE" | tr '[:upper:]' '[:lower:]')"
			LL="$$(echo "$$LABEL" | tr '[:upper:]' '[:lower:]')"
			[[ "$$LF" == *"$$RULE_FILTER"* || "$$LL" == *"$$RULE_FILTER"* ]] || continue
		fi
		FPATH="benchmarks/fixtures/$$FILE"
		[[ -f "$$FPATH" ]] || continue
		hyperfine --warmup 2 --runs 10 --ignore-failure \
			--export-json "benchmarks/results/$${FILE%.py}.json" \
			--command-name basilisk "target/release/basilisk check $$FPATH >/dev/null 2>&1" \
			--command-name pyright  "python3 -m pyright $$FPATH >/dev/null 2>&1" \
			--command-name mypy     "python3 -m mypy --ignore-missing-imports --no-error-summary $$FPATH >/dev/null 2>&1" \
			--command-name pyrefly  "pyrefly check $$FPATH >/dev/null 2>&1" \
			--command-name ty       "python3 -m ty check $$FPATH >/dev/null 2>&1" \
			> /dev/null 2>&1
		printf '  ✓ %s\n' "$$LABEL"
	done
	python3 scripts/benchmark_report.py benchmarks/results "$$RULE_FILTER"

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
	@echo -e '$(BOLD)$(CYAN)▶ Cleaning build artifacts$(RESET)'
	cargo clean
	rm -rf $(EXTENSION_DIR)/out $(EXTENSION_DIR)/*.vsix
	rm -f lcov.info
	echo -e '$(GREEN)✓ Clean complete$(RESET)'

# ── Help ──────────────────────────────────────────────────────────────────────

.PHONY: help

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'
