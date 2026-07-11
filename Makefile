# agent-pmo:74cf183
# =============================================================================
# Standard Makefile — Basilisk
# Cross-platform: Linux, macOS, Windows (via GNU Make)
# Exactly 7 standard targets: build, test, lint, fmt, clean, ci, setup
# =============================================================================

.PHONY: build test lint fmt clean ci setup mutation-test conformance bench reinstall-vsix reinstall-vsix-macos reinstall-vsix-prerelease

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
# Which crate to mutate. Every crate here mutates ALL its source (no code
# exclusions); only the TEST suite is scoped per crate (see mutation-test).
PKG                        ?= basilisk-checker
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
lint: _lint_rust _lint_vsix _lint_deslop

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

## mutation-test: Mutate ALL of a crate's source; kill with its fast test suite.
## PKG=basilisk-checker (default) | basilisk-lsp. Use ALL=1 for the unscoped
## checker suite (every test, not just the mutation-safe binaries).
mutation-test:
	@bash -euo pipefail -c '\
		package="$(PKG)"; \
		marker="$(_MUTATION_TEST_MARKER)"; \
		mutation_rustflags="$${RUSTFLAGS:-}"; \
		examine_re="."; \
		shard="$(SHARD)"; \
		shard_arg=""; \
		mutation_check="$(MUTATION_CHECK)"; \
		test_args=""; \
		test_desc=""; \
		if [ "$(ALL)" = "1" ] && [ "$$package" = "basilisk-checker" ]; then \
			mode="all"; \
			test_args=""; \
			test_desc="all tests (unscoped)"; \
		elif [ "$$package" = "basilisk-lsp" ]; then \
			mode="lsp"; \
			test_args="--lib"; \
			test_desc="lib unit tests (--lib; no E2E)"; \
		else \
			mode="working"; \
			mutation_rustflags="$${mutation_rustflags:+$$mutation_rustflags }--cfg mutation_testing"; \
			test_args="--test coverage_boost_33_tests --test mutation_kill_tests $$marker"; \
			test_desc="mutation-safe binaries + marker"; \
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
		echo -e "\033[0;36m  [diag] Tests: $$test_desc\033[0m"; \
		echo -e "\033[0;36m  [diag] Mutants: ALL source in $$package (no code excluded)\033[0m"; \
		if [ -n "$$shard" ]; then \
			echo -e "\033[0;36m  [diag] Shard: $$shard\033[0m"; \
		fi; \
		mutation_jobs="$${MUTATION_JOBS:-$$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"; \
		echo -e "\033[0;36m  [diag] Parallel jobs: $$mutation_jobs\033[0m"; \
		out_dir="$(_MUTATION_DIR)/mutants.out.$$mode"; \
		rm -rf "$$out_dir"; \
		mutants_count="$$(RUSTFLAGS="$$mutation_rustflags" cargo mutants --list --package "$$package" --re "$$examine_re" $$shard_arg | wc -l | tr -d " ")"; \
		if [ "$$mutants_count" -eq 0 ]; then \
			echo -e "\033[0;31m✗ No mutants selected\033[0m"; \
			exit 1; \
		fi; \
		echo -e "\033[0;36m  [diag] Total mutants: $$mutants_count\033[0m"; \
		if [ -n "$$test_args" ]; then \
			RUSTFLAGS="$$mutation_rustflags" cargo mutants \
				--jobs "$$mutation_jobs" --timeout 60 --baseline skip --copy-target true \
				--package "$$package" --re "$$examine_re" \
				$$shard_arg \
				--output "$$out_dir" \
				-- $$test_args || true; \
		else \
			RUSTFLAGS="$$mutation_rustflags" cargo mutants \
				--jobs "$$mutation_jobs" --timeout 60 --baseline skip --copy-target true \
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

## conformance: Score basilisk by RUNNING the REAL python/typing harness and
## write conformance/conformance_status.csv + the website report. Clones the
## suite FRESH every run (no cache); needs network + git. See [CHKARCH-CONFORMANCE].
conformance:
	@cargo build -p basilisk-cli --bin basilisk
	@python3 conformance/run_conformance.py --bin target/debug/basilisk

## bench: Benchmark Basilisk vs pyright/mypy/ty/pyrefly/zuban on the fixture suite.
## Requires hyperfine; competitor tools are skipped if not installed.
## run.sh does the CLEAN release rebuild itself (fresh binary under test) before
## timing, so the guarantee holds even when run.sh is invoked directly — this
## target just delegates. Writes per-fixture JSON + a summary to benchmarks/results/.
bench:
	@bash benchmarks/run.sh

## smoke-micropython: Real-world smoke test for typeshed-path
## [STUBRES-CUSTOM-TYPESHED] — points the checker at a pinned, unmodified
## micropython-stdlib-stubs release and asserts MicroPython stdlib resolves
## while CPython-only modules fall through per canonicality. Downloads one
## wheel from PyPI (network); intentionally outside the blocking CI matrix.
smoke-micropython:
	@python3 scripts/smoke_micropython_typeshed.py

## reinstall-vsix: Clean rebuild + reinstall a host-targeted VSIX. Builds the
## EXACT package the release.yml `vsix` job ships (via the shared _release_vsix
## recipe) and rebuilds every binary from a clean tree.
## Implements [VSIX-PACKAGING-PARITY].
reinstall-vsix: _clean_rust _clean_vsix _release_vsix _uninstall_vsix _install_vsix
	@echo -e '\033[0;32m✓ reinstall-vsix complete\033[0m'

