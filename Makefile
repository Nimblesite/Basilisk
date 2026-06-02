# agent-pmo:74cf183
# =============================================================================
# Standard Makefile — Basilisk
# Cross-platform: Linux, macOS, Windows (via GNU Make)
# Exactly 7 standard targets: build, test, lint, fmt, clean, ci, setup
# =============================================================================

.PHONY: build test lint fmt clean ci setup mutation-test conformance bench reinstall-vsix reinstall-vsix-prerelease

# ---------------------------------------------------------------------------
# OS Detection
# ---------------------------------------------------------------------------
ifeq ($(OS),Windows_NT)
  SHELL := powershell.exe
  .SHELLFLAGS := -NoProfile -Command
  RM = Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
  HOME ?= $(USERPROFILE)
else
  RM = rm -rf
endif

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
_EXTENSION_DIR             := vscode-extension
_ZED_DIR                   := basilisk-zed
_NVIM_DIR                  := basilisk.nvim
_MUTATION_DIR              := mutation_testing
_MUTATION_TEST_PACKAGE     := basilisk-checker
_MUTATION_TEST_MARKER      := mutation_safe
_COVERAGE_THRESHOLDS_FILE  := coverage-thresholds.json
OPEN                       ?= 0
ALL                        ?= 0
SHARD                      ?=
MUTATION_CHECK             ?= auto

# =============================================================================
# Standard Targets
# =============================================================================

## build: Compile/assemble all artifacts
build: _build_rust _build_vsix

## test: Fail-fast tests + coverage + threshold enforcement.
test: _audit
	@$(MAKE) --no-print-directory _test_rust && \
	$(MAKE) --no-print-directory -j3 _test_vsix _test_nvim _test_zed && \
	echo -e '\n\033[0;32m✓ All tests passed.\033[0m'

## lint: Run all linters/analyzers (read-only). Does NOT format.
lint: _lint_rust _lint_vsix

## fmt: Format all code in-place
fmt: _fmt_rust _fmt_python _fmt_vsix

## clean: Remove all build artifacts
clean: _clean_rust _clean_vsix

## ci: lint + test + build (full CI simulation)
ci: lint test build

## setup: Post-create dev environment setup
setup:
	@bash scripts/setup.sh

# =============================================================================
# Repo-Specific Targets
# =============================================================================

