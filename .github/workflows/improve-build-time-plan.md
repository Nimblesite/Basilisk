# Plan: Reduce CI Build Time from 21 min to ~8 min

## Context

The CI Build & Test job takes 21m46s. Of that, **15 minutes is compilation/linking** — specifically, linking ~310 test binaries in release mode. Each `.rs` file in `tests/` produces a separate binary that must be individually linked. The linker (GNU `ld`) is single-threaded, so 4 CI cores sit idle during the 13-minute linking phase.

Three optimizations, in order of impact:

---

## Phase 1: Test Binary Consolidation (~8-10 min savings)

Move test files into subdirectories so they're not auto-discovered as standalone binaries. Create harness files that include them via `mod`. This reduces 310 link operations to ~37.

### 1A. basilisk-checker (198 binaries -> 13)

**Create** `crates/basilisk-checker/tests/common/mod.rs` with shared helpers:
- `run(source) -> Result<Vec<Diagnostic>>` (parse -> resolve -> check)
- `codes(diags) -> Vec<&str>`
- `codes_owned(diags) -> Vec<String>` (for coverage_boost files that need owned strings)
- Common `use` imports

**Move** all small test files into `tests/checker/` subdirectory (files in subdirs are NOT auto-discovered by cargo). Create harness files in `tests/` that include them:

| Harness file | Includes | Count |
|---|---|---|
| `coverage_boost_tests.rs` | `checker/coverage_boost_{1..38}_tests.rs` | 38 |
| `e0001_e0025_tests.rs` | `checker/e0001_tests.rs` .. `checker/e0025_tests.rs` | ~23 |
| `e0026_e0050_tests.rs` | `checker/e0026_tests.rs` .. `checker/e0050_tests.rs` | ~21 |
| `e0051_e0075_tests.rs` | `checker/e0051_tests.rs` .. `checker/e0075_tests.rs` | ~21 |
| `e0076_e0100_tests.rs` | `checker/e0076_tests.rs` .. `checker/e0100_tests.rs` | ~22 |
| `e0101_e0125_tests.rs` | `checker/e0101_tests.rs` .. `checker/e0125_tests.rs` | ~22 |
| `e0126_e0149_tests.rs` | `checker/e0126_tests.rs` .. `checker/e0149_tests.rs` + w0040, w0050 | ~20 |
| `categorical_tests.rs` | deep_coverage, rules_coverage, mutation_kill, suppression, redundant_annotation | ~6 |
| `inference_all_tests.rs` | inference, inference_flow, collection_inference, types | ~4 |
| Keep standalone | `checker_tests.rs`, `advanced_rules_tests.rs`, `comprehensive_rules_tests.rs`, `config_override_tests.rs` | 4 |

Each harness file is tiny (~30 lines of `#[path]` + `mod` declarations). Each moved test file: remove local `run()`/`codes()` definitions, add `use super::common::*;`, change `#![allow(...)]` to module-level comments. The `#![allow(...)]` goes on the harness file instead.

**Delete** `coverage_boost_15_tests.rs.backup`.

### 1B. basilisk-resolver (65 binaries -> ~9)

Already has `common/mod.rs`. Move test files to `tests/resolver/` and create ~9 harness files grouped by topic (annotations, classes, functions, type system, protocols, resolution, mutants, typeddict).

### 1C. basilisk-lsp (37 binaries -> ~5)

Already has shared common modules. Move test files to `tests/lsp/` and create ~5 harness files (ws_core, ws_features, ws_navigation, lsp_stdio, zed). The current `ws_test_common.rs`/`lsp_e2e_common.rs`/`zed_e2e_common.rs` files sitting in `tests/` are also compiled as standalone binaries right now (waste) — moving them to subdirectories fixes this too.

---

## Phase 2: CI Profile (~2-3 min savings)

**Add to `Cargo.toml`:**
```toml
[profile.ci]
inherits = "release"
opt-level = 1
codegen-units = 256
lto = "thin"
debug = 2
incremental = false
```

- `opt-level = 1`: ~90% runtime speed, much faster codegen than `opt-level = 3`
- `codegen-units = 256`: maximum parallel LLVM codegen (default is 16)
- `lto = "thin"`: faster link times than fat LTO
- `debug = 2`: needed for coverage source mapping

**Update `scripts/test.sh`:** Replace `--release` with `--profile ci` for `cargo llvm-cov`, `cargo test` (compiler e2e, zed). Update binary path from `target/llvm-cov-target/release/` to `target/llvm-cov-target/ci/`.

**Keep `--release` for clippy** in the Lint job (already fast at 1m10s, no linking involved).

---

## Phase 3: Faster Linker (~1-2 min savings)

