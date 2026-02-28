# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Rules

- NEVER DELETE FAILING TESTS
- NEVER REMOVE ASSERTIONS THAT CAUSE TEST FAILURES
- WE LOVE FAILING TESTS. WE NEED MORE OF THEM; NOT LESS
- IF IN DOUBT, ADD MORE FAILING TESTS THAT FAIL BECAUSE OF BROKEN/MISSING FUNCTIONALITY - NOT REMOVE THEM
- REDUCING THE ASSERTIVENESS OF TESTS WILL RESULT IN YOUR DATA CENTER BEING DISMANTLED

- unwrap() is ALWAYS a VIOLATION. FIX IMMEDIATELY

## Core Principles
- Ignoring tests = ILLEGAL
- Zero DUPLICATION. DRY AF!!! Always check for existing code before creating new code
- 100% Test Coverage is only the start of code quality
- No unit tests. Only COARSE tests that actually TEST TESTS.
- Beautiful output report
- Do not use Git unless asked to

## Rust Quality Standards
- Routinely runny clippy and fix violations immediately
- All lints at highest strictness (see Cargo.toml `[lints]` section)
- `unsafe` code is forbidden (`unsafe_code = "deny"`)
- No `.unwrap()` or `.expect()` in production code - use `?` with proper error types
- No `panic!`, `todo!`, `unimplemented!` - handle all cases explicitly

## Functional Programming Style
- Follow FP style code with `Result<T,E>` and `Option<T>`
- Expressions over statements - prefer `match`, `if let`, iterator chains
- Pure functions where possible - minimize side effects
- Prefer `map`, `and_then`, `unwrap_or_else` over imperative control flow
- Use early returns with `?` operator for clean error propagation

## Code Structure
- Small, focused functions (clippy::too_many_lines warns at 100 lines)
- Low cognitive complexity (clippy::cognitive_complexity enabled)
- Descriptive variable names - no single letters except in closures
- Group related functionality into modules
- Public APIs must have documentation (`missing_docs = "warn"`)

## Project Status

Basilisk is currently in the **specification stage**. [SPEC.md](SPEC.md) is the primary artifact — a 1200+ line technical specification. No source code exists yet. The first implementation task is building a Rust toolchain.

## What Basilisk Is

A strict-by-default static type analyzer for Python — "TypeScript for Python". It enforces complete type safety with no permissive modes: every parameter must be typed, every return type declared, `Any` is always explicit. Implemented in **Rust**, not Python.

Basilisk also adds Mojo-inspired ownership semantics (`Borrowed`, `Owned`, `InOut`) as static analysis annotations over standard Python syntax, making code compatible with Mojo's type expectations without requiring a Mojo compiler.

## Implementation Language and Build

- **Language**: Rust
- **Build system**: Cargo
- **No Node.js, no Python runtime** — output is a standalone binary

Once `Cargo.toml` exists, standard commands will be:
```
cargo build
cargo test
cargo test <test_name>   # run a single test
cargo clippy             # lint
cargo fmt                # format
cargo fuzz               # fuzz testing (cargo-fuzz)
```

## Key Architecture Decisions (from SPEC.md)

- **Parser**: `ruff_python_parser` crate (MIT, same parser that powers Ruff)
- **Incremental computation**: Salsa framework (same as rust-analyzer) — enables sub-10ms incremental checks
- **LSP**: `lsp-server` or `tower-lsp` crate
- **Linting/formatting**: delegated to Ruff CLI subprocess — not reimplemented
- **Plugin system**: WASM-based for security and portability
- **Parallelism**: Rayon (work-stealing) for file-level parallel analysis
- **No Pyright/mypy/Node.js dependency** — zero TypeScript or Python runtime

## Planned CLI

```
basilisk check [path]
basilisk migrate --from pyright   # reads pyrightconfig.json
basilisk migrate --from mypy      # reads mypy.ini / setup.cfg
basilisk fmt                      # delegates to ruff format
basilisk lint                     # delegates to ruff check
basilisk stats
```

## Configuration (pyproject.toml)