## mutation-test: Run mutation-safe tests. Use ALL=1 for full checker suite.
mutation-test:
	@bash -euo pipefail -c '\
		package="$(_MUTATION_TEST_PACKAGE)"; \
		marker="$(_MUTATION_TEST_MARKER)"; \
		mutation_rustflags="$${RUSTFLAGS:-}"; \
		mode="working"; \
		test_filter="$$marker"; \
		examine_re=""; \
		shard="$(SHARD)"; \
		shard_arg=""; \
		mutation_check="$(MUTATION_CHECK)"; \
		if [ "$(ALL)" = "1" ]; then \
			mode="all"; \
			test_filter=""; \
			examine_re="."; \
		else \
			mutation_rustflags="$${mutation_rustflags:+$$mutation_rustflags }--cfg mutation_testing"; \
			tests_file="$$(mktemp)"; \
			RUSTFLAGS="$$mutation_rustflags" cargo test --package "$$package" "$$marker" -- --list > "$$tests_file"; \
			examine_re="$$(python3 scripts/mutation_examine_re.py "$$tests_file")"; \
			rm -f "$$tests_file"; \
		fi; \
		if [ -n "$$shard" ]; then \
			shard_label="$${shard//\//-of-}"; \
			mode="$${mode}-shard-$$shard_label"; \
			shard_arg="--shard $$shard"; \
		fi; \
		if [ "$$mutation_check" = "auto" ]; then \
			if [ -n "$$shard" ]; then \
				mutation_check="0"; \
			else \
				mutation_check="1"; \
			fi; \
		fi; \
		echo -e "\033[1m\033[0;36m▶ Mutation testing ($$mode): $$package\033[0m"; \
		echo -e "\033[0;36m  [diag] Tests: $${test_filter:-all}\033[0m"; \
		echo -e "\033[0;36m  [diag] Mutants: $$examine_re\033[0m"; \
		if [ -n "$$shard" ]; then \
			echo -e "\033[0;36m  [diag] Shard: $$shard\033[0m"; \
		fi; \
		out_dir="$(_MUTATION_DIR)/mutants.out.$$mode"; \
		rm -rf "$$out_dir"; \
		mutants_count="$$(RUSTFLAGS="$$mutation_rustflags" cargo mutants --list --package "$$package" --re "$$examine_re" $$shard_arg | wc -l | tr -d " ")"; \
		if [ "$$mutants_count" -eq 0 ]; then \
			echo -e "\033[0;31m✗ No mutants selected\033[0m"; \
			exit 1; \
		fi; \
		echo -e "\033[0;36m  [diag] Total mutants: $$mutants_count\033[0m"; \
		if [ -n "$$test_filter" ]; then \
			RUSTFLAGS="$$mutation_rustflags" cargo mutants \
				--jobs 4 --timeout 60 --baseline skip --copy-target true \
				--package "$$package" --re "$$examine_re" \
				$$shard_arg \
				--output "$$out_dir" \
				-- --test coverage_boost_33_tests --test mutation_kill_tests "$$test_filter" || true; \
		else \
			RUSTFLAGS="$$mutation_rustflags" cargo mutants \
				--jobs 4 --timeout 60 --baseline skip --copy-target true \
				--package "$$package" --re "$$examine_re" \
				$$shard_arg \
				--output "$$out_dir" || true; \
		fi; \
		results_dir="$$out_dir/mutants.out"; \
		missed_file="$$results_dir/missed.txt"; \
		unviable_file="$$results_dir/unviable.txt"; \
		caught_file="$$results_dir/caught.txt"; \
		timeout_file="$$results_dir/timeout.txt"; \
		missed=0; unviable=0; caught=0; timed_out=0; \
		[ -s "$$missed_file" ] && missed="$$(wc -l < "$$missed_file" | tr -d " ")" || true; \
		[ -s "$$unviable_file" ] && unviable="$$(wc -l < "$$unviable_file" | tr -d " ")" || true; \
		[ -s "$$caught_file" ] && caught="$$(wc -l < "$$caught_file" | tr -d " ")" || true; \
		[ -s "$$timeout_file" ] && timed_out="$$(wc -l < "$$timeout_file" | tr -d " ")" || true; \
		echo -e "\033[1m\033[0;36m▶ Results: $$mutants_count mutants — $$caught caught, $$missed missed, $$unviable unviable, $$timed_out timeout\033[0m"; \
		report="$(_MUTATION_DIR)/mutants_report.html"; \
		scores="$(_MUTATION_DIR)/mutation_scores.json"; \
		if [ "$$mutation_check" = "1" ]; then \
			python3 "$(_MUTATION_DIR)/mutants_report.py" \
				"$$results_dir/outcomes.json" \
				"$$report" \
				--scores "$$scores" \
				--scope "$$mode"; \
		else \
			python3 "$(_MUTATION_DIR)/mutants_report.py" \
				"$$results_dir/outcomes.json" \
				"$$report"; \
		fi; \
		echo -e "\033[0;36m  Report: $$report\033[0m"; \
		if [ "$$missed" -gt 0 ]; then \
			echo -e "\033[0;33m  Missed mutants ($$missed) — recorded in baseline:\033[0m"; \
			cat "$$missed_file"; \
		fi; \
	'

## conformance: Run the PEP typing conformance suite and write
## conformance/conformance_status.csv. Fetches the upstream suite if missing;
## use FETCH=1 to force a re-download.
conformance:
	@bash scripts/conformance.sh $(if $(filter 1,$(FETCH)),--fetch,)

## bench: Benchmark Basilisk vs pyright/mypy/ty/pyrefly on the fixture suite.
## Requires hyperfine; competitor tools are skipped if not installed.
## Writes per-fixture JSON + a summary to benchmarks/results/.
bench:
	@cargo build --release --bin basilisk
	@bash benchmarks/run.sh

## reinstall-vsix: Clean rebuild + reinstall a host-targeted VSIX.
## Mirrors .github/workflows/release.yml `vsix` job for the host platform.
reinstall-vsix: _clean_rust _clean_vsix _release_vsix _uninstall_vsix _install_vsix
	@echo -e '\033[0;32m✓ reinstall-vsix complete\033[0m'

