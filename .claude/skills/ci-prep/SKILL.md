---
name: ci-prep
description: Prepare the Basilisk codebase for CI. Reads the CI workflow, builds a checklist, then loops through fmt/clippy/build/test until every check passes. Use before submitting a PR or when the user wants to ensure CI will pass.
argument-hint: "[optional focus area]"
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---
<!-- agent-pmo:74cf183 -->

# CI Prep — Get Basilisk PR-Ready

You MUST NOT STOP until every check passes.

AIM: 
- PREP BRANCH FOR PR SUBMISSION
- AVOID LARGE CHANGES AND RABBIT HOLES
- WE WANT TO DUST OFF THE CURRENT FUNCTIONALITY AND MAKE SURE IT WILL PASS CI

## Step 1: Read the CI Pipeline and Build Your Checklist

Read the CI workflow:

```bash
cat .github/workflows/ci.yml
```

Parse EVERY step. Extract the exact commands CI runs. Build a numbered checklist. The CI pipeline changes over time so read it fresh every time — do not rely on assumptions.

**EVERY job that runs in CI MUST be run locally.** No exceptions. No "skip on macOS" or "needs display." If a tool is missing, install it (`brew install neovim`, ensure VS Code is in `/Applications`, etc.) and continue. If a target genuinely cannot run on this host, that is a TANK HARD condition — surface it to the user, do not silently skip.

Example only. you must not use this. you must parse the gh action:
1. **Lint job** (`cargo fmt --all --check`, `cargo clippy --release --all-targets` for workspace, same for zed extension)
2. **Zed Extension** (`cargo build --release --target wasm32-wasip2 --manifest-path basilisk-zed/Cargo.toml`, clippy, tests)
3. **Rust Tests & Coverage** (`./scripts/test-rust.sh` — runs tests with `cargo llvm-cov` and enforces per-crate coverage thresholds)
4. **VS Code Extension** (`make _test_vsix` — works on macOS via the native window server; on Linux it auto-uses `xvfb-run` when `DISPLAY` is empty)
5. **Neovim Extension** (`make _test_nvim` — requires `nvim` 0.11+, `pytest`, `luarocks`. Install via `brew install neovim luarocks` on macOS)
6. **Mutation Testing** (`make mutation-test` — runs `cargo mutants` and asserts no regression vs. `mutation_testing/mutation_scores.json`. Requires `cargo install cargo-mutants`)

**Mutation testing local-cadence exception.** The full mutation run takes ~10+ minutes even on a fast machine and is the ONLY checklist item you may skip locally: skip it when nothing in the mutation scope changed since the last green mutation run (no changes to mutated Rust code, `#[mutation_safe]` tests, `mutation_testing/`, or `scripts/mutation_examine_re.py`). If anything in scope changed, run it. CI always runs the full sharded suite on GitHub Actions regardless (4 parallel shards, ~9 min wall-clock), so the gate is never actually skipped — this exception only trims redundant local wall-clock.

## Step 2: The Checklist

- Run the checklist in order
- Coverage thresholds: read from `coverage-thresholds.json` at the repo root (single source of truth)
- If you can't run any step, TANK HARD
- If coverage exceeds the threshold, bump it in `coverage-thresholds.json` (ratchet UP only, subtract 1% buffer)
- If the coverage threshold is not met, TANK HARD

## Step 3: The Fix Loop

For each failing check:

1. Read the error output carefully
2. Find the root cause in the source code
3. Fix the actual code — never suppress warnings, never remove assertions, never lower thresholds
4. Re-run the check to confirm it passes
5. Move to the next check

When you reach the end of the checklist, **GO BACK TO THE START** and run the entire checklist again. A fix for one check may have broken an earlier check.

**Keep looping until you get a COMPLETE CLEAN RUN with ZERO failures from start to finish.**

## Step 4: Reporting

Once all checks pass cleanly, report:
- Which checks ran
- That all passed
- Any significant fixes that were made
- If you're fixing CI failures, you MUST commit/push and monitor the action log. Upon failure, REPEAT the process

## Rules

- Running all the tests that the CI runs is critical. **Every CI job runs locally — period.** Never report "passed" while skipping a job.
- NEVER stop with failing checks. Loop until everything is green.
- NEVER suppress lint warnings with `#[allow(...)]` — fix the code.
- NEVER remove test assertions or delete tests to make them pass.
- NEVER lower coverage thresholds.
- NEVER skip a CI job locally because of "platform" or "display" reasons. macOS has its own window server; Linux has `xvfb-run`. Install missing tools instead of skipping. If a job genuinely cannot run on the host, TANK HARD — tell the user what's missing and ask how to proceed. Do not declare success. (Sole exception: the mutation-testing local-cadence rule in Step 1 — CI always runs the full sharded mutation suite.)
- Fix the CODE, not the checks.
- If stuck on the same failure after 3 attempts, ask the user for help. Do NOT silently give up.
- All Rust quality rules from CLAUDE.md apply: no `unwrap()`, no `panic!`, no `unsafe`, small focused functions (<20 lines), `Result<T,E>` everywhere.
