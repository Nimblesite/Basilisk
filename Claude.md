# CLAUDE.md

⚠️ CRITICAL: DO NOT USE GIT!!!!

⚠️ CRITICAL: WE TREAT THIS CODEBASE WITH RESPECT. THIS CODE WOULD PASS REVIEW AT Google, Meta and Microsoft. WE DON'T ALLOW BAD CODE. NOT EVEN FOR ONE LINE. THIS CODEBASE RECEIVES A GRADE OF A+. ANYTHING LESS IS ⛔️ILLEGAL AND YOU MUST FIX IT IMMEDIATELY.

Target: 100% PEP conformance. Canonical version: **Python 3.12**. Read the PEP conformance readme carefully.

The project is a Python type checker and comprehensive LSP (test explorer, debugging, profiling, autofixes) built in Rust.

**Overall aim: FIX THE PYTHON DEVELOPER EXPERIENCE.**
One IDE extension = COMPLETE PYTHON DEVELOPMENT EXPERIENCE. SEAMLESS, FAST, COMPLETE.

# Too Many Cooks - MANDATORY

⚠️ REGISTER IMMEDIATELY!!!

COORDINATOR: dictate orders through plans and messages. DELEGATE!!!
OTHERS: do exactly as the coordinator says. CONSTANTLY CHECK MESSAGES AND COMPLY!!!

- Lock files before editing. Don't edit locked files.
- Respond to messages quickly. Others are waiting.

# Documentation Structure

- `docs/specs/` — All specifications
- `docs/plans/` — Implementation plans
- `docs/` — Standalone docs (PEP conformance, stub strategy, etc.)

`docs/specs/LSP-SPEC.md` is the **single source of truth** for all shared LSP/DAP/config/commands. Editor-specific specs point back to it.

# Critical Docs