## reinstall-vsix-prerelease: Same as reinstall-vsix but packages with
## --pre-release so the VSIX matches what the release pipeline builds for
## tags like v0.1.0-alpha. Use to dry-run a prerelease install locally.
reinstall-vsix-prerelease: VSCE_PRERELEASE := 1
reinstall-vsix-prerelease: _clean_rust _clean_vsix _release_vsix _uninstall_vsix _install_vsix
	@echo -e '\033[0;32m✓ reinstall-vsix-prerelease complete\033[0m'

# =============================================================================
# Internal Recipes
# =============================================================================

_clean_rust:
	@echo -e '\033[1m\033[0;36m▶ Cleaning Rust artifacts\033[0m' && \
	cargo clean && \
	$(RM) lcov.info && \
	echo -e '\033[0;32m✓ Rust clean complete\033[0m'

_clean_vsix:
	@echo -e '\033[1m\033[0;36m▶ Cleaning VSIX artifacts\033[0m' && \
	$(RM) $(_EXTENSION_DIR)/out $(_EXTENSION_DIR)/*.vsix ./*.vsix $(_EXTENSION_DIR)/NOTICES && \
	echo -e '\033[0;32m✓ VSIX clean complete\033[0m'

_uninstall_binaries:
	@echo -e '\033[1m\033[0;36m▶ Removing installed binaries\033[0m' && \
	cargo uninstall basilisk 2>/dev/null || true && \
	cargo uninstall basilisk-profiler-helper 2>/dev/null || true && \
	echo -e '\033[0;32m✓ Binaries removed\033[0m'

_build_rust:
	@echo -e '\033[1m\033[0;36m▶ Building Rust (release)\033[0m' && \
	cargo build --release && \
	echo -e '\033[0;32m✓ Rust build complete\033[0m'

_install_binaries:
	@echo -e '\033[1m\033[0;36m▶ Installing binaries\033[0m' && \
	cargo install --path crates/basilisk-cli --force && \
	cargo install --path crates/basilisk-profiler-helper --force && \
	echo -e '\033[0;32m✓ Binaries installed\033[0m'

_build_vsix:
	@echo -e '\033[1m\033[0;36m▶ Building VS Code extension\033[0m' && \
	cd $(_EXTENSION_DIR) && npm ci && npm run compile && \
	echo -e '\033[0;32m✓ VS Code extension compiled\033[0m'

# _release_vsix: build a host-targeted VSIX. Mirrors the per-platform steps
# of the `vsix` job in .github/workflows/release.yml. Single recipe so host
# detection vars are consistent throughout.
_release_vsix:
	@set -e; \
	case "$$(uname -s)" in \
		Darwin) plat=darwin; exe="" ;; \
		Linux)  plat=linux;  exe="" ;; \
		MINGW*|MSYS*|CYGWIN*) plat=win32; exe=".exe" ;; \
		*) echo "Unsupported OS: $$(uname -s)" >&2; exit 1 ;; \
	esac; \
	case "$$(uname -m)" in \
		arm64|aarch64) arch=arm64; rust_arch=aarch64 ;; \
		x86_64|amd64)  arch=x64;   rust_arch=x86_64 ;; \
		*) echo "Unsupported arch: $$(uname -m)" >&2; exit 1 ;; \
	esac; \
	case "$$plat" in \
		darwin) rust_target="$$rust_arch-apple-darwin" ;; \
		linux)  rust_target="$$rust_arch-unknown-linux-gnu" ;; \
		win32)  rust_target="$$rust_arch-pc-windows-msvc" ;; \
	esac; \
	target="$$plat-$$arch"; \
	echo -e "\033[1m\033[0;36m▶ Building VSIX for $$target ($$rust_target)\033[0m"; \
	cargo build --release --target "$$rust_target" --bin basilisk; \
	if [ "$$plat" = "darwin" ]; then \
		cargo build --release --target "$$rust_target" --bin basilisk-profiler-helper; \
	fi; \
	node $(_EXTENSION_DIR)/scripts/stage-runtime.mjs "target/$$rust_target/release" "$$target"; \
	cp shipwright.json $(_EXTENSION_DIR)/shipwright.json; \
	cp NOTICES $(_EXTENSION_DIR)/NOTICES; \
	repo_root="$$(pwd)"; \
	cd $(_EXTENSION_DIR) && npm ci && npm run compile && npm run sync:shipwright; \
	echo -e "\033[1m\033[0;36m▶ Vendoring debugpy into the VSIX bundle\033[0m"; \
	node scripts/vendor-debugpy.mjs; \
	prerelease_flag=""; \
	if [ -n "$(VSCE_PRERELEASE)" ]; then prerelease_flag="--pre-release"; fi; \
	npx vsce package $$prerelease_flag --target "$$target" --ignore-other-target-folders --out "$$repo_root/basilisk-$$target.vsix"; \
	echo -e "\033[1m\033[0;36m▶ Verifying VSIX bundles every manifest component\033[0m"; \
	node scripts/verify-shipwright.mjs vsix "$$repo_root/basilisk-$$target.vsix" "$$target"; \
	echo -e "\033[0;32m✓ VSIX built at basilisk-$$target.vsix$${prerelease_flag:+ (pre-release)}\033[0m"

_uninstall_vsix:
	@echo -e '\033[1m\033[0;36m▶ Uninstalling VSIX\033[0m' && \
	code --uninstall-extension nimblesite.basilisk 2>/dev/null || true && \
	echo -e '\033[0;32m✓ VSIX uninstalled\033[0m'

_install_vsix:
	@echo -e '\033[1m\033[0;36m▶ Installing VSIX\033[0m' && \
	code --install-extension $$(ls -t ./*.vsix | head -1) && \
	echo -e '\033[0;32m✓ VSIX installed\033[0m'

_lint_rust:
	@echo -e '\033[1m\033[0;36m▶ Linting Rust\033[0m' && \
	cargo check --workspace --all-targets && \
	cargo clippy --workspace --all-targets -- -D warnings && \
	echo -e '\033[0;32m✓ Rust lint passed\033[0m'

_lint_vsix:
	@echo -e '\033[1m\033[0;36m▶ Linting VS Code extension\033[0m' && \
	cd $(_EXTENSION_DIR) && npm ci --silent && npm run lint && \
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
	cd $(_EXTENSION_DIR) && npm run lint:fix && \
	echo -e '\033[0;32m✓ VS Code extension formatted\033[0m'

_audit:
	@bash scripts/audit.sh

_test_rust:
	@OPEN=$(OPEN) bash scripts/test-rust.sh

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
	cd $(_EXTENSION_DIR) && npm ci && npm run compile; \
	echo -e '\033[1m\033[0;36m▶ VS Code extension — stage the REAL release bundle\033[0m'; \
	node scripts/sync-shipwright-manifest.mjs; \
	node scripts/stage-runtime.mjs "$$(dirname "$$BASILISK_BIN")"; \
	node scripts/vendor-debugpy.mjs; \
	node scripts/verify-shipwright.mjs manifest; \
	echo -e '\033[1m\033[0;36m▶ VS Code extension — ESLint\033[0m'; \
	npm run lint; \
	echo -e '\033[1m\033[0;36m▶ VS Code E2E tests\033[0m'; \
	VSCODE_TEST_CMD="npm test -- --coverage"; \
	if [ -z "$${DISPLAY:-}" ] && command -v xvfb-run >/dev/null 2>&1; then \
	    VSCODE_TEST_CMD="xvfb-run -a $$VSCODE_TEST_CMD"; \
	fi; \
	$$VSCODE_TEST_CMD; \
	echo -e '\033[1m\033[0;36m▶ VS Code extension — coverage threshold\033[0m'; \
	VSIX_LCOV="$$REPO_ROOT/$(_EXTENSION_DIR)/coverage/lcov.info"; \
	VSIX_THRESHOLD=$$(python3 -c 'import json; print(json.load(open("'"$$REPO_ROOT"'/$(_COVERAGE_THRESHOLDS_FILE)"))["projects"]["vsix"]["threshold"])'); \
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

_package_zed:
	@echo -e '\033[1m\033[0;36m▶ Building basilisk CLI for Zed\033[0m' && \
	cargo install --path crates/basilisk-cli --force && \
	echo "$$(which basilisk) installed" && \
	echo "" && \
	echo "Now reinstall the dev extension in Zed:" && \
	echo "  Cmd+Shift+P -> 'zed: install dev extension'" && \
	echo "  Select: $(_ZED_DIR)"
