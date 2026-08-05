# agent-pmo:74cf183
# =============================================================================
# Standard Makefile — Basilisk
# Cross-platform: Linux, macOS, Windows (via GNU Make)
# Exactly 7 standard targets: build, test, lint, fmt, clean, ci, setup
# =============================================================================

.PHONY: build test lint fmt clean ci setup book mutation-test conformance mutation-conformance bench bench-basilisk reinstall-vsix reinstall-vsix-macos reinstall-vsix-prerelease package-zed

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
_BOOK_DIR                  := book
_MUTATION_DIR              := mutation_testing
_MUTATION_TEST_PACKAGE     := basilisk-checker
_MUTATION_TEST_MARKER      := mutation_safe
# Which crate to mutate. Every crate here mutates ALL its source (no code
# exclusions); only the TEST suite is scoped per crate (see mutation-test).
PKG                        ?= basilisk-checker
# EXISTING checker test binaries fed to the mutation run as the killing suite.
# These are the broad, FAST rule-test binaries already in the repo (thousands of
# assertions, sub-2s to execute); they are what actually kill the whole-crate
# mutant pool. Add existing binaries here to raise the kill rate — never invent
# new tests just for mutation. Slow/E2E-ish binaries are deliberately omitted so
# the per-mutant test run stays cheap.
#
# The one class of NEW binary that belongs here is a `#[mutation_safe]` suite
# WIDENING the examined scope ([CHKARCH-TESTING-MUTATION-RATCHET]): those tests
# assert real rule behaviour first and would earn their place with the ratchet
# switched off — they are listed so the functions they newly bring in-scope are
# actually exercised, not scored as missed.
#
# Order matters, but only a little. `cargo test` stops at the first failing
# binary, so a mutant dies as soon as a binary that kills it runs;
# `mutation_kill_tests` exists to kill these mutants, so it runs first. Measured
# effect of the move alone: 31 -> 30 timeouts. It is kept because it is free and
# directionally right, NOT because it solved anything — see `--timeout` below for
# what actually did ([CHKARCH-TESTING-MUTATION-RATCHET]).
_CHECKER_MUTATION_TESTS := \
	--test mutation_kill_tests \
	--test mutation_kill_constructors_tests \
	--test coverage_boost_tests \
	--test coverage_boost_32_tests \
	--test coverage_boost_33_tests \
	--test coverage_boost_34_tests \
	--test coverage_boost_35_tests \
	--test coverage_boost_36_tests \
	--test coverage_boost_37_tests \
	--test coverage_boost_38_tests \
	--test checker_tests \
	--test checker_rules_a_tests \
	--test checker_rules_b_tests \
	--test checker_rules_c_tests \
	--test checker_rules_d_tests \
	--test checker_rules_e_tests \
	--test checker_rules_f_tests \
	--test checker_rules_g_tests \
	--test comprehensive_rules_tests \
	--test advanced_rules_tests \
	--test categorical_tests \
	--test fp_elimination_tests \
	--test config_override_tests \
	--test rule_tags_tests \
	--test cached_tests \
	--test incremental_tests \
	--test incremental_cross_tests \
	--test incremental_resolved_tests \
	--test inference_all_tests
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
lint: _lint_rust _lint_vsix _lint_deslop _lint_docs

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

## book: Build and EPUBCheck The Basilisk Book outline EPUB
book:
	@$(MAKE) --no-print-directory -C $(_BOOK_DIR) epub

