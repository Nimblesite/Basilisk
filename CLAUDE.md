<!-- agent-pmo:f87d349 -->
# CLAUDE.md

Code here must comfortably pass review at a top-tier engineering org. Fix shortcomings as you find them.

# Conformance Is the Prime Directive

Target: **100% conformance** with the [Python typing spec](https://typing.python.org/en/latest/spec/index.html), measured ONLY by the [python/typing conformance suite](https://github.com/python/typing/tree/main/conformance/tests) — nothing else. This outranks every other concern in this file. Read the [conformance README](https://github.com/python/typing/blob/main/conformance/README.md) carefully. Python-version boundaries apply only where the typing spec, an accepted PEP, or Python language semantics defines one; Basilisk has no canonical Python release.

⚠️ **Never touch the scoreboard — move the number by FIXING the checker.** FORBIDDEN: disabling/deleting/unregistering any rule, deleting rule source (`crates/basilisk-checker/src/rules/*.rs`), removing rules from `all_rules()`, rule-suppressing config (the legacy `basilisk.json` is no longer read), hand-editing `conformance/conformance_status.csv`, loosening `coverage-thresholds.json` (`threshold` / `max_false_positives`). See [CHKARCH-CONFORMANCE], [CHKARCH-CONFORMANCE-MODE]. ⚠️

⚠️ **One conformance path**, run fresh every CI run: `python3 conformance/run_conformance.py`. No step skippable — (1) `git clone` the tests **and** the harness from `python/typing@main` HEAD, no cache/committed fixtures; (2) clean `cargo build --release` from THIS checkout, never the PyPI wheel, never instrumented; (3) run the suite's OWN unmodified `conformance/src/main.py --only-run basilisk` (its `type_checker.py` ships the official `BasiliskTypeChecker`) against that binary via `BASILISK_BIN`, failing hard on ANY false positive or missed required error (100% / 0 FP); (4) regenerate `conformance_status.csv` from the harness's own `results/basilisk/*.toml`. A vendored scorer, reimplemented/injected adapter, cached fixtures, or committed results standing in for a live run is a **BUILD FAILURE**. ⚠️

- The score is the binary in its default config — every PEP rule on, nothing configured ([CHKARCH-CONFIGURATION-ONLY]). Never quote a number produced any other way.
- **Precision is the whole game.** A file passes iff the upstream `errors_diff` is empty: an error on EVERY `# E` line, EVERY `# E[tag]` group satisfied, NOTHING on an unmarked line.
- **Every failure is a false positive, not a miss.** The checker already catches every required error; files fail because a strict house rule fires on spec-valid code. Fix by teaching the checker to recognise the valid construct — never by missing a required error or silencing a rule ([CHKARCH-CONFORMANCE-MODE]).
- **Ratchets, always.** Pass-% only up, FP ceiling only down (`coverage-thresholds.json`); benchmark times only down ([CHKARCH-TESTING-BENCH-RATCHET]). Moving a ratchet the wrong way means the change isn't done.
- Basilisk is listed in the [official results](https://github.com/python/typing/blob/main/conformance/results/results.html) at 100%. Dropping below is ⛔️ ILLEGAL.

# Design Principles

One IDE extension = a complete, fast Python workflow. The LSP drives all functionality — extensions only react to LSP signals (commands, state changes) and NEVER register a command the LSP doesn't advertise.

Basilisk has **no modes** — behaviour is per-rule configuration ([CHKARCH-CONFIGURATION-ONLY]). The default enables every PEP typing-spec rule and nothing else; opinionated house-style rules (require-annotation `BSK-0001/0002/0004`, require-`@override` `BSK-0025`, redundant-annotation `BSK-0050`, explicit-`Any` nudge `BSK-0014`) are opt-in. Every diagnostic must teach — explain why, not just what.

# Documentation Honesty — No Unsubstantiated Claims

Trust is the product. Applies **everywhere** — specs, plans, README, website, marketing, code comments.

- **Every empirical or comparative claim about the outside world** (stats, adoption, competitor capability/performance/conformance numbers, market facts, attributed quotes) MUST carry an inline link to the authoritative source that actually makes that claim. Link it or delete it — NEVER invent or approximate. A value that drifts (a competitor's conformance %, a download size) links to its live source, never a frozen figure.
- **Self-measured, reproducible metrics are exempt** (e.g. our conformance score from the unmodified `python/typing` scorer) — but state how they're measured and don't compare them against numbers from a different methodology.


# Documentation Structure

The spec-ID web is the fabric of this repository and is non-negotiable:

- Every spec section has a unique, non-numeric, hierarchical ID (`[GROUP-TOPIC]` / `[GROUP-TOPIC-DETAIL]`).
- Code references its spec ID in comments (e.g. `// Implements [LSP-HOVER]`) so `grep [LSP-` walks spec → code → tests in one shot. Tests cross-reference both the spec ID and the code.
- Find code, tests, or specs that aren't linked? Add the missing ID or reference.
- `docs/INDEX.md` — full index. `docs/specs/[COMPONENT]-[FEATURE]-SPEC.md`, `docs/plans/[COMPONENT]-[FEATURE]-PLAN.md`.
- `docs/specs/LSP-ARCHITECTURE-SPEC.md` is the **single source of truth** for all shared LSP/DAP/config/commands; editor-specific specs point back to it.

# Rules

Build scripts live in the Makefile. [Pyrefly](https://pyrefly.org/en/docs/) and [Pyright](https://microsoft.github.io/pyright/#/) are reference implementations to compare against — NEVER copy from their code.

- **Top priority: reduce duplication.** Run `deslop:find-similar` BEFORE writing new code and `deslop:top-offenders` after changing code. Merge duplicates; keep it DRY.
- Aggressively hoist shared code into shared crates/modules/packages. Use [lspkit](https://crates.io/crates/lspkit) where possible.
- Centralize all global state: one global-state file per app, no state outside it. All mutable state uses Signals — no stale state on screen.
- Keep dependency versions in sync across `.github/workflows/ci.yml` and `.devcontainer/Dockerfile`.
- Define spec models in [typeDiagram markup](https://typediagram.dev/docs/language-reference.html); generate ADTs with the [typeDiagram code generator](https://typediagram.dev/docs/cli.html) pointed at the markup.
- Don't use Git unless asked.
- Treat legacy code as code to be removed — there is no legacy code in this codebase.
- Avoid regex to parse anything, use ruff.
- Keep files under 500 LOC; break up larger files. Move files rather than copying them.
- Use your judgment — do NOT stop to ask the user questions.
- NEVER kill a VS Code process (including in the browser) — it disrupts active debugging and test sessions.
- Bug Fix Process: [fix bug skill](.claude/skills/fix-bug/SKILL.md)

## Git & Branch Discipline

Git is off-limits unless explicitly asked. When git IS used:

- **NEVER push to `main` directly.** Every change ships via PR → CI green → merge.
- **NEVER list the agent as a commit co-author** — no `Co-Authored-By` trailer, no agent attribution.
- **Work on exactly ONE branch.** Reuse the existing feature branch; if multiple exist, merge them into one before any other work.
- **Worktrees are forbidden** — never run `git worktree`.
- **NEVER close anything you did not open** — no issue, PR, discussion, or review thread, however stale. Including auto-close keywords: write `Refs #123`, never `Closes/Fixes #123`.

## Testing

- Target 100% coverage on every measure. Each PR MUST INCREASE overall coverage or it is a failure.
- NEVER delete a failing test, remove a failure-causing assertion, reduce assertiveness, or ignore tests. Broken or missing functionality gets MORE failing tests, never fewer.
- Mutation score only increases; widen scope over time by adding `#[mutation_safe]` tests over more rules/functions. The gate ([CHKARCH-TESTING-MUTATION-RATCHET], baseline `mutation_testing/mutation_scores.json`) fails CI if the viable mutant pool shrinks, caught drops, missed/timeout rise, or kill rate drops.
- `make test` is FAIL-FAST — NEVER use `--no-fail-fast`.
- `make test` always computes and enforces coverage. The threshold lives in `coverage-thresholds.json` at the repo root — not env vars, not GH repo variables, not CI YAML. Ratchet only; below threshold fails the pipeline.
- VSIX tests must not call `whenCommandReady` or `vscode.commands.getCommands(true)` to check existence — the core code does that. Assert through the UI or, worst case, internal VSIX state.

## Benchmarks

Performance is a feature; both the conformance and benchmark ratchets hold simultaneously ([CHKARCH-TESTING-BENCH-RATCHET]). A conformance fix that blows the benchmark gate is NOT done — optimize or restructure it.

- Run `make bench` whenever you touch checker hot paths (resolver visitors, rule `check` loops, new conformance logic). Every run does `cargo clean` + a fresh `--release` build and pulls the latest official release of each competitor (pyright, mypy, ty, pyrefly, zuban) before timing.
- **Write always.** Measured numbers go to `benchmarks/status/<machine>.csv` immediately and unconditionally — after every fixture and again at the end (`benchmarks/summarize.py`). A run that measured a number but didn't record it is a lie.
- **Gate separately.** A zero-tolerance read-only gate compares those numbers against the **committed** baseline (read from git, not the working copy) and fails if basilisk is slower on any fixture. The gate cannot be disabled or widened. New machines establish a baseline only after a successful run is committed.

## Logging Standards

- **Structured logging only** — `tracing` + `tracing-subscriber`, never `println!`/`eprintln!` for diagnostics. If you can't see what's happening, add more logging.
- Log at entry/exit of significant operations (`error|warn|info|debug|trace`), with structured fields not interpolation: `tracing::info!(user_id = 42, action = "checkout")`.
- VS Code extension: detailed logs go to a file in the extension's state folder AND to the Output Channel.
- **NEVER log PII** (names, emails, phone, IPs) or secrets — log `"key: present"` or a truncated hash.

## Rust Quality Standards

- Run clippy and fmt routinely; fix violations promptly. All lints at highest strictness (Cargo.toml `[lints]`). Add lints if in doubt; never remove them.
- `unsafe` is forbidden (`unsafe_code = "deny"`). `unwrap()` is always a violation — use `?` with proper error types. No `panic!`, `todo!`, `unimplemented!` — handle every case and return `Result<T, E>`.
- `Result<T, E>` and `Option<T>` everywhere; early returns with `?`. Expressions over statements (`match`, `if let`, iterator chains). Pattern matching over casting or unwrapping. Pure functions; minimize side effects.
- Small, focused functions (<20 lines) with low cognitive complexity (clippy::cognitive_complexity enabled). Descriptive names (no single letters except in closures). Group related functionality into modules; document public APIs.

# Too Many Cooks — Multi-Agent Coordination

Register before starting work. Coordinator dictates orders through plans and messages and delegates; others follow and check messages regularly. Lock files before editing, never edit locked files, and respond to messages promptly.

# Website

- **Minimize CSS classes**; name them after what the element IS, not what section it's in. Avoid LLM-default colors (e.g. purple) — use RNG and color wheels.

## Per-diagnostic error pages (`/errors/BSK-XXXX/`)

Every diagnostic ends with `see: https://www.basilisk-python.dev/errors/BSK-XXXX` (the `docs_url` on each rule's `ErrorCode`). Pages are generated for all codes from checker source — `[WEBSITE-ERROR-PAGES]` (`docs/specs/WEBSITE-ERROR-PAGES-SPEC.md`). The single source is `website/src/_data/rules.json`:

```bash
python3 scripts/gen_rules_reference.py --data   # writes website/src/_data/rules.json
```

It extracts the `//! BSK-XXXX:` summary + doc-comment body (prose and ```python examples) from each `crates/basilisk-checker/src/rules/*.rs`. **Rerun it after adding or renaming a rule** — CI regenerates and `diff`s `rules.json` (`[WEBSITE-ERROR-PAGES-DRIFT]`), and rule-source edits count as website changes so the guard runs. The same data drives the `/docs/rules/` table and counts. Pages render via `website/src/errors/error.njk`; a worked-example screenshot appears automatically for any code in `screenshots/shots.mjs`.

# Architecture

Strict-by-default Python type checker and comprehensive LSP in **Rust**. Users can flick errors down to warnings and adopt type safety incrementally, or just use the LSP for autofixes, formatting, debugging, and profiling.

- **Parser**: `ruff_python_parser`. **Incremental**: Salsa — sub-10ms incremental checks.
- **Formatting**: `ruff_python_formatter` embedded in-process ([LSPFMT-ENGINE]); import hygiene reimplemented natively on the Ruff AST ([LSPFMT-IMPORTS]). The `ruff` CLI is NOT a runtime dependency — never spawn it.
- **Concurrency**: Tokio in the LSP server (request multiplexing + `spawn_blocking`); analysis is single-threaded on one dedicated large-stack thread ([LSPARCH-ARCH-STACK]).
- **No Pyright/mypy/Node.js** — zero TypeScript or Python runtime.

## Migration to `lspkit`

Cross-cutting LSP scaffolding here is being distilled into the generic `lspkit-*` workspace in [`Nimblesite/lsp_toolkit`](https://github.com/Nimblesite/lsp_toolkit). Prefer `lspkit-*` crates for new LSP infrastructure; when changing existing scaffolding, flag in the PR description if the patch duplicates `lspkit` and reference the upstream crate.

| Current path | Toolkit crate |
|---|---|
| `crates/basilisk-lsp/src/server/mod.rs:96` tower-lsp `Server` setup | `lspkit-server` (hand-rolled JSON-RPC + `Dispatcher` + `Capabilities`; no `tower-lsp` dependency) |
| `crates/basilisk-lsp/src/workspace.rs:39–116` `WorkspaceIndex` + import-graph invalidation | `lspkit-vfs` + consumer-side index |
| `crates/basilisk-lsp/src/server/handlers/{navigation,features}.rs` | `lspkit-server::Dispatcher::register` per method name |
| `crates/basilisk-lsp/src/server/init.rs:224–242` diagnostic publication | `lspkit-server::diagnostics::DiagnosticsBus` |
| `crates/basilisk-lsp/src/server/mod.rs:61,64` debounce + file-watcher loop | `lspkit-live::watcher::FileWatcher` + `lspkit-live::scheduler::spawn` |
| `crates/basilisk-lsp/src/config.rs:35–100` `WorkspaceConfig` loader | `lspkit-config::load_from_ancestor` |
| `crates/basilisk-lsp/tests/lsp/ws_test_common.rs` E2E fixture | not yet in toolkit (v0.1 follow-up) |
