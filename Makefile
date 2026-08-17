# agent-pmo:74cf183
# =============================================================================
# Standard Makefile — Basilisk
# Cross-platform: Linux, macOS, Windows (via GNU Make)
#
# PUBLIC targets (the whole interface — 7 standard + 3 repo-specific):
#   build test lint fmt clean ci setup   — the standard seven, plus
#   test-checker                         — fast type-checker test subset
#   test-checker-all                     — the full type-checker suite
#   conformance                          — fixture regression indicator
#
# Everything else is an INTERNAL recipe named with a leading underscore. They
# are still runnable (`make _bench`, `make _mutation_test`) — they are just not
# part of the advertised interface, and variants are variables, not targets:
#   _bench ONLY=basilisk        _reinstall_vsix TARGET=darwin-arm64 PRERELEASE=1
#   conformance MUTATED=1       _mutation_test PKG=... ALL=1 SHARD=...
# =============================================================================

.PHONY: build test test-checker test-checker-all lint fmt clean ci setup conformance

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
# Where THIS Makefile lives. Recipes run in the caller's cwd (the release
# attribution tests drive them from a temp tree), so repo scripts are located
# relative to the Makefile rather than relative to `pwd`.
_MK_DIR                    := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))
_ZED_DIR                   := basilisk-zed
_NVIM_DIR                  := basilisk.nvim
_BOOK_DIR                  := book
_MUTATION_DIR              := mutation_testing
_MUTATION_TEST_PACKAGE     := basilisk-checker
_MUTATION_TEST_MARKER      := mutation_safe
# Which crate to mutate. Every crate here mutates ALL its source (no code
# exclusions); only the TEST suite is scoped per crate (see _mutation_test).
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
_CHECKER_TEST_PACKAGES     := -p basilisk-checker -p basilisk-resolver \
                              -p basilisk-canonical -p basilisk-parser
OPEN                       ?= 0
ALL                        ?= 0
SHARD                      ?=
MUTATION_CHECK             ?= auto
# Variant switches for the internal recipes (see the header block).
T                          ?=
ONLY                       ?=
MUTATED                    ?= 0
TARGET                     ?=
PRERELEASE                 ?=
_CHECKER_REPORT_LOG        := target/test-checker-all.log