## mutation-test: Mutate a crate's source and kill with its fast test suite.
## PKG=basilisk-checker (default) | basilisk-lsp. The per-PR `working` gate scopes
## mutants to the functions the mutation-safe binaries cover (via
## scripts/mutation_examine_re.py) so it finishes inside CI's 60-min budget. Use
## ALL=1 for the WHOLE-crate run (examine_re=".", every line, no exclusions) —
## thorough but hours-long, so it is an offline/scheduled run, never the PR gate.
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
			test_desc="all tests (unscoped); mutants: WHOLE crate (examine_re=.)"; \
		elif [ "$$package" = "basilisk-lsp" ]; then \
			mode="lsp"; \
			test_args="--lib"; \
			test_desc="lib unit tests (--lib; no E2E)"; \
		else \
			mode="working"; \
			mutation_rustflags="$${mutation_rustflags:+$$mutation_rustflags }--cfg mutation_testing"; \
			test_args="--lib $(_CHECKER_MUTATION_TESTS)"; \
			test_desc="lib unit tests + broad existing rule-test binaries"; \
			tests_file="$$(mktemp)"; \
			RUSTFLAGS="$$mutation_rustflags" cargo test --package "$$package" "$$marker" -- --list > "$$tests_file" 2>/dev/null || true; \
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
		echo -e "\033[0;36m  [diag] Tests: $$test_desc\033[0m"; \
		echo -e "\033[0;36m  [diag] Mutant selection (examine_re): $$examine_re\033[0m"; \
		if [ -n "$$shard" ]; then \
			echo -e "\033[0;36m  [diag] Shard: $$shard\033[0m"; \
		fi; \
		cores="$$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)"; \
		half_cores="$$(( cores / 2 ))"; \
		default_jobs="$$(( half_cores < 4 ? half_cores : 4 ))"; \
		[ "$$default_jobs" -lt 1 ] && default_jobs=1 || true; \
		mutation_jobs="$${MUTATION_JOBS:-$$default_jobs}"; \
		echo -e "\033[0;36m  [diag] Parallel jobs: $$mutation_jobs (cores=$$cores; capped low — each job is a full cargo rebuild, over-parallelism starves the test phase into false TIMEOUTs)\033[0m"; \
		out_dir="$(_MUTATION_DIR)/mutants.out.$$mode"; \
		rm -rf "$$out_dir"; \
		mutants_count="$$(RUSTFLAGS="$$mutation_rustflags" cargo mutants --list --package "$$package" --re "$$examine_re" $$shard_arg | wc -l | tr -d " ")"; \
		if [ "$$mutants_count" -eq 0 ]; then \
			echo -e "\033[0;31m✗ No mutants selected\033[0m"; \
			exit 1; \
		fi; \
		echo -e "\033[0;36m  [diag] Total mutants: $$mutants_count\033[0m"; \
		echo -e "\033[0;36m  [diag] Test timeout: 400s — a mutated lib relinks every test binary in each parallel job; at 120s that link cost alone exhausted the budget, so killable mutants were recorded as TIMEOUT and credited as kills without ever being evaluated\033[0m"; \
		if [ -n "$$test_args" ]; then \
			RUSTFLAGS="$$mutation_rustflags" cargo mutants \
				--jobs "$$mutation_jobs" --timeout 400 --build-timeout 600 --baseline skip --copy-target true \
				--package "$$package" --test-package "$$package" --re "$$examine_re" \
				$$shard_arg \
				--output "$$out_dir" \
				-- $$test_args || true; \
		else \
			RUSTFLAGS="$$mutation_rustflags" cargo mutants \
				--jobs "$$mutation_jobs" --timeout 400 --build-timeout 600 --baseline skip --copy-target true \
				--package "$$package" --test-package "$$package" --re "$$examine_re" \
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
		killed="$$((caught + timed_out))"; \
		echo -e "\033[1m\033[0;36m▶ Results: $$mutants_count mutants — $$killed killed ($$caught caught + $$timed_out timeout-as-kill), $$missed missed, $$unviable unviable\033[0m"; \
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

## conformance: Run the pristine fixture regression check with python/typing's
## unmodified harness at the last revision carrying its Basilisk adapter. Writes
## internal evidence only; it is not a current official conformance score.
## Clones FRESH every run (no cache); needs network + git.
conformance:
	@cargo build -p basilisk-cli --bin basilisk
	@python3 conformance/run_conformance.py --bin target/debug/basilisk

## mutation-conformance: Gate basilisk on the AST-PRESERVING MUTATED fixtures
## (consistent import renames + whitespace reformatting; sharkdp's harness,
## vendored verbatim). Identical semantics, so a structural checker must hold
## its verdicts; the internal pass-rate ratchet may only rise. Neither this nor
## the pristine result is a current official conformance score. See
## conformance/run_mutation_conformance.py and docs/CONFORMANCE-INTEGRITY-AUDIT.md.
mutation-conformance:
	@cargo build --release -p basilisk-cli --bin basilisk
	@python3 conformance/run_mutation_conformance.py --bin target/release/basilisk

## bench: Benchmark Basilisk vs pyright/mypy/ty/pyrefly/zuban on the fixture suite.
## INDICATIVE ONLY — this runs on a developer workstation under whatever else it
## is doing, so nothing passes or fails on the result. Compare tools within one
## run; do not compare across machines or across time.
## Requires hyperfine; competitor tools are skipped if not installed.
## run.sh does the CLEAN release rebuild itself (fresh binary under test) before
## timing, so the guarantee holds even when run.sh is invoked directly — this
## target just delegates. Writes per-fixture JSON + a summary to benchmarks/results/.
bench:
	@bash benchmarks/run.sh

## bench-basilisk: Re-time ONLY basilisk (local iteration on a perf fix).
## Same clean release rebuild and same stability policy — it just skips the five
## competitors, which add minutes per iteration and say nothing about a change to
## this tree. Their CSV cells and versions carry forward verbatim and the header
## records that they were not re-timed. Refused in CI, which runs the full sweep.
bench-basilisk:
	@BENCH_ONLY_BASILISK=1 bash benchmarks/run.sh

