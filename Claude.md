# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

# Rules

## Core Principles
- Ignoring tests = ILLEGAL
- Zero DUPLICATION. DRY AF!!! Always check for existing code before creating new code
- 100% Test Coverage is only the start of code quality
- No unit tests. Only COARSE tests that actually TEST TESTS.
- Beautiful output report
- Do not use Git unless asked to

## Rust Quality Standards
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

| Layer | Tool | Purpose |
|---|---|---|
| Unit tests | `cargo test` | Per-crate correctness |
| Integration tests | Multi-file scenarios | Cross-module type checking |
| Conformance tests | Python typing test suite | PEP compliance (target: 100%) |
| Golden file tests | Expected diagnostic output | Regression |
| Fuzzing | `cargo-fuzz` | Crash resistance |
| Property tests | `proptest` crate | Type system invariants |
| Benchmarks | Criterion | Performance regression gates |

Performance targets: PyTorch (600K LOC), Django (250K LOC), FastAPI (30K LOC), stdlib (500K LOC).

## Development Roadmap Phases

1. Foundation — parser, name resolver, basic type checker, CLI
2. LSP and VS Code extension
3. Strict-by-default (all E0001–E0025 rules, 80% PEP conformance)
4. Mojo safety annotations (ownership, immutability)
5. WASM plugin system + Django/Pydantic/SQLAlchemy plugins
6. Production hardening (95%+ PEP, SARIF/JUnit output)
7. Ecosystem (plugin marketplace, community stubs)
