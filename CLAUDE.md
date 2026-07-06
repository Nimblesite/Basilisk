<!-- agent-pmo:f87d349 -->
# CLAUDE.md

Code here must comfortably pass review at a top-tier engineering org. Keep quality high and fix shortcomings as you find them.

⚠️ The conformance test suite is the **single source of all authority**: https://github.com/python/typing/tree/main/conformance/tests. Conformance is measured ONLY by how accurately Basilisk passes these tests — nothing else. ⚠️

⚠️ Disabling, deleting, or unregistering ANY conformance rule is FORBIDDEN. Move the number by FIXING the checker, NEVER by touching the scoreboard: no `basilisk.json`, no deleting rule source (`crates/basilisk-checker/src/rules/*.rs`), no removing rules from `all_rules()`, no hand-editing `conformance/conformance_status.csv`, no loosening `coverage-thresholds.json` (`threshold` / `max_false_positives`). `score.py` purges stale config before scoring — deleting a rule to dodge that purge is the SAME crime by another route. See [CHKARCH-CONFORMANCE], [CHKARCH-CONFORMANCE-MODE]. ⚠️

## Conformance Is the Prime Directive

Target: **100% PEP conformance**, canonical Python **3.12**. Read the [PEP conformance README](https://github.com/python/typing/blob/main/conformance/README.md) carefully. This discipline outranks every other concern in this file.

- **One reproducible scorer.** `python3 conformance/score.py` runs the real, sha-pinned `python/typing` calculator over the unmodified binary in its default config — every PEP rule on, nothing configured ([CHKARCH-CONFORMANCE], [CHKARCH-CONFIGURATION-ONLY]). The score is exactly what a user gets out of the box; never quote a number produced any other way.
- **Precision is the whole game.** A file passes iff the upstream `errors_diff` is empty: emit an error on EVERY `# E` line, satisfy EVERY `# E[tag]` group, and emit NOTHING on a line the suite does not mark. Follow each PEP exactly — no missed required error, no stray diagnostic.
- **Every failure is a false positive, not a miss.** The checker already catches every required error; files fail because a strict house-rule fires on spec-valid code. Close the gap by making the checker PRECISE — teach it to recognise the valid construct — never by missing a required error or silencing a rule ([CHKARCH-CONFORMANCE-MODE]).
- **Ratchets, always.** Pass-% only goes UP and the false-positive ceiling only goes DOWN (`coverage-thresholds.json`: `conformance.threshold`, `conformance.max_false_positives`); benchmark times only go DOWN ([CHKARCH-TESTING-BENCH-RATCHET]). A change that moves any ratchet the wrong way is not done.

## Design Principles

We are building a better Python developer experience: one IDE extension for a complete, fast workflow. The LSP drives all functionality — IDE extensions only react to LSP signals (commands, state changes) and NEVER register a command the LSP doesn't advertise.

Basilisk has **no modes** — behaviour is per-rule configuration ([CHKARCH-CONFIGURATION-ONLY]). The default enables every PEP typing-spec rule and nothing else; opinionated house-style rules (require-annotation `BSK-E0001/E0002/E0004`, require-`@override` `BSK-E0025`, redundant-annotation `BSK-W0050`, explicit-`Any` nudge `BSK-W0014`) are opt-in. Every diagnostic must teach — explain why, not just what.

# Documentation Structure

The spec-ID web is the fabric of this repository and is non-negotiable:

- Every spec section has a unique, non-numeric, hierarchically structured ID (`[GROUP-TOPIC]` / `[GROUP-TOPIC-DETAIL]`).
- All code references its spec ID in comments (e.g. `// Implements [LSP-HOVER]`) so `grep [LSP-` walks spec → code → tests in one shot.
- All tests cross-reference both the spec ID and the code.
- Find code, tests, or specs that aren't linked? Fix it — add the missing ID or reference.

- `docs/INDEX.md` — full index of all docs
- `docs/specs/` — specifications (naming: `[COMPONENT]-[FEATURE]-SPEC.md`)
- `docs/plans/` — implementation plans (naming: `[COMPONENT]-[FEATURE]-PLAN.md`)

`docs/specs/LSP-ARCHITECTURE-SPEC.md` is the **single source of truth** for all shared LSP/DAP/config/commands. Editor-specific specs point back to it.

# Reference

- [Python type system spec](https://typing.python.org/en/latest/spec/index.html)
- [Pyrefly](https://pyrefly.org/en/docs/) | [Pyright](https://microsoft.github.io/pyright/#/) — reference implementations to compare against; NEVER copy from their code.
- [Conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html) — being listed here is the goal 

Refer to the Makefile for build scripts

# Rules

- **Top priority: reduce duplication.** Run `deslop:find-similar` BEFORE writing new code and `deslop:top-offenders` after changing code. Always merge duplicates and keep it DRY.
- Aggressively hoist shared code into shared crates/modules/packages.
- Centralize all global state: each app has a single global-state file, and NO state lives outside it. All mutable state uses Signals for reactivity — no stale state on screen.
- Keep dependencies and versions in sync across `.github/workflows/ci.yml` and `.devcontainer/Dockerfile` at all times.
- Use [typeDiagram markup](https://typediagram.dev/docs/language-reference.html) to define models in the specs. Generate the ADTs using the [typeDiagram code generator](https://typediagram.dev/docs/cli.html) pointing at the markup.
- Don't use Git unless asked.
- Treat legacy code as code to be removed — there is no legacy code in this codebase.
- Avoid regex to parse anything, use ruff.
- Keep files under 500 LOC; break up larger files. Move files rather than copying them.
- Use your judgment — do NOT stop to ask the user questions.
- NEVER kill a VS Code process (including in the browser) — it disrupts active debugging and test sessions.
- Bug Fix Process: [fix bug skill](.claude/skills/fix-bug/SKILL.md)

## Documentation Honesty — No Unsubstantiated Claims

Trust is the product; a fabricated or contradictory figure destroys it. This applies **everywhere** — specs, plans, README, website, marketing, and code comments.

- **Every empirical or comparative claim about the outside world** (stats, adoption, competitor capability/performance/conformance numbers, market facts, attributed quotes) MUST carry an inline link to the authoritative source that actually makes that claim. Link the URL or delete the claim — NEVER invent or approximate one. A value that drifts (a competitor's pinned conformance %, a download size) links to its live source, never a frozen figure.

- **Self-measured, reproducible metrics are exempt** (e.g. our own conformance score from the unmodified `python/typing` scorer in CI) — but state how they're measured and don't compare them against numbers from a different methodology.

## Git & Branch Discipline

Git is off-limits unless you are explicitly asked. When git IS used:

- **NEVER push to `main` directly.** Every change ships via PR → CI green → merge. No exceptions.
- **NEVER list the agent as a commit co-author** — no `Co-Authored-By` trailer, no agent attribution.
- **Work on exactly ONE branch.** Reuse the existing feature branch; never open a second. If multiple feature branches exist, merge them into one immediately before any other work.
- **Worktrees are forbidden** — never run `git worktree`.

## Testing

- NEVER delete a failing test, remove a failure-causing assertion, reduce assertiveness, or ignore tests. Broken or missing functionality gets MORE failing tests, never fewer.
- Mutation score only increases. Widen scope over time by adding `#[mutation_safe]` tests over more rules/functions. The gate ([CHKARCH-TESTING-MUTATION-RATCHET], baseline `mutation_testing/mutation_scores.json`) fails CI if the viable mutant pool shrinks, caught drops, missed/timeout rise, or kill rate drops.
- `make test` is FAIL-FAST — it stops at the first failure. NEVER use `--no-fail-fast`; it saves CI minutes.
- `make test` always computes and enforces coverage. The threshold lives in `coverage-thresholds.json` at the repo root — not env vars, not GH repo variables, not CI YAML. Below threshold fails the pipeline. Ratchet only.

### IDE Extension Testing

VSIX tests must not call `whenCommandReady` or `vscode.commands.getCommands(true)` to check for existence. The core code does that; tests assert the command exists through the UI or, worst case, internal VSIX state.

## Benchmarks

Performance is a feature: conformance must never be traded for it, nor it for conformance. Both ratchets hold simultaneously ([CHKARCH-TESTING-BENCH-RATCHET]).

- Run `make bench` whenever you touch checker hot paths (resolver visitors, rule `check` loops, new conformance logic). It fails if basilisk gets >25% slower on any fixture vs the committed baseline `benchmarks/status/<machine>.csv`.
- A conformance fix that blows the benchmark gate is NOT done — optimize or restructure it.
- `BENCH_NO_GATE=1` baseline resets are for fixture-set changes only and must be justified in the PR description.

## Logging Standards

- **Structured logging only.** NEVER `println!`/`eprintln!` for diagnostics — use `tracing` + `tracing-subscriber`. If you can't see what's happening, add more logging.
- **Log at entry/exit of significant operations.** Levels: `error|warn|info|debug|trace`.
- **Structured fields, not string interpolation** — `tracing::info!(user_id = 42, action = "checkout")`, never format strings.
- **VS Code extension:** detailed logs go to a file in the extension's state folder AND to the VS Code Output Channel.
- **NEVER log PII** (names, emails, phone, IPs) or secrets. Log `"key: present"` or a truncated hash, never the value.

## Rust Quality Standards

- Run clippy and fmt routinely; fix violations promptly. All lints at highest strictness (see Cargo.toml `[lints]`). Add lints if in doubt; never remove them.
- `unsafe` code is forbidden (`unsafe_code = "deny"`).
- `unwrap()` is always a violation — use `?` with proper error types.
- No `panic!`, `todo!`, `unimplemented!` — handle every case and return `Result<T, E>`.

## Functional Programming Style

- `Result<T, E>` and `Option<T>` everywhere; early returns with `?` for clean propagation.
- Expressions over statements — `match`, `if let`, iterator chains.
- Pattern matching over casting or unwrapping. Pure functions; minimize side effects.

## Code Structure

- Small, focused functions (<20 lines) with low cognitive complexity (clippy::cognitive_complexity enabled).
- Descriptive variable names (no single letters except in closures).
- Group related functionality into modules. Public APIs must have documentation.

# Too Many Cooks — Multi-Agent Coordination

Register before starting work.

- Coordinator: dictate orders through plans and messages, and delegate.
- Others: follow the coordinator's direction and check messages regularly.
- Lock files before editing; don't edit locked files.
- Respond to messages promptly — others may be waiting.

# Website

## CSS

- **Minimize CSS classes** — consolidate where possible.
- Name classes after what the element IS, not what section it's in.
- Avoid common LLM-default colors (e.g. purple) — use RNG and color wheels.

## Per-diagnostic error pages (`/errors/BSK-XXXX/`)

Every diagnostic the CLI prints ends with `see: https://www.basilisk-python.dev/errors/BSK-XXXX` (the `docs_url` on each rule's `ErrorCode`). Those pages are **generated for all codes** from the checker source — see `[WEBSITE-ERROR-PAGES]` (`docs/specs/WEBSITE-ERROR-PAGES-SPEC.md`). The single source is `website/src/_data/rules.json`, produced by:

```bash
python3 scripts/gen_rules_reference.py --data   # writes website/src/_data/rules.json
```

It extracts the `//! BSK-XXXX:` summary + doc-comment body (prose and ```python examples) from each `crates/basilisk-checker/src/rules/*.rs`. **After adding or renaming a rule, rerun it** — CI fails otherwise: the website job regenerates and `diff`s `rules.json` (`[WEBSITE-ERROR-PAGES-DRIFT]`), and rule-source edits are classified as website changes so the guard runs. The same data drives the `/docs/rules/` table and counts (no hand-maintained code lists). Pages render via `website/src/errors/error.njk`; a worked-example screenshot appears automatically for any code present in `screenshots/shots.mjs`.


# Architecture

Strict-by-default Python type checker and comprehensive LSP built in **Rust**. One IDE extension = complete Python dev experience. Users can flick errors down to warnings and incrementally adopt type safety, or just use the LSP for autofixes, formatting, debugging, and profiling.

- **Parser**: `ruff_python_parser` (MIT, same as Ruff)
- **Incremental**: Salsa framework — sub-10ms incremental checks
- **Formatting**: `ruff_python_formatter` crate embedded in-process ([LSPFMT-ENGINE]); import hygiene reimplemented natively on the Ruff AST ([LSPFMT-IMPORTS]). The `ruff` CLI is NOT a runtime dependency — never spawn it.
- **Parallelism**: Rayon (work-stealing, file-level)
- **No Pyright/mypy/Node.js** — zero TypeScript or Python runtime

Diagnostic codes: `BSK-E####` / `BSK-W####`. See `docs/specs/CHECKER-ARCHITECTURE-SPEC.md` ([CHKARCH]) for full architecture, diagnostic ranges, and conformance scoring.

## Migration to `lspkit`

The cross-cutting LSP scaffolding in this repo (tower-lsp setup, workspace index, file watcher + debouncer, diagnostics publication, capability builder, config loader) is being distilled into the generic `lspkit-*` workspace, maintained in the private repository [`Nimblesite/lsp_toolkit`](https://github.com/Nimblesite/lsp_toolkit).

- **New LSP infrastructure work:** prefer `lspkit-*` crates over reinventing it here.
- **Changes to existing scaffolding here:** flag in the PR description if the patch duplicates `lspkit` functionality, and reference the upstream crate.

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