## torture: Type-torture scoreboard — hard, spec-grounded typing problems
## scored conformance-style (`# E` lines) against pyright/mypy/ty/pyrefly/zuban,
## every tool in its out-of-the-box defaults, with hang detection as a
## correctness axis. WRITE-ALWAYS to benchmarks/torture/status/torture.csv,
## read-only regression gate against the committed baseline (exit 3).
## Needs target/release/basilisk (or BASILISK_BIN); build it first.
torture:
	@python3 benchmarks/torture/run_torture.py

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
	$(RM) $(_EXTENSION_DIR)/out $(_EXTENSION_DIR)/*.vsix ./*.vsix \
		$(_EXTENSION_DIR)/NOTICES $(_EXTENSION_DIR)/THIRD-PARTY-LICENSES \
		$(_EXTENSION_DIR)/RUST-DEPENDENCY-LICENSES \
		$(_EXTENSION_DIR)/VSCODE-DEPENDENCY-LICENSES && \
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
# [STUBRES-TYPESHED-LICENSE] Every binary-bearing package carries the Basilisk
# license and the exact third-party attribution files.
_release_vsix:
	@set -e; \
	python3 scripts/verify_release_attribution.py --policy-only; \
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
	cp VSCODE-DISTRIBUTION-LICENSE $(_EXTENSION_DIR)/LICENSE.txt; \
	cp NOTICES THIRD-PARTY-LICENSES RUST-DEPENDENCY-LICENSES \
		VSCODE-DEPENDENCY-LICENSES $(_EXTENSION_DIR)/; \
	repo_root="$$(pwd)"; \
	cd $(_EXTENSION_DIR) && npm ci && npm run licenses:check && \
		npm run compile && npm run sync:shipwright; \
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

# No `cargo check` pass: clippy-driver IS rustc plus extra lint passes over the
# same --workspace --all-targets unit graph, so every error and warning `cargo
# check` can emit, `cargo clippy` already emits. Running both compiled the whole
# workspace TWICE — clippy sets RUSTC_WORKSPACE_WRAPPER, which enters the
# fingerprint for workspace units, so the second pass shares nothing but
# dependencies. Nothing is suppressed here; one pass reports what two did.
_lint_rust:
	@echo -e '\033[1m\033[0;36m▶ Linting Rust\033[0m' && \
	cargo clippy --workspace --all-targets -- -D warnings && \
	cargo audit && \
	bash scripts/check-dependency-shape.sh && \
	bash scripts/check-illegal-tags.sh && \
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

# Generated-documentation drift gates. The published READMEs (GitHub, the VSIX
# on both Marketplace and Open VSX, PyPI) are rendered from docs/readme/
# ([README]), and the diagnostic reference data is generated from the checker
# rule sources ([WEBSITE-ERROR-PAGES-DRIFT]) — editing either output by hand,
# or editing a source without regenerating, fails here as it does in CI.
_lint_docs:
	@echo -e '\033[1m\033[0;36m▶ Checking generated documentation\033[0m' && \
	python3 scripts/gen_readmes.py --check && \
	python3 scripts/gen_rules_reference.py --data /tmp/basilisk-rules.json && \
	diff -u website/src/_data/rules.json /tmp/basilisk-rules.json > /dev/null || \
		{ echo 'rules.json is stale — run: python3 scripts/gen_rules_reference.py --data'; exit 1; } && \
	echo -e '\033[0;32m✓ Generated documentation is in sync\033[0m'

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

## package-zed: Build the local Zed dev loop — compile the extension to WASM,
## install the basilisk CLI, then print the `zed: install dev extension` steps.
## Point the dev extension at the locally built binary with
## `BASILISK_PATH=$$(which basilisk)` or `lsp.basilisk.binary.path`
## ([ZED-DIST]); with neither, it downloads the release binary.
package-zed:
	@echo -e '\033[1m\033[0;36m▶ Building Zed extension (wasm32-wasip2)\033[0m' && \
	rustup target add wasm32-wasip2 && \
	cargo build --release --target wasm32-wasip2 --manifest-path $(_ZED_DIR)/Cargo.toml && \
	echo -e '\033[1m\033[0;36m▶ Building basilisk CLI for Zed\033[0m' && \
	cargo install --path crates/basilisk-cli --force && \
	echo "$$(which basilisk) installed" && \
	echo "" && \
	echo "Now reinstall the dev extension in Zed:" && \
	echo "  Cmd+Shift+P -> 'zed: install dev extension'" && \
	echo "  Select: $(_ZED_DIR)"
