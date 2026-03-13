⚠️ CRITICAL: DO NOT USE GIT!!!!

This file provides guidance when working with code in this repository. We are shooting for 100% PEP conformance. The conformance test suite requires **Python 3.12** — this is the canonical target version for the entire project. Read the PEP conformance readme carefully.

The project encompasses a Python type checker, as well as a comprehensive LSP that includes test explorer integration, debugging, and profiling. There are many autofixes and these are a critical aspect of the overall development experience.

Overall aim: FIX THE PYTHON DEVELOPER EXPERIENCE.

One IDE extension = COMPLETE PYTHON DEVELOPMENT EXPERIENCE. Seamless, fast, complete

# Too Many Cooks - MANDATORY

COORDINATOR: dictate the orders to others through plans and messages
OTHERS: do exactly as the coordinator says. CONSTANTLY CHECK YOUR MESSAGES AND DO WHAT THE COORDINATOR SAYS!!!

- Lock files before editing. Don't edit locked files.
- Respond to messages quickly. Others are waiting for you

# Documentation Structure

- `docs/specs/` — All specifications (project spec, LSP spec, editor extension specs, compiler spec, etc.)
- `docs/plans/` — Implementation plans for features and integrations
- `docs/` — Standalone docs (PEP conformance, stub strategy, etc.)

`docs/specs/LSP-SPEC.md` is the **single source of truth** for all shared LSP/DAP/config/commands. Editor-specific specs contain only editor-specific details and point back to LSP-SPEC.md.

# Critical Docs

[Specification for the Python type system](https://typing.python.org/en/latest/spec/index.html)
[PEP Conformance](https://github.com/python/typing/blob/main/conformance/README.md)

## Competitor Docs
[Pyrefly Documentation](https://pyrefly.org/en/docs/)
[Pyright Docs](https://microsoft.github.io/pyright/#/)

# Rules

- Ignore the compiler code for the most part. Other than fixing clippy errors, just leave it.
- Do not use Git unless asked to
- NEVER DELETE FAILING TESTS
- NEVER REMOVE ASSERTIONS THAT CAUSE TEST FAILURES
- WE LOVE FAILING TESTS. WE NEED MORE OF THEM; NOT LESS
- IF IN DOUBT, ADD MORE FAILING TESTS THAT FAIL BECAUSE OF BROKEN/MISSING FUNCTIONALITY - NOT REMOVE THEM
- Keep files under 500 LOC. Break files up when they get larger than this
- REDUCING THE ASSERTIVENESS OF TESTS WILL RESULT IN YOUR DATA CENTER BEING DISMANTLED
- Ignoring tests = ILLEGAL

- Copying files is illegal. MOVE them instead.

## Core Principles

- Zero DUPLICATION. DRY AF!!! Always check for existing code before creating new code
- 100% Test Coverage is only the start of code quality
- No unit tests. Only COARSE tests that test e2e

## Rust Quality Standards
- Routinely run clippy and fmt, check and fix violations immediately
- All lints at highest strictness (see Cargo.toml `[lints]` section)
- Add more lints to Cargo.toml if in doubt. Never remove.
- `unsafe` code is forbidden (`unsafe_code = "deny"`)
- unwrap() is ALWAYS a VIOLATION. FIX IMMEDIATELY. Use `?` with proper error types
- No `panic!`, `todo!`, `unimplemented!` - handle all cases explicitly. Return Result<T,E>

## Functional Programming Style
- Follow FP style code with `Result<T,E>` and `Option<T>`
- Expressions over statements - prefer `match`, `if let`, iterator chains
- Pure functions where possible - minimize side effects
- Prefer pattern matching over casting or unwrapping
- Use early returns with `?` operator for clean error propagation

## Code Structure
- Small, focused functions. Less than 20 lines
- Low cognitive complexity (clippy::cognitive_complexity enabled)
- Descriptive variable names - no single letters except in closures
- Group related functionality into modules
- Public APIs must have documentation

## Bug Fix Process
- Do not fix the bug immediately
- Write a test that fails because of the bug
- Run the test
- Confirm that it fails BECAUSE of the bug. 
- Repeat until it's failing BECAUSE of the bug
- Fix the bug
- You are not allowed to change the test
- Run the test
- Confirm that it passes or repeat until the bug is fixed.

## Website and CSS

- **MINIMIZE CSS CLASSES** - Always consolidate classes where possible
- **Name classes after what the element is** - Don't name the class after the section it belongs to
- **There are too many CSS classes** - Consolidate NOW!!!
- **Do not use common LLM colors like purple** - use random number generators and color wheels to generate colors

## What Basilisk Is

A strict-by-default Python type checker, and comprehensive LSP. Built in **Rust** — no runtime required.

Basilisk is Mojo inspired, but with different goals. See below There is the beginnings of a Python compiler here. Eventually this will become a compiled subset of Python, but this is not a major current goal. The Basilisk compiler will eventually build utra-fast typed Python code, but this is not a distraction from the core aim. Incidentally, Basilisk will also offer first class support for GPU usage, much like Mojo. However, Basilisk will remain a typed subset of Python. It will not deviate from the Python language, other than removing dynamic typing from the language.

## Goal

Basilisk's goal is to make the Python development experience amazing in any LSP based IDE. The user can turn it on any time and easily flick the current errors down to warning so that they can incrementally move towards a type safe Python codebase. 

Or, just use the top tier LSP experience for autofixes, formatting, debugging, and extras like profiling. Python shouldn't be a frankenstein of different analyzers and components. Basilisk is a one-stop-shop for Python. The user can switch off strict typing so that it's still an amazing experience for basic Python scripts with no typing.

## Key Architecture Decisions (from docs/specs/SPEC.md)

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

Key diagnostic ranges defined in docs/specs/SPEC.md:
- `E0001–E0025`: Core type errors (missing annotations, type mismatches, unknown types)
- `E003x`: Ownership violations (mutation of Borrowed, use-after-move, implicit copy of large struct)
- `E004x`: Immutability violations (mutation of immutable param, reassignment of parameter)
- `E005x / E006x`: Structural discipline and implicit coercion

## Alternative Ecosystems

Pyright is the gold standard that Basilisk must compare itself to. You can [view the code here](https://github.com/microsoft/pyright) as a reference, but NEVER copy any of the code from the Pyright codebase.

## Testing Strategy (per docs/specs/SPEC.md)

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

### PEP Conformance

We are currently at around 83%. We are shooting for 100% conformance, but first, our aim is to get the VSIX and other IDE experiences up to a top tier level.

To run the conformance suite locally:
```
python3.12 -m venv .venv
source .venv/bin/activate
pip install -r conformance/requirements.txt
```

### Mutation Testing (`cargo-mutants`)

`cargo-mutants` is the standard Rust mutation testing tool. It mutates your code (flips operators, removes return values, etc.) and verifies that at least one test fails for each mutation. If no test fails, the tests are not testing what they claim to test.

Run locally:
```bash
sh scripts/mutate.sh
```

Ocassionally run the mutation tests and remove tests from the list that cause hanging.

### Benchmarking

Performance is critical. Keep these benchmarks running. Check them occasionally, and aim towards improving time on slow rules. In particular, we want to keep pace with Pyrefly. Please add extra rules to the benchmarks when we know they are fully operational