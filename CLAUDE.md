<!-- agent-pmo:f87d349 -->
# CLAUDE.md

This codebase is held to a high standard: code here should comfortably pass review at a top-tier engineering organization. Please keep quality high and address shortcomings as you find them, rather than leaving them for later.

⚠️ USING GIT IS ⛔️ ILLEGAL ⚠️ SIGNING A GIT COMMIT WITH CLAUDE CODE AS A COAUTHOR IS SUPER ⛔️ ILEGAL ⚠️

⚠️ CONFORMANCE SCORE INCREASES MONOTONICALLY, BENCHMARKS AND FALSE POSITIVES DECREASE MONOTONICALLY ⚠️

⚠️ AVOID DUPLICATION OF ALL KINDS AND PRACTICE TOKEN ECONOMICS ⚠️

⚠️ DO NOT STOP TO ASK QUESTIONS. USE YOUR JUDGMENT WITHOUT ASKING THE USER ⚠️

⚠️ DO NOT KILL A VS Code PROCESS (including in the browser) — it disrupts active debugging and test sessions. ⚠️

Key design principles:

- We are building a better Python dev experience
- The LSP drives the functionality, not the IDE extension.
- IDE extensions react to signals from the LSP (commands, state changes) and adjust accordingly.
- IDE extensions never register commands the LSP doesn't advertise.

Target: 100% PEP conformance. Canonical version: **Python 3.12**. Read the PEP conformance readme carefully.

The project is a Python type checker and comprehensive LSP (test explorer, debugging, profiling, autofixes) built in Rust.

**Overall aim: improve the Python developer experience.**
One IDE extension should provide a complete, seamless, and fast Python development experience.

# Documentation Structure

- All spec sections **must** have unique, non-numeric, hierarchically structured spec IDs.
- All code **must** refer to a spec ID.
- All tests must cross-reference the spec ID and code.
- This is the fabric of the repository and how all information is linked — please treat it as non-negotiable.
- If you find code or tests not linked to a spec ID, fix it.
- If you find spec sections with no ID, add one.

- `docs/INDEX.md` — Full index of all docs
- `docs/specs/` — Specifications (naming: `[COMPONENT]-[FEATURE]-SPEC.md`)
- `docs/plans/` — Implementation plans (naming: `[COMPONENT]-[FEATURE]-PLAN.md`)

`docs/specs/LSP-ARCHITECTURE-SPEC.md` is the **single source of truth** for all shared LSP/DAP/config/commands. Editor-specific specs point back to it.

- Specs must have non-numeric, hierarchically structured IDs (`[GROUP-TOPIC]` / `[GROUP-TOPIC-DETAIL]`).
- Code and tests must reference the spec IDs in comments (e.g. `// Implements [LSP-HOVER]`) so `grep [LSP-` finds spec -> code -> tests in one shot.

# Critical Docs