## reinstall-vsix-macos: Clean rebuild + reinstall the macOS VSIX (darwin-arm64)
## — byte-for-byte the artifact the release.yml `vsix` darwin job publishes. Pins
## the target so it matches the shipped macOS package regardless of host, and
## rebuilds every binary from a clean tree.
## Implements [VSIX-PACKAGING-PARITY].
reinstall-vsix-macos: export BSK_VSIX_TARGET := darwin-arm64
reinstall-vsix-macos: _clean_rust _clean_vsix _release_vsix _uninstall_vsix _install_vsix
	@echo -e '\033[0;32m✓ reinstall-vsix-macos complete (darwin-arm64)\033[0m'

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

# _release_vsix: build a host-targeted VSIX — the EXACT artifact the release.yml
# `vsix` job ships for that platform. Single recipe shared by reinstall-vsix,
# reinstall-vsix-macos, and the e2e gate (_test_vsix), so tests, local installs,
# and the published package can never diverge. Set BSK_VSIX_TARGET (e.g.
# darwin-arm64) to pin the platform regardless of host; unset auto-detects from
# uname. Implements [VSIX-PACKAGING-PARITY].
_release_vsix:
	@set -e; \
	if [ -n "$${BSK_VSIX_TARGET:-}" ]; then \
		target="$$BSK_VSIX_TARGET"; \
		plat="$${target%-*}"; arch="$${target##*-}"; \
	else \
		case "$$(uname -s)" in \
			Darwin) plat=darwin ;; \
			Linux)  plat=linux ;; \
			MINGW*|MSYS*|CYGWIN*) plat=win32 ;; \
			*) echo "Unsupported OS: $$(uname -s)" >&2; exit 1 ;; \
		esac; \
		case "$$(uname -m)" in \
			arm64|aarch64) arch=arm64 ;; \
			x86_64|amd64)  arch=x64 ;; \
			*) echo "Unsupported arch: $$(uname -m)" >&2; exit 1 ;; \
		esac; \
		target="$$plat-$$arch"; \
	fi; \
	case "$$arch" in \
		arm64) rust_arch=aarch64 ;; \
		x64)   rust_arch=x86_64 ;; \
		*) echo "Unsupported arch: $$arch" >&2; exit 1 ;; \
	esac; \
	exe=""; \
	case "$$plat" in \
		darwin) rust_target="$$rust_arch-apple-darwin" ;; \
		linux)  rust_target="$$rust_arch-unknown-linux-gnu" ;; \
		win32)  rust_target="$$rust_arch-pc-windows-msvc"; exe=".exe" ;; \
		*) echo "Unsupported platform: $$plat" >&2; exit 1 ;; \
	esac; \
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
	echo -e "\033[1m\033[0;36m▶ Validating Shipwright manifest\033[0m"; \
	node scripts/verify-shipwright.mjs manifest; \
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
	cargo audit && \
	echo -e '\033[0;32m✓ Rust lint passed\033[0m'

_lint_vsix:
	@echo -e '\033[1m\033[0;36m▶ Linting VS Code extension\033[0m' && \
	cd $(_EXTENSION_DIR) && npm ci --silent && npm run lint && \
	echo -e '\033[0;32m✓ VS Code lint passed\033[0m'

# Deslop duplication gate ([CI-DESLOP]). Reads the committed .deslop.toml budget
# and exits non-zero when repo-wide duplication exceeds the ceiling. Requires the
# `deslop` CLI on PATH (scripts/install-deslop.sh; checked by scripts/audit.sh).
_lint_deslop:
	@echo -e '\033[1m\033[0;36m▶ Deslop duplication gate\033[0m' && \
	deslop . && \
	echo -e '\033[0;32m✓ Deslop duplication gate passed\033[0m'

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

# _test_vsix: run the VS Code E2E suite against the EXACT release bundle. Builds
# the real per-platform VSIX through the shared _release_vsix recipe — same
# release binaries, manifest-driven staging, debugpy vendoring, and vsce
# packaging the release ships — then runs the e2e tests against that staged
# bundle, so the suite can never validate a different package than what users
# install. Implements [VSIX-PACKAGING-PARITY].
_test_vsix:
	@set -e; \
	REPO_ROOT="$$(pwd)"; \
	echo -e '\033[1m\033[0;36m▶ Building the EXACT release VSIX bundle (shared _release_vsix recipe)\033[0m'; \
	$(MAKE) --no-print-directory _release_vsix; \
	echo -e '\033[1m\033[0;36m▶ VS Code extension — ESLint\033[0m'; \
	( cd $(_EXTENSION_DIR) && npm run lint ); \
	echo -e '\033[1m\033[0;36m▶ VS Code E2E tests (against the staged release bundle)\033[0m'; \
	VSCODE_TEST_CMD="npm test -- --coverage"; \
	if [ -z "$${DISPLAY:-}" ] && command -v xvfb-run >/dev/null 2>&1; then \
	    VSCODE_TEST_CMD="xvfb-run -a $$VSCODE_TEST_CMD"; \
	fi; \
	( cd $(_EXTENSION_DIR) && $$VSCODE_TEST_CMD ); \
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
