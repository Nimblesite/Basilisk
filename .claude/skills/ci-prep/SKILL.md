---
name: ci-prep
description: Prepare the Basilisk codebase for CI. Reads the CI workflow, builds a checklist, then loops through fmt/clippy/build/test until every check passes. Use before submitting a PR or when the user wants to ensure CI will pass.
argument-hint: "[optional focus area]"
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
---

# CI Prep — Get Basilisk PR-Ready

You MUST NOT STOP until every check passes.

## Step 1: Read the CI Pipeline and Build Your Checklist

Read the CI workflow:

```bash
cat .github/workflows/ci.yml
```

Parse EVERY step. Extract the exact commands CI runs. Build a numbered checklist. The CI pipeline changes over time so read it fresh every time — do not rely on assumptions.

The current CI pipeline runs these jobs (verify against the actual file):

1. **Lint job** (`cargo fmt --all --check`, `cargo clippy --release --all-targets` for workspace, same for zed extension)
2. **Zed Extension** (`cargo build --release --target wasm32-wasip2 --manifest-path basilisk-zed/Cargo.toml`, clippy, tests)
3. **Rust Tests & Coverage** (`./scripts/test-rust.sh` — runs tests with `cargo llvm-cov` and enforces per-crate coverage thresholds)
4. **VS Code Extension** (`./scripts/test-vscode.sh`)
5. **Neovim Extension** (`./scripts/test-nvim.sh`)

For local CI prep, focus on checks 1–3 (lint, zed, rust tests). VS Code and Neovim tests require X11/display and are impractical locally.

## Step 2: The Checklist

Run these in order. Fix failures before moving on.

### Check 1: cargo fmt

```bash
cargo fmt --all --check
```

If it fails, run `cargo fmt --all` to fix, then re-check.

### Check 2: cargo clippy (workspace)

```bash
cargo clippy --release --all-targets 2>&1
```

CI uses `RUSTFLAGS="-D warnings"` — all warnings are errors. Fix every clippy warning. Never use `#[allow(...)]`.

### Check 3: cargo clippy (zed extension)

```bash
cargo clippy --release --all-targets --manifest-path basilisk-zed/Cargo.toml 2>&1
```

### Check 4: cargo build (zed extension WASM)

```bash
cargo build --release --target wasm32-wasip2 --manifest-path basilisk-zed/Cargo.toml 2>&1
```

### Check 5: Rust tests with coverage

```bash
./scripts/test-rust.sh
```

This runs `cargo llvm-cov` with coverage instrumentation and enforces per-crate thresholds. If `cargo-llvm-cov` is not installed:

```bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
```

Coverage thresholds (from `scripts/test-rust.sh`):
- basilisk-checker: 92%
- basilisk-cli: 94%
- basilisk-db: 100%
- basilisk-lsp: 74%
- basilisk-mojo: 91%
- basilisk-parser: 100%
- basilisk-plugin: 100%
- basilisk-resolver: 95%
- basilisk-stubs: 100%
- basilisk-config: 92%

If you cannot run `./scripts/test-rust.sh` (missing `cargo-llvm-cov`), fall back to:

```bash
cargo test --workspace --exclude basilisk-compiler 2>&1
```

But note: coverage thresholds will NOT be checked. Install `cargo-llvm-cov` to fully replicate CI.

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

## Rules

- NEVER stop with failing checks. Loop until everything is green.
- NEVER suppress lint warnings with `#[allow(...)]` — fix the code.
- NEVER remove test assertions or delete tests to make them pass.
- NEVER lower coverage thresholds.
- Fix the CODE, not the checks.
- If stuck on the same failure after 3 attempts, ask the user for help. Do NOT silently give up.
- All Rust quality rules from CLAUDE.md apply: no `unwrap()`, no `panic!`, no `unsafe`, small focused functions (<20 lines), `Result<T,E>` everywhere.
