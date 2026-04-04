# CLAUDE.md

⚠️ CRITICAL: WE TREAT THIS CODEBASE WITH RESPECT. THIS CODE WOULD PASS REVIEW AT Google, Meta and Microsoft. WE DON'T ALLOW BAD CODE. NOT EVEN FOR ONE LINE. THIS CODEBASE RECEIVES A GRADE OF A+. ANYTHING LESS IS ⛔️ILLEGAL AND YOU MUST FIX IT IMMEDIATELY.

⚠️ KEY DESIGN PRINCIPLE: LSP DRIVES THE FUNCTIONALITY - NOT THE IDE EXTENSION
⚠️ IDE EXTENSIONS LISTEN FOR THINGS LIKE COMMANDS FROM THE LSP STATE CHANGE AND ADJUST ACCORDINGLY
⚠️ THE IDE EXTENSIONS NEVER REGISTERS COMMANDS ETC THAT THE LSP DOESN'T ADVERTISE

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

- `docs/INDEX.md` — Full index of all docs
- `docs/specs/` — Specifications (naming: `[COMPONENT]-[FEATURE]-SPEC.md`)
- `docs/plans/` — Implementation plans (naming: `[COMPONENT]-[FEATURE]-PLAN.md`)

`docs/specs/LSP-ARCHITECTURE-SPEC.md` is the **single source of truth** for all shared LSP/DAP/config/commands. Editor-specific specs point back to it.

- Specs MUST have non-numeric, hierarchically structured IDs
- Code and tests MUST reference the spec ids

# Critical Docs

- [Python type system spec](https://typing.python.org/en/latest/spec/index.html)
- [PEP Conformance](https://github.com/python/typing/blob/main/conformance/README.md)
- [Pyrefly](https://pyrefly.org/en/docs/) | [Pyright](https://microsoft.github.io/pyright/#/) (competitors)
- [Python Type System Conformance Test Results](https://github.com/python/typing/blob/main/conformance/results/results.html) <- We are going to get listed here (interesting article: https://sinon.github.io/future-python-type-checkers/#zuban-from-david-halter)


# Rules

- TOP PRIORITY: REDUCE CODE DUPLICATION. ALWAYS MERGE SIMILAR CODE. ALWAYS SEARCH FOR CODE BEFORE ADDING NEW CODE. 
- Zero duplication. DRY AF!!! Check for existing code before writing new code
- Aggressively move code that can be shared out to shared crates/modules/packages
- CENTRALIZE ALL GLOBAL STATE
- Each app has a single file for global state. No state must exit outside this file.
- allow(clippy = ⛔️ ILLEGAL. 
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

## IDE Extension Testing

- VSIX tests must not call things like `whenCommandReady` or `vscode.commands.getCommands(true)` to check the existence. The core code must do this and the tests must assert the command exists through the UI or worst case internal VSIX state

## Core Principles

- Logging is critical. Can't see what's happening? Add more logging immediately
- DRY, DRY, DRY
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

## Architecture

Strict-by-default Python type checker and comprehensive LSP built in **Rust**. One IDE extension = complete Python dev experience. Users can flick errors down to warnings and incrementally adopt type safety, or just use the LSP for autofixes, formatting, debugging, and profiling.

- **Parser**: `ruff_python_parser` (MIT, same as Ruff)
- **Incremental**: Salsa framework — sub-10ms incremental checks
- **Linting/formatting**: Ruff CLI subprocess — not reimplemented
- **Parallelism**: Rayon (work-stealing, file-level)
- **No Pyright/mypy/Node.js** — zero TypeScript or Python runtime

Diagnostic codes: `BSK-E####` / `BSK-W####`. Pyright is the gold standard to compare against — NEVER copy from the Pyright codebase.

See `docs/specs/CHECKER-ARCHITECTURE-SPEC.md` for full architecture, diagnostic ranges, and testing strategy.