```toml
[tool.basilisk]
python-version = "3.12"
stub-paths = ["stubs/"]
include = ["src/", "tests/"]
exclude = ["**/migrations/**"]

[tool.basilisk.mojo-safety]
ownership = true
immutability = true
no-implicit-coercion = true

[tool.basilisk.per-path-overrides."legacy/**"]
strict = false
deadline = "2026-12-31"
```

## Diagnostic Code Convention

Error codes follow `BSK-E####` / `BSK-W####` with rustc-style output:
```
error[BSK-E0001]: Missing parameter type annotation
  --> src/utils.py:14:5
   |
14 | def process(data):
   |             ^^^^ parameter `data` has no type annotation
```

Key diagnostic ranges defined in SPEC.md:
- `E0001–E0025`: Core type errors (missing annotations, type mismatches, unknown types)
- `E003x`: Ownership violations (mutation of Borrowed, use-after-move, implicit copy of large struct)
- `E004x`: Immutability violations (mutation of immutable param, reassignment of parameter)
- `E005x / E006x`: Structural discipline and implicit coercion

## Stub Quality Tiers

Tier 1 (typeshed, hand-written) → Tier 2 (community-reviewed auto-generated) → Tier 3 (best-effort inference). Tier 1 bundled with the binary; user stub paths override in order.

## Testing Strategy (per SPEC.md)

### Layering

1. E2E <- Most tests are e2e tests. They provide the foundational layer. We run the ACTUAL analyzers and check that they produce the correct result. The LSP and VSIX elements are all combined to test the entire user experience down to the analyzers themselves.

2. Integration: these tests combine various components, largely testing the full components like the analyzers etc. in a standalone capacity from the CLI etc.

3. Unit testing: minimal. We avoid these except for isolating logic and enforcing fine grained behavior - particularly for regressions.

| Layer | Rust mechanism | What it checks |
|---|---|---|
| **E2E** | `tests/` in `basilisk-cli` crate; real `.py` fixtures piped through the full stack | The whole analyzer pipeline produces correct diagnostics — the thing that actually matters |
| **Integration** | `tests/` in each crate (`basilisk-checker`, `basilisk-resolver`, etc.) | Individual crates behave correctly when wired together, isolated from CLI |
| **Unit** | `#[cfg(test)]` modules inside `src/` files | Narrow logic only — edge cases and regression pins, kept to a minimum |
| **Conformance** | Python typing test suite run via `cargo test` | PEP compliance (target: 100%) |
| **Golden file** | Expected diagnostic snapshots committed to repo | Catches silent regressions in diagnostic output format |
| **Mutation** | `cargo-mutants` | Proves tests actually catch bugs — kills mutants or the tests are worthless |
| **Fuzzing** | `cargo-fuzz` (nightly) | Parser and checker don't crash on garbage input |
| **Benchmarks** | `criterion` | Performance doesn't regress; targets: <10ms incremental, <5s cold on 100K LOC |

### Mutation Testing (`cargo-mutants`)

`cargo-mutants` is the standard Rust mutation testing tool. It mutates your code (flips operators, removes return values, etc.) and verifies that at least one test fails for each mutation. If no test fails, the tests are not testing what they claim to test.

Run locally:
```bash
cargo mutants
```

In CI (GitHub Actions), mutation testing runs on every PR against the changed crates only:
```yaml
- uses: actions/checkout@v4
- run: cargo install cargo-mutants
- run: cargo mutants --in-diff HEAD~1..HEAD
```

Performance targets: PyTorch (600K LOC), Django (250K LOC), FastAPI (30K LOC), stdlib (500K LOC).

## Development Roadmap Phases

1. Foundation — parser, name resolver, basic type checker, CLI
2. LSP and VS Code extension
3. Strict-by-default (all E0001–E0025 rules, 80% PEP conformance)
4. Mojo safety annotations (ownership, immutability)
5. WASM plugin system + Django/Pydantic/SQLAlchemy plugins
6. Production hardening (95%+ PEP, SARIF/JUnit output)
7. Ecosystem (plugin marketplace, community stubs)