# Summary report for test-checker-all: one line per test — file, test name,
# result — then the counts. Nothing else. The raw cargo stream (thousands of
# panic messages and backtraces) goes to the log file, not the terminal.
#
# The one thing that is NOT suppressed is a compile error. A crate that fails
# to build contributes zero test lines, so a report that only counted results
# would print a small, clean, all-green summary for a suite that never ran.
# Only genuine build failures match — `error[E0308]:` and `could not compile`;
# cargo's `error: test failed` / `error: N targets failed:` tallies are the
# run's own exit summary and are already counted as FAILED.
export _CHECKER_REPORT_AWK
define _CHECKER_REPORT_AWK
cap == 1 {
    if ($$0 ~ /^[[:space:]]*$$/ || $$0 ~ /^test / ||
        $$0 ~ /^[[:space:]]*(Running|Compiling|Finished|Doc-tests|warning)/) { cap = 0 }
    else { B[++nb] = $$0; next }
}
/^error/ {
    if ($$0 ~ /^error: could not compile/) { buildfail = 1; next }
    if ($$0 ~ /^error: test failed/ || $$0 ~ /targets failed:/) { next }
    ne++; cap = 1; B[++nb] = $$0
    next
}
/Running / && /\(.*\)/ {
    b = $$0
    sub(/.*\(/, "", b); sub(/\).*/, "", b)
    sub(/.*\//, "", b); sub(/-[0-9a-fA-F]+$$/, "", b)
    bin = b
    next
}
/^[[:space:]]*Doc-tests / {
    bin = "doc-tests " $$2
    next
}
/^test result:/ { next }
/^test .* \.\.\. / {
    line = $$0
    sub(/^test /, "", line)
    i = index(line, " ... ")
    tn = substr(line, 1, i - 1)
    res = substr(line, i + 5)
    if (res == "ok") { np++; printf "%-32s %-64s ok\n", bin, tn }
    else if (res ~ /^FAILED/) { nf++; printf "%-32s %-64s \033[0;31mFAILED\033[0m\n", bin, tn }
    else if (res ~ /^ignored/) { ng++; printf "%-32s %-64s ignored\n", bin, tn }
    next
}
END {
    if (ne || buildfail) {
        printf "\n\033[0;31mBUILD FAILED (%d) — tests behind this never ran\033[0m\n", ne
        for (i = 1; i <= nb; i++) print B[i]
    }
    printf "\npassed %d   failed %d   ignored %d   total %d\n", np, nf, ng, np + nf + ng
}
endef

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

## test-checker: Type-checker test subset — the analysis crates only (checker,
## resolver, canonical, parser). No coverage, no audit, no VSIX/Neovim/Zed
## suites, so it is the loop to run while iterating on a rule. Fail-fast: cargo
## stops at the first failing binary. It is NOT the gate: `make test` still is.
## Filter with T=<substring>, e.g. `make test-checker T=narrowing`.
test-checker:
	@echo -e '\033[1m\033[0;36m▶ Type-checker test subset\033[0m' && \
	cargo test $(_CHECKER_TEST_PACKAGES) -- $(T) && \
	echo -e '\033[0;32m✓ Type-checker tests passed\033[0m'

## test-checker-all: Same crates, --no-fail-fast, and a report: one line per
## test — file, test name, result — then the counts. Fail-fast would stop at
## the first red binary and silently skip the thousands of tests behind it;
## this runs and reports every one. Panic output goes to the log, not the
## terminal. Filter with T=<substring>.
test-checker-all:
	@mkdir -p target
	@cargo test --no-fail-fast $(_CHECKER_TEST_PACKAGES) -- $(T) \
		> $(_CHECKER_REPORT_LOG) 2>&1; \
	status=$$?; \
	awk "$$_CHECKER_REPORT_AWK" $(_CHECKER_REPORT_LOG); \
	exit $$status

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
# Repo-Specific Recipes
#
# Only `conformance` is public here. The rest are internal (leading underscore)
# — occasional, human-driven runs that do not belong in the advertised
# interface. Invoke them by their exact name, e.g. `make _bench`.
# =============================================================================

# _book: Build and EPUBCheck The Basilisk Book outline EPUB
_book:
	@$(MAKE) --no-print-directory -C $(_BOOK_DIR) epub

# _mutation_test: Mutate a crate's source and kill with its fast test suite.
# PKG=basilisk-checker (default) | basilisk-lsp. The per-PR `working` gate scopes
## mutants to the functions the mutation-safe binaries cover (via
# scripts/mutation_examine_re.py) so it finishes inside CI's 60-min budget. Use
# ALL=1 for the WHOLE-crate run (examine_re=".", every line, no exclusions) —
# thorough but hours-long, so it is an offline/scheduled run, never the PR gate.
_mutation_test:
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

## conformance: Fixture regression indicator — a number to READ, never a gate,
## never a figure to publish (CLAUDE.md "Conformance"). Clones FRESH every run
## (no cache); needs network + git.
##
## Default: the pristine fixtures through python/typing's unmodified harness at
## the last revision carrying its Basilisk adapter.
##
## MUTATED=1: the same fixtures AST-PRESERVINGLY mutated (consistent import
## renames + whitespace reformatting; sharkdp's harness, vendored verbatim).
## Identical semantics, so a structural checker must hold its verdicts — the
## GAP between the two rates is what locates spelling dependence, not the
## height of either. See conformance/run_mutation_conformance.py and
## docs/CONFORMANCE-INTEGRITY-AUDIT.md.
##
## "Never a gate" is literal in both branches: each script exits 0 whatever it
## measures. Until 2026-08-09 the MUTATED=1 branch invoked a ratchet that
## failed the build when the rate fell below a stored floor, which contradicted
## this very help text and the CLAUDE.md rule it cites. The ratchet and its
## baseline file were deleted, not made conditional.
##
## Neither result is a current official conformance score. Writes internal
## evidence only.
conformance:
	@if [ "$(MUTATED)" = "1" ]; then \
		cargo build --release -p basilisk-cli --bin basilisk && \
		python3 conformance/run_mutation_conformance.py --bin target/release/basilisk; \
	else \
		cargo build -p basilisk-cli --bin basilisk && \
		python3 conformance/run_conformance.py --bin target/debug/basilisk; \
	fi

# _bench: Benchmark Basilisk vs pyright/mypy/ty/pyrefly/zuban on the fixture suite.
# INDICATIVE ONLY — this runs on a developer workstation under whatever else it
# is doing, so nothing passes or fails on the result. Compare tools within one
# run; do not compare across machines or across time.
# Requires hyperfine; competitor tools are skipped if not installed.
# run.sh does the CLEAN release rebuild itself (fresh binary under test) before
# timing, so the guarantee holds even when run.sh is invoked directly — this
# recipe just delegates. Writes per-fixture JSON + a summary to benchmarks/results/.
#
# ONLY=basilisk re-times ONLY basilisk (local iteration on a perf fix). Same
# clean release rebuild and same stability policy — it just skips the five
# competitors, which add minutes per iteration and say nothing about a change to
# this tree. Their CSV cells and versions carry forward verbatim and the header
# records that they were not re-timed. Refused in CI, which runs the full sweep.
_bench:
	@if [ "$(ONLY)" = "basilisk" ]; then \
		BENCH_ONLY_BASILISK=1 bash benchmarks/run.sh; \
	else \
		bash benchmarks/run.sh; \
	fi

# _torture: Type-torture scoreboard — hard, spec-grounded typing problems
# scored conformance-style (`# E` lines) against pyright/mypy/ty/pyrefly/zuban,
# every tool in its out-of-the-box defaults, with hang detection as a
# correctness axis. WRITE-ALWAYS to benchmarks/torture/status/torture.csv,
# read-only regression gate against the committed baseline (exit 3).
# Needs target/release/basilisk (or BASILISK_BIN); build it first.
_torture:
	@python3 benchmarks/torture/run_torture.py

# _smoke_micropython: Real-world smoke test for typeshed-path
# [STUBRES-CUSTOM-TYPESHED] — points the checker at a pinned, unmodified
# micropython-stdlib-stubs release and asserts MicroPython stdlib resolves
# while CPython-only modules fall through per canonicality. Downloads one
# wheel from PyPI (network); intentionally outside the blocking CI matrix.
_smoke_micropython:
	@python3 scripts/smoke_micropython_typeshed.py

# _reinstall_vsix: Clean rebuild + reinstall a host-targeted VSIX. Builds the
# EXACT package the release.yml `vsix` job ships (via the shared _release_vsix
# recipe) and rebuilds every binary from a clean tree.
# Implements [VSIX-PACKAGING-PARITY].
#
# TARGET=darwin-arm64 pins the platform regardless of host, so the artifact is
# byte-for-byte what the release.yml `vsix` darwin job publishes; unset
# auto-detects from uname. PRERELEASE=1 packages with --pre-release, matching
# what the release pipeline builds for tags like v0.1.0-alpha.
_reinstall_vsix: export BSK_VSIX_TARGET := $(TARGET)
_reinstall_vsix: VSCE_PRERELEASE := $(PRERELEASE)
_reinstall_vsix: _clean_rust _clean_vsix _release_vsix _uninstall_vsix _install_vsix
	@echo -e '\033[0;32m✓ _reinstall_vsix complete\033[0m'

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

# _release_vsix: build the VSIX — the EXACT artifact the release.yml `vsix` job
# ships. Single recipe shared by reinstall-vsix and the e2e gate (_test_vsix), so
# tests, local installs, and the published package can never diverge.
# Implements [VSIX-PACKAGING-PARITY].
#
# ONE package, no `--target`: the extension is a notice ([WITHDRAWAL-SURFACES])
# and bundles no binary, so there is nothing platform-specific left to build.
# The Rust build, the runtime staging, the debugpy vendoring, the Shipwright
# bundle verification and the third-party attribution files are all gone with
# it — a VSIX carrying none of their content must not claim any of it. The
# packaged tree is asserted afterwards rather than assumed: shipping the type
# checker again is the one failure that must be impossible.
_release_vsix:
	@set -e; \
	repo_root="$$(pwd)"; \
	echo -e "\033[1m\033[0;36m▶ Building the notice VSIX\033[0m"; \
	cp VSCODE-DISTRIBUTION-LICENSE $(_EXTENSION_DIR)/LICENSE.txt; \
	cd $(_EXTENSION_DIR) && npm ci && npm run licenses:check && npm run compile; \
	prerelease_flag=""; \
	if [ -n "$(VSCE_PRERELEASE)" ]; then prerelease_flag="--pre-release"; fi; \
	npx vsce package $$prerelease_flag --out "$$repo_root/basilisk.vsix"; \
	echo -e "\033[1m\033[0;36m▶ Verifying the VSIX ships no checker\033[0m"; \
	bash "$(_MK_DIR)scripts/verify-vsix-inert.sh" "$$repo_root/basilisk.vsix"; \
	echo -e "\033[0;32m✓ VSIX built at basilisk.vsix$${prerelease_flag:+ (pre-release)}\033[0m"

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

# Generated-documentation drift gates. The published READMEs are rendered from
# docs/readme/ ([README]), and the site's copy is extracted from the messaging
# spec ([WITHDRAWAL-COPY]) — editing either output by hand, or editing a source
# without regenerating, fails here as it does in CI. The withdrawal gate is the
# load-bearing one: it is what stops the site saying something the messaging
# spec does not.
_lint_docs:
	@echo -e '\033[1m\033[0;36m▶ Checking generated documentation\033[0m' && \
	python3 scripts/gen_readmes.py --check && \
	python3 scripts/gen_withdrawal_copy.py --check && \
	python3 scripts/test_published_readmes.py && \
	python3 scripts/check_public_copy.py && \
	python3 scripts/test_check_public_copy.py && \
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

# _package_zed: Build the local Zed dev loop — compile the extension to WASM,
# install the basilisk CLI, then print the `zed: install dev extension` steps.
# Point the dev extension at the locally built binary with
# `BASILISK_PATH=$$(which basilisk)` or `lsp.basilisk.binary.path`
# ([ZED-DIST]); with neither, it downloads the release binary.
_package_zed:
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