- [Python type system spec](https://typing.python.org/en/latest/spec/index.html)
- [PEP Conformance](https://github.com/python/typing/blob/main/conformance/README.md)
- [Pyrefly](https://pyrefly.org/en/docs/) | [Pyright](https://microsoft.github.io/pyright/#/) (reference implementations)
- [Python Type System Conformance Test Results](https://github.com/python/typing/blob/main/conformance/results/results.html) (our goal is to be listed here; relevant background: https://sinon.github.io/future-python-type-checkers/#zuban-from-david-halter)

# Build Commands

Cross-platform GNU Make. On Windows: `choco install make` or use the one in Git for Windows.

```bash
make build   # compile everything
make test    # FAIL-FAST tests + coverage + threshold (ONLY test entry point)
make lint    # all linters/analyzers (no formatting)
make fmt     # format in place
make clean   # remove build artifacts
make ci      # lint + test + build (full CI simulation)
make setup   # post-create dev environment setup
```

**There are exactly 7 standard targets — please don't add others.** `make test` runs the test runner with its fail-fast flag, collects coverage, asserts measured >= threshold from `coverage-thresholds.json`, and exits non-zero on any failure. To debug a single test, invoke the runner directly — that is not a Makefile target.

**`make fmt`** formats code in-place. **`make lint`** runs linters/analyzers (read-only, no formatting). **`make test`** runs tests with coverage. Three separate targets — no overlap.

# Rules

- Top priority: reduce code duplication. Merge similar code, and search for existing code before adding new code.
- Use the Deslop MCP to check for existing similar code with `find-similar` before writing new code, and `top-offenders` after modifying code. Always merge duplicate code.
- Keep it DRY. Check for existing code before writing new code.
- Aggressively move code that can be shared out to shared crates/modules/packages.
- Centralize all global state. 
- All state that can change uses Signals for reactivity. No stale state on screen
- Each app has a single file for global state. No state should live outside this file.
- `allow(clippy = ...)` is not permitted.
- Keep the dependencies and versions in these two files in sync at all times: `.github/workflows/ci.yml`, `.devcontainer/Dockerfile`.
- Ignore compiler code (except clippy fixes).
- Don't use Git unless asked.
- Treat legacy code as code to be removed — there is no legacy code in this codebase.
- Avoid regex. Use the proper parsing mechanism — usually ruff.
- Keep files under 500 LOC. Break up larger files.
- Move files rather than copying them.

## Git & Branch Discipline

Git is off-limits unless you are explicitly asked. When git IS used:

- **Never push to `main` directly.** Every change ships via PR → CI green → merge. No exceptions.
- **Never list the agent as a commit co-author.** No `Co-Authored-By` trailer, no agent attribution.
- **Work on exactly ONE branch at a time.** Reuse the existing feature branch; never open a second.
- **Never start a new branch when a feature branch already exists.** Check first.
- **If multiple feature branches exist, merge them into one immediately**, before any other work.
- **Worktrees are forbidden.** Never run `git worktree`.

Auto-memory is OFF (`.claude/settings.json` → `"autoMemoryEnabled": false`). Every durable rule goes through a reviewed PR to this file — never auto-captured memory.

## Testing

Testing is critical. We aim for 100% test coverage and a high mutation score at all times. Focus on assertions, not just coverage.

- Never delete failing tests.
- Mutation score monotonically increases. Include more Rust code in the mutation testing suite over time: widen scope by adding `#[mutation_safe]` tests over more rules/functions. The gate ([CHKARCH-TESTING-MUTATION-RATCHET], baseline `mutation_testing/mutation_scores.json`) fails CI if the viable mutant pool shrinks, caught drops, missed/timeout rise, or kill rate drops.
- Never remove assertions that cause test failures.
- Add more failing tests for broken or missing functionality — never remove them.
- Don't reduce test assertiveness.
- Don't ignore tests.
- `make test` is FAIL-FAST — it stops at the first failure. Never use `--no-fail-fast`; it saves CI minutes.
- `make test` always computes coverage and enforces it. The threshold lives in `coverage-thresholds.json` at the repo root — not env vars, not GH repo variables, not CI YAML. Below threshold fails the pipeline. Ratchet only.

## Benchmarks

Pay attention to benchmarks — performance is a feature and conformance must never be traded for it, nor it for conformance. Both ratchets hold simultaneously ([CHKARCH-TESTING-BENCH-RATCHET]).

- Run `make bench` whenever you touch checker hot paths (resolver visitors, rule `check` loops, new conformance logic). It fails if basilisk gets >25% slower on any fixture vs the committed baseline `benchmarks/status/<machine>.csv`.
- A conformance fix that blows the benchmark gate is not done — optimise or restructure it.
- `BENCH_NO_GATE=1` baseline resets are for fixture-set changes only and must be justified in the PR description.

## IDE Extension Testing

- VSIX tests must not call things like `whenCommandReady` or `vscode.commands.getCommands(true)` to check for existence. The core code must do this, and the tests must assert the command exists through the UI or, worst case, internal VSIX state.

## Core Principles

- Logging is critical. If you can't see what's happening, add more logging.
- DRY, DRY, DRY.
- Use `deslop:find-similar` before creating new code and `deslop:top_offenders` after changing code.
- 100% test coverage is only the start.
- No unit tests. Only coarse e2e tests.

## Logging Standards

- **Structured logging only.** Never `println!`/`eprintln!` for diagnostics. Use `tracing` + `tracing-subscriber`.
- **Log at entry/exit of significant operations.** Levels: `error|warn|info|debug|trace`.
- **Structured fields, not string interpolation.** `tracing::info!(user_id = 42, action = "checkout")` — never format strings.
- **VS Code extension:** detailed logs to a file in the extension's state folder AND to the VS Code Output Channel.
- **Never log PII** (names, emails, phone, IPs) or secrets. Log `"key: present"` or a truncated hash, never the value.

## Rust Quality Standards

- Run clippy and fmt routinely; fix violations promptly.
- All lints at highest strictness (see Cargo.toml `[lints]`).
- Add lints to Cargo.toml if in doubt. Never remove.
- `unsafe` code is forbidden (`unsafe_code = "deny"`).
- `unwrap()` is always a violation. Use `?` with proper error types.
- No `panic!`, `todo!`, `unimplemented!` — handle all cases, return `Result<T,E>`.

## Functional Programming Style

- `Result<T,E>` and `Option<T>` everywhere.
- Expressions over statements — `match`, `if let`, iterator chains.
- Pure functions, minimize side effects.
- Pattern matching over casting or unwrapping.
- Early returns with `?` for clean error propagation.

## Code Structure

- Small, focused functions (<20 lines).
- Low cognitive complexity (clippy::cognitive_complexity enabled).
- Descriptive variable names (no single letters except in closures).
- Group related functionality into modules.
- Public APIs must have documentation.

## Bug Fix Process

1. Write a test that fails because of the bug.
2. Run the test — confirm it fails because of the bug.
3. Repeat until it's failing for the right reason.
4. Fix the bug (do not change the test).
5. Run the test — confirm it passes.

# Too Many Cooks — Multi-Agent Coordination

Please register before starting work.

- Coordinator: dictate orders through plans and messages, and delegate.
- Others: follow the coordinator's direction and check messages regularly.
- Lock files before editing. Don't edit locked files.
- Respond to messages promptly — others may be waiting.

## Website and CSS

- **Minimize CSS classes** — consolidate where possible.
- Name classes after what the element IS, not what section it's in.
- Avoid common LLM-default colors (e.g. purple) — use RNG and color wheels.

## Architecture

Strict-by-default Python type checker and comprehensive LSP built in **Rust**. One IDE extension = complete Python dev experience. Users can flick errors down to warnings and incrementally adopt type safety, or just use the LSP for autofixes, formatting, debugging, and profiling.

- **Parser**: `ruff_python_parser` (MIT, same as Ruff)
- **Incremental**: Salsa framework — sub-10ms incremental checks
- **Linting/formatting**: Ruff CLI subprocess — not reimplemented
- **Parallelism**: Rayon (work-stealing, file-level)
- **No Pyright/mypy/Node.js** — zero TypeScript or Python runtime

Diagnostic codes: `BSK-E####` / `BSK-W####`. Pyright is the reference implementation to compare against — never copy from the Pyright codebase.

See `docs/specs/CHECKER-ARCHITECTURE-SPEC.md` for full architecture, diagnostic ranges, and testing strategy.

## Migration to `lspkit`

The cross-cutting LSP scaffolding in this repo (tower-lsp setup, workspace index, file watcher + debouncer, diagnostics publication, capability builder, config loader) is being distilled into the generic `lspkit-*` workspace, maintained in the private repository [`Nimblesite/lsp_toolkit`](https://github.com/Nimblesite/lsp_toolkit).

**For new LSP infrastructure work:** prefer `lspkit-*` crates over reinventing it here.
**For changes to existing scaffolding in this repo:** flag in the PR description if the patch duplicates `lspkit` functionality, and reference the upstream crate.

Mapping (current → toolkit crate):

| Current path | Toolkit crate |
|---|---|
| `crates/basilisk-lsp/src/server/mod.rs:96` tower-lsp `Server` setup | `lspkit-server` (hand-rolled JSON-RPC + `Dispatcher` + `Capabilities`) — **note:** the toolkit does not depend on `tower-lsp` |
| `crates/basilisk-lsp/src/workspace.rs:39–116` `WorkspaceIndex` + import-graph invalidation | `lspkit-vfs` (`Vfs`, `DocumentUri`, incremental edits) + consumer-side index |
| `crates/basilisk-lsp/src/server/handlers/{navigation,features}.rs` handler split | `lspkit-server::Dispatcher::register` per method name |
| `crates/basilisk-lsp/src/server/init.rs:224–242` diagnostic publication | `lspkit-server::diagnostics::DiagnosticsBus` |
| `crates/basilisk-lsp/src/server/mod.rs:61,64` debounce constants + file-watcher loop | `lspkit-live::watcher::FileWatcher` + `lspkit-live::scheduler::spawn` |
| `crates/basilisk-lsp/src/config.rs:35–100` `WorkspaceConfig` loader | `lspkit-config::load_from_ancestor` (consumer supplies the file name + struct) |
| `crates/basilisk-lsp/tests/lsp/ws_test_common.rs` E2E fixture | (not yet in toolkit; harness crate is a v0.1 follow-up) |

Code in this repo is **not** being removed — it stays canonical until the toolkit matures. This note exists so future work reuses `lspkit` for new servers and avoids widening this repo's scaffolding.