**Update `.cargo/config.toml`:**
```toml
[target.'cfg(target_os = "linux")']
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

Only affects Linux (CI). macOS local dev uses the system linker (already fast).

**Update `.github/workflows/ci.yml`:** Add `lld` to the apt-get install step (both lint and test jobs).

**Update `.devcontainer/Dockerfile`:** Add `lld` to keep deps in sync (per CLAUDE.md rule).

**Note on RUSTFLAGS:** CI sets `RUSTFLAGS="-D warnings"` as env var. Target-specific `rustflags` in config.toml are merged with env RUSTFLAGS by cargo — they don't conflict. Verify after implementation.

---

## Implementation Order

1. **Phase 3 first** (linker) — smallest change, lowest risk, immediate benefit
2. **Phase 2 second** (profile) — simple config change
3. **Phase 1 last** (consolidation) — biggest change, biggest win. Do one crate at a time:
   - basilisk-checker first (biggest payoff)
   - basilisk-resolver second
   - basilisk-lsp third

## Files Modified

- `Cargo.toml` — add `[profile.ci]`
- `.cargo/config.toml` — add lld linker config
- `.github/workflows/ci.yml` — install lld
- `.devcontainer/Dockerfile` — install lld
- `scripts/test.sh` — `--profile ci`, update binary paths
- `crates/basilisk-checker/tests/` — create `common/mod.rs`, create `checker/` subdir, create harness files, move 185 test files
- `crates/basilisk-resolver/tests/` — create `resolver/` subdir, create harness files, move 56 test files
- `crates/basilisk-lsp/tests/` — create `lsp/` subdir, create harness files, move 32 test files

## Verification

After each phase:
1. `cargo test --workspace` — all tests pass
2. `./scripts/test.sh` — full coverage pipeline passes all thresholds
3. `cargo fmt --all --check` — formatting clean
4. `cargo clippy --release --all-targets` — no warnings
5. Push and verify CI passes with reduced time

## Risks

- **Coverage numbers unchanged** — consolidation doesn't change which code executes, just how test binaries are structured
- **500 LOC limit** — harness files are ~30 lines; moved test files keep original sizes
- **Test name collisions** — unlikely since test functions are prefixed with error codes (e.g., `e0001_missing_param`)
- **`#![allow]` attributes** — must become `#[allow]` on module items or be placed on the harness file (applies to all submodules)

---

## Checklist

### Phase 3: Faster Linker
- [x] Add lld linker config to `.cargo/config.toml`
- [x] Install lld in `.github/workflows/ci.yml` (lint job)
- [x] Install lld in `.github/workflows/ci.yml` (test job)
- [x] Add lld to `.devcontainer/Dockerfile`
- [ ] Verify RUSTFLAGS env var merges correctly with config.toml rustflags
- [ ] Push and verify CI passes

### Phase 2: CI Profile
- [x] Add `[profile.ci]` to root `Cargo.toml`
- [x] Update `scripts/test.sh`: `cargo llvm-cov` to use `--profile ci`
- [x] Update `scripts/test.sh`: compiler E2E tests to use `--profile ci`
- [x] Update `scripts/test.sh`: Zed extension tests to use `--profile ci`
- [x] Update `scripts/test.sh`: binary path from `release/` to `ci/`
- [x] Update `scripts/test.sh`: `cargo llvm-cov report` commands to use `--profile ci`
- [ ] Push and verify CI passes with coverage thresholds intact

### Phase 1A: Consolidate basilisk-checker tests (198 -> 13 binaries)
- [x] Delete `coverage_boost_15_tests.rs.backup`
- [x] Create `crates/basilisk-checker/tests/common/mod.rs` with shared `run()`, `codes()`, `codes_owned()`
- [x] Create `crates/basilisk-checker/tests/checker/` subdirectory
- [x] Move 38 `coverage_boost_*_tests.rs` files to `checker/` subdir
- [x] Create `coverage_boost_tests.rs` harness file
- [x] Move e0001-e0025 test files to `checker/` subdir
- [x] Create `e0001_e0025_tests.rs` harness file
- [x] Move e0026-e0050 test files to `checker/` subdir
- [x] Create `e0026_e0050_tests.rs` harness file
- [x] Move e0051-e0075 test files to `checker/` subdir
- [x] Create `e0051_e0075_tests.rs` harness file
- [x] Move e0076-e0100 test files to `checker/` subdir
- [x] Create `e0076_e0100_tests.rs` harness file
- [x] Move e0101-e0125 test files to `checker/` subdir
- [x] Create `e0101_e0125_tests.rs` harness file
- [x] Move e0126-e0149 + w0040/w0050 test files to `checker/` subdir
- [x] Create `e0126_e0149_tests.rs` harness file
- [x] Move categorical test files to `checker/` subdir
- [x] Create `categorical_tests.rs` harness file
- [x] Move inference/types test files to `checker/` subdir
- [x] Create `inference_all_tests.rs` harness file
- [x] Remove `run()`/`codes()` boilerplate from all moved files, use `super::common::*`
- [x] Convert `#![allow(...)]` inner attributes to harness-level attributes
- [x] Run `cargo test -p basilisk-checker` — all tests pass
- [x] Run `cargo fmt --all --check` — clean

### Phase 1B: Consolidate basilisk-resolver tests (65 -> 9 binaries)
- [x] Create `crates/basilisk-resolver/tests/resolver/` subdirectory
- [x] Move test files to `resolver/` subdir
- [x] Create 9 harness files grouped by topic
- [x] Update module imports in moved files
- [x] Run `cargo test -p basilisk-resolver` — all tests pass

### Phase 1C: Consolidate basilisk-lsp tests (40 -> 5 binaries)
- [x] Create `crates/basilisk-lsp/tests/lsp/` subdirectory
- [x] Move test files and common modules to `lsp/` subdir
- [x] Create 5 harness files (ws_core, ws_features, ws_navigation, lsp_stdio, zed)
- [x] Run `cargo test -p basilisk-lsp` — all tests pass

### Final Verification
- [x] `cargo test --workspace` — all tests pass
- [ ] `./scripts/test.sh` — full coverage pipeline passes
- [x] `cargo fmt --all --check` — clean
- [ ] `cargo clippy --release --all-targets` — no warnings
- [ ] Push and verify CI time is under 10 minutes
