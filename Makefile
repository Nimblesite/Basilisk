# agent-pmo:2efd847
# =============================================================================
# Standard Makefile — Basilisk
# Cross-platform: Linux, macOS, Windows (via GNU Make)
# Exactly 7 standard targets: build, test, lint, fmt, clean, ci, setup
# =============================================================================

.PHONY: build test lint fmt clean ci setup conformance package-vsix install-binaries mutation-test

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
MUTATION_TEST_PACKAGE := basilisk-checker
MUTATION_TEST_MARKER  := mutation_safe
ALL           ?= 0
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

mutation-test: ## Run mutation-safe tests by default. Use ALL=1 for the full checker suite.
	@bash -euo pipefail -c '\
		package="$(MUTATION_TEST_PACKAGE)"; \
		marker="$(MUTATION_TEST_MARKER)"; \
		mutation_rustflags="$${RUSTFLAGS:-}"; \
		mode="working"; \
		test_filter="$$marker"; \
		examine_re=""; \
		if [ "$(ALL)" = "1" ]; then \
			mode="all"; \
			test_filter=""; \
			examine_re="."; \
		else \
			mutation_rustflags="$${mutation_rustflags:+$$mutation_rustflags }--cfg mutation_testing"; \
			tests_file="$$(mktemp)"; \
			RUSTFLAGS="$$mutation_rustflags" cargo test --package "$$package" "$$marker" -- --list > "$$tests_file"; \
			examine_re="$$(python3 -c '"'"'import pathlib,sys; prefix="mutation_safe_"; rules=[]; [rules.append(code) for line in pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines() for index in [line.find(prefix)] if index >= 0 for code in [line[index + len(prefix):index + len(prefix) + 5]] if len(code) == 5 and code[0] == "e" and code[1:].isdigit() and code not in rules]; sys.exit("no mutation-safe tests found; add #[mutation_safe(rule = \"eNNNN\")]") if not rules else print(r"rules/(" + "|".join(rules) + r")[.]rs")'"'"' "$$tests_file")"; \
			rm -f "$$tests_file"; \
		fi; \
		echo -e "\033[1m\033[0;36m▶ Mutation testing ($$mode): $$package\033[0m"; \
		echo -e "\033[0;36m  [diag] Tests: $${test_filter:-all}\033[0m"; \
		echo -e "\033[0;36m  [diag] Mutants: $$examine_re\033[0m"; \
		if [ -n "$$test_filter" ]; then \
			RUSTFLAGS="$$mutation_rustflags" cargo test --package "$$package" "$$test_filter"; \
		else \
			RUSTFLAGS="$$mutation_rustflags" cargo test --package "$$package"; \
		fi; \
		rm -rf "$(MUTATION_DIR)/mutants.out.$$mode".*; \
		mutants_file="$$(mktemp)"; \
		RUSTFLAGS="$$mutation_rustflags" cargo mutants --list --package "$$package" --re "$$examine_re" --exclude-re "src/inference" > "$$mutants_file"; \
		total="$$(wc -l < "$$mutants_file" | tr -d " ")"; \
		if [ "$$total" -eq 0 ]; then \
			echo -e "\033[0;31m✗ No mutants selected\033[0m"; \
			rm -f "$$mutants_file"; \
			exit 1; \
		fi; \
		i=0; \
		while IFS= read -r mutant; do \
			i=$$((i + 1)); \
			exact_re="$$(python3 -c "import re, sys; print(\"^\" + re.escape(sys.argv[1]) + \"$$\")" "$$mutant")"; \
			out_dir="$(MUTATION_DIR)/mutants.out.$$mode.$$(printf "%04d" "$$i")"; \
			echo -e "\033[1m\033[0;36m▶ Mutant $$i/$$total\033[0m"; \
			echo -e "\033[0;36m  [diag] $$mutant\033[0m"; \
			if [ -n "$$test_filter" ]; then \
				RUSTFLAGS="$$mutation_rustflags" cargo mutants --jobs 1 --timeout 30 --baseline skip --package "$$package" --re "$$exact_re" --exclude-re "src/inference" --cargo-test-arg "$$test_filter" --output "$$out_dir"; \
			else \
				RUSTFLAGS="$$mutation_rustflags" cargo mutants --jobs 1 --timeout 30 --baseline skip --package "$$package" --re "$$exact_re" --exclude-re "src/inference" --output "$$out_dir"; \
			fi; \
		done < "$$mutants_file"; \
		rm -f "$$mutants_file"; \
		echo -e "\033[0;32m✓ Mutation testing complete: $$total mutants checked\033[0m"; \
	'

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
