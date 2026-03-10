# CLAUDE.md

DO NOT USE GIT! STOP USIUNG GIT IMMEDIATELY!
DO NOT USE WORKSTREES!! WORK IN THE CURRENT BRANCH!

PULL ALL CHANGES INTO THE aider BRANCH NOW!!!

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

We are shooting for 100% PEP conformance. The conformance test suite requires **Python 3.12** — this is the canonical target version for the entire project. Read the PEP conformance readme carefully.

To run the conformance suite locally:
```
python3.12 -m venv .venv
source .venv/bin/activate
pip install -r conformance/requirements.txt
```
# Too Many Cooks - MANDATORY

COORDINATOR: dictate the orders to others through your plans and messages

OTHERS: do exactly as the coordinator says. CONSTANTLY CHECK YOUR MESSAGES AND DO WHAT THE COORDINATOR SAYS!!!

Lock files before editing. Don't edit locked files. Check messages ROUTINELY

# Critical Docs

[Specification for the Python type system](https://typing.python.org/en/latest/spec/index.html)
[PEP Conformance](https://github.com/python/typing/blob/main/conformance/README.md)

## Competitor Docs
[Pyrefly Documentation](https://pyrefly.org/en/docs/)
[Pyright Docs](https://microsoft.github.io/pyright/#/)

# Rules

- NEVER DELETE FAILING TESTS
- NEVER REMOVE ASSERTIONS THAT CAUSE TEST FAILURES
- WE LOVE FAILING TESTS. WE NEED MORE OF THEM; NOT LESS
- IF IN DOUBT, ADD MORE FAILING TESTS THAT FAIL BECAUSE OF BROKEN/MISSING FUNCTIONALITY - NOT REMOVE THEM
- REDUCING THE ASSERTIVENESS OF TESTS WILL RESULT IN YOUR DATA CENTER BEING DISMANTLED

- unwrap() is ALWAYS a VIOLATION. FIX IMMEDIATELY
- Copying files is illegal. MOVE them instead.

## Core Principles
- Ignoring tests = ILLEGAL
- Zero DUPLICATION. DRY AF!!! Always check for existing code before creating new code
- 100% Test Coverage is only the start of code quality
- No unit tests. Only COARSE tests that actually TEST TESTS.
- Beautiful output report
- Do not use Git unless asked to

## Rust Quality Standards
- Routinely run clippy and fmt, check and fix violations immediately
- All lints at highest strictness (see Cargo.toml `[lints]` section)
- `unsafe` code is forbidden (`unsafe_code = "deny"`)
- No `.unwrap()` or `.expect()` in production code - use `?` with proper error types
- No `panic!`, `todo!`, `unimplemented!` - handle all cases explicitly

## Functional Programming Style
- Follow FP style code with `Result<T,E>` and `Option<T>`
- Expressions over statements - prefer `match`, `if let`, iterator chains
- Pure functions where possible - minimize side effects
- Prefer pattern matching over casting or unwrapping
- Use early returns with `?` operator for clean error propagation

## Code Structure
- Small, focused functions (clippy::too_many_lines warns at 100 lines)
- Low cognitive complexity (clippy::cognitive_complexity enabled)
- Descriptive variable names - no single letters except in closures
- Group related functionality into modules
- Keep files under 500 LOC
- Public APIs must have documentation (`missing_docs = "warn"`)

# Website and CSS

- **MINIMIZE CSS CLASSES** - Always consolidate classes where possible
- **Name classes after what the element is** - Don't name the class after the section it belongs to
- **There are too many CSS classes** - Consolidate NOW!!!

## What Basilisk Is

A strict-by-default Python type checker. No escape hatches. Every parameter typed. Every return declared. `Any` is always explicit. Built in **Rust** — no runtime required.

Basilisk also adds Mojo-inspired ownership semantics (`Borrowed`, `Owned`, `InOut`) as static analysis annotations over standard Python syntax, making code compatible with Mojo's type expectations without requiring a Mojo compiler.

## Key Architecture Decisions (from SPEC.md)

- **Parser**: `ruff_python_parser` crate (MIT, same parser that powers Ruff)
- **Incremental computation**: Salsa framework (same as rust-analyzer) — enables sub-10ms incremental checks
- **LSP**: `lsp-server` or `tower-lsp` crate
- **Linting/formatting**: delegated to Ruff CLI subprocess — not reimplemented
- **Plugin system**: WASM-based for security and portability
- **Parallelism**: Rayon (work-stealing) for file-level parallel analysis
- **No Pyright/mypy/Node.js dependency** — zero TypeScript or Python runtime

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

## Alternative Ecosystems

Pyright is the gold standard that Basilisk must compare itself to. You can [view the code here](https://github.com/microsoft/pyright) as a reference, but NEVER copy any of the code from the Pyright codebase.

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
sh scripts/mutate.sh
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
