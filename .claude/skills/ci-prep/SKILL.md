---
name: ci-prep
description: Prepare the Basilisk codebase for CI. Reads the CI workflow, builds a checklist, then loops through fmt/clippy/build/test until every check passes. Use before submitting a PR or when the user wants to ensure CI will pass.
argument-hint: "[optional focus area]"
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---
<!-- agent-pmo:2efd847 -->

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

Example only. you must not use this. you must parse the gh action:
1. **Lint job** (`cargo fmt --all --check`, `cargo clippy --release --all-targets` for workspace, same for zed extension)
2. **Zed Extension** (`cargo build --release --target wasm32-wasip2 --manifest-path basilisk-zed/Cargo.toml`, clippy, tests)
3. **Rust Tests & Coverage** (`./scripts/test-rust.sh` — runs tests with `cargo llvm-cov` and enforces per-crate coverage thresholds)
4. **VS Code Extension** (`./scripts/test-vscode.sh`)
5. **Neovim Extension** (`./scripts/test-nvim.sh`)

For local CI prep, focus on checks 1–3 (lint, zed, rust tests). VS Code and Neovim tests require X11/display and are impractical locally.

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

- Running all the tests that the CI runs is critical. They must all pass.
- NEVER stop with failing checks. Loop until everything is green.
- NEVER suppress lint warnings with `#[allow(...)]` — fix the code.
- NEVER remove test assertions or delete tests to make them pass.
- NEVER lower coverage thresholds.
- Fix the CODE, not the checks.
- If stuck on the same failure after 3 attempts, ask the user for help. Do NOT silently give up.
- All Rust quality rules from CLAUDE.md apply: no `unwrap()`, no `panic!`, no `unsafe`, small focused functions (<20 lines), `Result<T,E>` everywhere.