- [Python type system spec](https://typing.python.org/en/latest/spec/index.html)
- [PEP Conformance](https://github.com/python/typing/blob/main/conformance/README.md)
- [Pyrefly](https://pyrefly.org/en/docs/) | [Pyright](https://microsoft.github.io/pyright/#/) (competitors)

# Rules

- 
   allow(clippy = ⛔️ ILLEGAL. If you have to do this, you better add a damn good reason!!! 
   **aggressively remove** allow from the code!!!
- Zero duplication. DRY AF!!! Check for existing code before writing new code
- Aggressively move code that can be shared out to shared crates/modules/packages
- Keep the dependencies and versions in these two files in sync at all times: .github/workflows/ci.yml, .devcontainer/Dockerfile
- Ignore compiler code (except clippy fixes)
- Do not use Git unless asked
- There is NO SUCH THING AS LEGACY CODE in this codebase. Legacy = DELETED
- Regex = ⛔️ ILLEGAL. Use the proper parsing mechanism - usually ruff
- Keep files under 500 LOC. Break up larger files.
- Copying files is illegal. MOVE them instead.

## Testing

Testing is absolutely critical. We aim for 100% test coverage and a high mutation score at all times. Focus on assertions; not just coverage

- NEVER DELETE FAILING TESTS
- NEVER REMOVE ASSERTIONS THAT CAUSE TEST FAILURES
- ADD more failing tests for broken/missing functionality — NEVER remove them
- REDUCING TEST ASSERTIVENESS = DATA CENTER DISMANTLED
- Ignoring tests = ILLEGAL

## Core Principles

- Logging is critical. Can't see what's happening? Add more logging immediately
- 100% test coverage is only the start
- No unit tests. Only COARSE e2e tests

## Rust Quality Standards

- Run clippy and fmt routinely, fix violations immediately
- All lints at highest strictness (see Cargo.toml `[lints]`)
- Add lints to Cargo.toml if in doubt. Never remove.
- `unsafe` code forbidden (`unsafe_code = "deny"`)
- `unwrap()` is ALWAYS a violation. Use `?` with proper error types
- No `panic!`, `todo!`, `unimplemented!` — handle all cases, return `Result<T,E>`

## Functional Programming Style

- `Result<T,E>` and `Option<T>` everywhere
- Expressions over statements — `match`, `if let`, iterator chains
- Pure functions, minimize side effects
- Pattern matching over casting or unwrapping
- Early returns with `?` for clean error propagation

## Code Structure

- Small, focused functions (<20 lines)
- Low cognitive complexity (clippy::cognitive_complexity enabled)
- Descriptive variable names (no single letters except in closures)
- Group related functionality into modules
- Public APIs must have documentation

## Bug Fix Process

1. Write a test that fails because of the bug
2. Run the test — confirm it fails BECAUSE of the bug
3. Repeat until it's failing for the right reason
4. Fix the bug (do NOT change the test)
5. Run the test — confirm it passes

## Website and CSS

- **MINIMIZE CSS CLASSES** — consolidate where possible
- Name classes after what the element IS, not what section it's in
- **Do not use common LLM colors like purple** — use RNG and color wheels

## What Basilisk Is

Strict-by-default Python type checker and comprehensive LSP. Built in **Rust** — no runtime required.

Mojo-inspired but different goals. Includes early-stage Python compiler (compiled subset of Python, GPU support), but the core aim is the type checker and LSP. Basilisk remains a typed subset of Python — it will not deviate from the language, only remove dynamic typing.

## Goal

Make the Python dev experience amazing in any LSP-based IDE. Users can turn it on any time, flick errors down to warnings, and incrementally move towards type safety. Or just use the LSP for autofixes, formatting, debugging, and profiling. One-stop-shop — no frankenstein of analyzers. Strict typing can be switched off for basic scripts.

## Key Architecture (from docs/specs/SPEC.md)

- **Parser**: `ruff_python_parser` (MIT, same as Ruff)
- **Incremental**: Salsa framework (same as rust-analyzer) — sub-10ms incremental checks
- **LSP**: `lsp-server` or `tower-lsp`
- **Linting/formatting**: Ruff CLI subprocess — not reimplemented
- **Plugins**: WASM-based
- **Parallelism**: Rayon (work-stealing, file-level)
- **No Pyright/mypy/Node.js** — zero TypeScript or Python runtime

## Diagnostic Code Convention

Error codes: `BSK-E####` / `BSK-W####`, rustc-style output:
```
error[BSK-E0001]: Missing parameter type annotation
  --> src/utils.py:14:5
   |
14 | def process(data):
   |             ^^^^ parameter `data` has no type annotation
```

Ranges (defined in docs/specs/SPEC.md):
- `E0001–E0025`: Core type errors
- `E003x`: Ownership violations
- `E004x`: Immutability violations
- `E005x / E006x`: Structural discipline and implicit coercion

## Alternative Ecosystems

Pyright is the gold standard to compare against. [View code](https://github.com/microsoft/pyright) as reference, but NEVER copy from the Pyright codebase.

## Testing Strategy (per docs/specs/SPEC.md)

### Layering

1. **E2E** — most tests. Run ACTUAL analyzers, check correct results. LSP and VSIX combined for full user experience testing.
2. **Integration** — test full components (analyzers etc.) standalone from CLI.
3. **Unit** — minimal. Only for isolating logic and regression pins.

| Layer | Mechanism | What it checks |
|---|---|---|
| **E2E** | `tests/` in `basilisk-cli`; real `.py` fixtures through full stack | Correct diagnostics from full pipeline |
| **Integration** | `tests/` in each crate | Crates behave correctly wired together |
| **Unit** | `#[cfg(test)]` modules in `src/` | Edge cases and regression pins only |
| **Conformance** | Python typing test suite via `cargo test` | PEP compliance (target: 100%) |
| **Golden file** | Diagnostic snapshots in repo | Silent regression detection |
| **Mutation** | `cargo-mutants` | Tests actually catch bugs |
| **Fuzzing** | `cargo-fuzz` (nightly) | No crashes on garbage input |
| **Benchmarks** | `criterion` | <10ms incremental, <5s cold on 100K LOC |

### PEP Conformance

Currently ~83%. Target 100%, but VSIX/IDE experience is the immediate priority. Conformance tests live in `crates/basilisk-cli/tests/conformance/`.

### Mutation Testing

Run `sh scripts/mutate.sh`. Occasionally run and remove tests that cause hanging.

### Benchmarking

Performance is critical. Keep benchmarks running, improve slow rules, keep pace with Pyrefly. Add rules to benchmarks when fully operational.
