<!-- agent-pmo:f87d349 -->
# CLAUDE.md

Code here must comfortably pass review at a top-tier engineering org. Fix shortcomings as you find them.

# Accuracy Is the Prime Directive

Basilisk must be correct on Python it has never seen. Every rule decides from the resolved AST — bindings, types, symbol identity — never from how the source happens to be spelled. `from typing import Final as F` behaves identically to `typing.Final`; reformatting a file changes no diagnostic.

Basilisk was **removed from the python/typing conformance results** on 2026-08-05, at its own author's request — [python/typing#2330](https://github.com/python/typing/pull/2330), reverting [#2316](https://github.com/python/typing/pull/2316). The reason: *"Many of Basilisk's rules match against raw source text and hard-coded typing symbol names instead of resolved symbols on the AST."* Semantics-preserving edits to the suite — renaming imports, adjusting whitespace — broke **113 of 141 test files**. The score was real; the checker under it was not.

Conformance is a downstream sample of that accuracy, and one this codebase is overfitted to. A change that lifts the conformance number without improving analysis of arbitrary Python is a regression.

## Text-matched logic — test, delete, report

A rule whose behaviour depends on the *spelling* of its input rather than the *meaning* is broken, whatever it scores. Finding and removing it outranks every other task here. Find it by:

- Raw source-text matching — `.contains` / `starts_with` / `ends_with` on user code (131 of 145 rule files today).
- Hard-coded symbol spellings instead of resolved identity: `t == "typing.Final"`, `text.starts_with("Callable[")`, `import.module == "typing"`.
- Any regex over Python source.
- Logic keyed to a test fixture: rule files named after conformance tests (`generics_base_class_2.rs`, `constructors_call_init`), branches for shapes only the suite contains, comments citing a test file as justification.
- Detection that fires on formatting — line breaks, spacing, quote style, comment text, statement order.

On encountering it, do exactly three things — **do not fix it, do not rewrite it, do not leave a TODO**:

1. **Write a test that fails** because of the incorrect code — pin the real defect: an aliased import, a reformatted source, a shape the conformance suite never contains.
2. **Delete the offending code.**
3. **Tell the user what you deleted and why**, and that the test is now failing.

Replacing it is not your call. The point is to surface every one of these so the user can acknowledge it and decide what gets built back. A checker with fewer rules and visible failing tests is the correct outcome; a diagnostic that only fires on one spelling looks like coverage and isn't.

**A failing test that pins real incorrect behaviour is worth more than a passing fixture carried by logic that does not analyse code.** The first is an accurate map of what Basilisk cannot yet do; the second is a false claim that it can. Given the choice, take the failing test — every time.

## What a correct rule looks like

The yardstick for judging code — not licence to go and fix it:

- Decides on the **resolved semantic model** from `basilisk-resolver`, never tokens or text. Parses with `ruff_python_parser`.
- Named for the **typing-spec concept** it implements, not a test file.
- Survives **semantics-preserving mutation**: aliased imports, reformatting, reordering → identical diagnostics. Rules without that coverage are unverified.
- Tested against Python the conformance suite has never contained.

## Direction of travel

Background, not a directive: strip text-matched logic, establish which rules genuinely analyse code, and rebuild around those — deliberately, with the user, not as a side effect of some other task. Anything that can't be made to work on the AST gets removed rather than propped up; a smaller trustworthy checker beats a large unreliable one. Analysis Basilisk can't do reliably may be delegated to an external engine. Deletion is a legitimate outcome.

## Conformance's role

`python3 conformance/run_conformance.py` stays honest: fresh `git clone` from `python/typing@main`, clean `cargo build --release` from THIS checkout, the suite's own unmodified `src/main.py --only-run basilisk` via `BASILISK_BIN`. A vendored scorer, injected adapter, cached fixtures, or committed results standing in for a live run is a **BUILD FAILURE**. The number is a regression detector, never an objective:

- **Never publish, quote, or market a conformance figure** — nothing may imply Basilisk is in the official results.
- **Never re-submit to python/typing** until the mutation harness passes clean and an external audit has run.
- Never move the number by touching the scoreboard: rule-suppressing config, deleting source to dodge a failure, hand-editing `conformance/conformance_status.csv`, loosening `coverage-thresholds.json` ([CHKARCH-CONFORMANCE]).
- **A drop caused by removing text-matched logic is progress.** Record it and say so plainly — never restore the code or fake a pass to hold a ratchet. The boundary is intent: deleting a rule to reach a number hides the loss; deleting text-matched logic leaves a failing test behind and reports the drop.
- `coverage-thresholds.json` still gates the pass percentage at 100 with zero false positives, so the first honest deletion fails `make test`. That floor is the incentive that caused the fitting; removing it is the user's call. Until they decide: **delete anyway, report the drop and the failing gate, and stop there.**

# Design Principles

One IDE extension = a complete, fast Python workflow. The LSP drives all functionality — extensions only react to LSP signals and NEVER register a command the LSP doesn't advertise.

**No modes** — behaviour is per-rule configuration ([CHKARCH-CONFIGURATION-ONLY]). Default: every PEP typing-spec rule and nothing else; house-style rules (`BSK-0001/0002/0004` require-annotation, `BSK-0025` require-`@override`, `BSK-0050` redundant-annotation, `BSK-0014` explicit-`Any`) are opt-in. Every diagnostic teaches — why, not just what.

# Documentation Honesty

Trust is the product. Applies everywhere — specs, plans, README, website, marketing, code comments.

- **Every claim about the outside world** (stats, adoption, competitor numbers, market facts, quotes) carries an inline link to the source making it. Link it or delete it. Drifting values link live, never frozen.
- **Self-measured metrics** state how they're measured, are reproducible, and are never compared across methodologies. Conformance isn't publishable at all (above).
- **Book screenshots are release evidence** — captured from the book's pinned released build; never mocked, redrawn, generated, or hand-composed, not even labelled "diagram". Crop and resize freely; never repaint product pixels. No real capture → omit it. See [`book/VISUAL-DESIGN-SYSTEM.md`](book/VISUAL-DESIGN-SYSTEM.md#screenshot-contract).

# Documentation Structure

The spec-ID web is non-negotiable:

- Every spec section has a unique, non-numeric, hierarchical ID (`[GROUP-TOPIC-DETAIL]`).
- Code cites its spec ID in comments (`// Implements [LSP-HOVER]`) so `grep [LSP-` walks spec → code → tests; tests cross-reference both. Anything unlinked gets the missing ID.
- `docs/INDEX.md` indexes `docs/specs/[COMPONENT]-[FEATURE]-SPEC.md` and `docs/plans/[COMPONENT]-[FEATURE]-PLAN.md`. `docs/specs/LSP-ARCHITECTURE-SPEC.md` is the **single source of truth** for shared LSP/DAP/config/commands.

# Rules

Build scripts live in the Makefile. [Pyrefly](https://pyrefly.org/en/docs/) and [Pyright](https://microsoft.github.io/pyright/#/) are references to compare against — NEVER copy their code.

- **Never parse with strings or regex** — `ruff_python_parser` and the resolver only.
- **After correctness, reduce duplication.** `deslop:find-similar` before writing new code, `deslop:top-offenders` after. Merge duplicates.
- Hoist shared code into shared crates/modules. Use [lspkit](https://crates.io/crates/lspkit) where possible.
- One global-state file per app. All mutable state uses Signals — no stale state on screen.
- Keep dependency versions in sync across `.github/workflows/ci.yml` and `.devcontainer/Dockerfile`.
- Define spec models in [typeDiagram markup](https://typediagram.dev/docs/language-reference.html); generate ADTs with its [code generator](https://typediagram.dev/docs/cli.html).
- Don't use Git unless asked.
- Legacy code is code to be removed; there is none here.
- Files under 500 LOC. Move files rather than copying.
- Use your judgment — do NOT stop to ask questions. (Reporting a deletion isn't a question; report and continue.)
- NEVER kill a VS Code process — it disrupts active debugging and test sessions.
- Bug Fix Process: [fix bug skill](.claude/skills/fix-bug/SKILL.md)

## Git & Branch Discipline

Off-limits unless explicitly asked. When git IS used:

- **NEVER push to `main`** — every change ships via PR → CI green → merge.
- **NEVER list the agent as co-author** — no `Co-Authored-By`, no agent attribution.
- **Exactly ONE branch.** Reuse the feature branch; merge multiples into one first.
- **Worktrees are forbidden.**
- **NEVER close anything you did not open** — write `Refs #123`, never `Closes/Fixes #123`.

## Testing

Tests must **enforce behaviour**, not work around the gaps in it. Judge a test by what it would catch, never by whether it's green.

- Tests exercise **meaning, not spelling**: every rule test gets an aliased-import and a reformatted variant, with identical diagnostics. The harness that would enforce this across the suite ([CHKARCH-TESTING-SEMANTIC-MUTATION]) **does not exist yet** — until it does, every rule is unverified and must be described that way.
- Test against Python the conformance suite has never contained. A test copied from `conformance/tests/` cannot detect a rule fitted to `conformance/tests/`.
- NEVER delete a failing test, remove a failure-causing assertion, reduce assertiveness, or ignore tests. Broken functionality gets MORE failing tests, never fewer.
- Target 100% coverage on every measure. Each PR MUST increase overall coverage. Line coverage proves execution, never assertion — a rule at 100% coverage and zero real assertions is the normal failure, not an edge case.
- Mutation score only increases; widen scope with `#[mutation_safe]` tests. The gate ([CHKARCH-TESTING-MUTATION-RATCHET], `mutation_testing/mutation_scores.json`) fails CI if the mutant pool shrinks, caught drops, missed/timeout rise, or kill rate drops. **Read the denominator before the rate:** scope is opt-in, so the committed 100% covers 161 mutants out of an ~82k-LOC crate; timeouts are credited as kills; survivors are aggregated into a count. Never narrow scope to protect a rate, and never kill a mutant by asserting on incidental output instead of the behaviour it changed.
- `make test` is FAIL-FAST — never `--no-fail-fast`. It enforces coverage from `coverage-thresholds.json` at the repo root, not env vars or CI YAML. Ratchet only.
- VSIX tests must not call `whenCommandReady` or `getCommands(true)` to check existence — assert through the UI, or worst case internal VSIX state.

## Benchmarks

The benchmark is **indicative, not a gate** ([CHKARCH-TESTING-BENCH]) — it runs on a workstation against whatever else that machine is doing, shifting absolute times by tens of percent between identical runs. **Nothing in CI passes or fails on a benchmark number, and no gate is to be reintroduced.**

- Run `make bench` when touching checker hot paths. Each run does `cargo clean` + a fresh `--release` build and pulls the latest release of each competitor (pyright, mypy, ty, pyrefly, zuban).
- **Write always.** Numbers go to `benchmarks/status/<machine>.csv` after every fixture and at the end (`benchmarks/summarize.py`). Measuring without recording is a lie.
- **Read correctly.** Compare tools *within* one run, never across machines or times. See `website/src/docs/benchmarks.njk`.

## Logging

- **Structured only** — `tracing` + `tracing-subscriber`, never `println!`/`eprintln!`. Can't see what's happening? Add more logging.
- Log entry/exit of significant operations with structured fields: `tracing::info!(user_id = 42, action = "checkout")`.
- VS Code extension: detailed logs to a file in the extension's state folder AND the Output Channel.
- **NEVER log PII** or secrets — log `"key: present"` or a truncated hash.

## Rust Quality

- Clippy and fmt routinely. All lints at highest strictness (Cargo.toml `[lints]`). Add lints if in doubt; never remove them.
- `unsafe` is forbidden (`unsafe_code = "deny"`). `unwrap()` is always a violation — use `?` with proper error types. No `panic!`, `todo!`, `unimplemented!`.
- `Result<T, E>` / `Option<T>` everywhere; early returns with `?`. Expressions over statements. Pattern matching over casting. Pure functions.
- Functions <20 lines, low cognitive complexity. Descriptive names (no single letters outside closures). Group into modules; document public APIs.

# Too Many Cooks — Multi-Agent Coordination

Register before starting work. The coordinator dictates orders through plans and messages; others follow and check messages regularly. Lock files before editing, never edit locked files, respond promptly.

# Website

**Minimize CSS classes**; name them after what the element IS, not what section it's in. Avoid LLM-default colors (e.g. purple) — use RNG and color wheels.

## Per-diagnostic error pages (`/errors/BSK-XXXX/`)

Every diagnostic ends with `see: https://www.basilisk-python.dev/errors/BSK-XXXX` (each rule's `ErrorCode.docs_url`). Pages generate from checker source — `[WEBSITE-ERROR-PAGES]`. Single source is `website/src/_data/rules.json`:

```bash
python3 scripts/gen_rules_reference.py --data
```

It extracts the `//! BSK-XXXX:` summary + doc-comment body from each `crates/basilisk-checker/src/rules/*.rs`. **Rerun after adding or renaming a rule** — CI regenerates and `diff`s it (`[WEBSITE-ERROR-PAGES-DRIFT]`). The same data drives `/docs/rules/`. Pages render via `website/src/errors/error.njk`; screenshots appear for any code in `screenshots/shots.mjs`.

# Architecture

Strict-by-default Python type checker and comprehensive LSP in **Rust**. Users can flick errors down to warnings and adopt type safety incrementally, or use the LSP alone for autofixes, formatting, debugging, and profiling.

- **Parser**: `ruff_python_parser`. **Incremental**: Salsa — sub-10ms incremental checks.
- **Formatting**: `ruff_python_formatter` in-process ([LSPFMT-ENGINE]); import hygiene native on the Ruff AST ([LSPFMT-IMPORTS]). The `ruff` CLI is NOT a runtime dependency — never spawn it.
- **Concurrency**: Tokio in the LSP server; analysis single-threaded on one dedicated large-stack thread ([LSPARCH-ARCH-STACK]).
- **No Pyright/mypy/Node.js** — zero TypeScript or Python runtime.

## Migration to `lspkit`

LSP scaffolding is being distilled into the `lspkit-*` workspace in [`Nimblesite/lsp_toolkit`](https://github.com/Nimblesite/lsp_toolkit). Prefer `lspkit-*` for new infrastructure; flag in the PR if a patch duplicates it.

| Current path | Toolkit crate |
|---|---|
| `crates/basilisk-lsp/src/server/mod.rs:96` tower-lsp `Server` | `lspkit-server` |
| `crates/basilisk-lsp/src/workspace.rs:39–116` `WorkspaceIndex` | `lspkit-vfs` + consumer-side index |
| `crates/basilisk-lsp/src/server/handlers/{navigation,features}.rs` | `lspkit-server::Dispatcher::register` |
| `crates/basilisk-lsp/src/server/init.rs:224–242` diagnostics | `lspkit-server::diagnostics::DiagnosticsBus` |
| `crates/basilisk-lsp/src/server/mod.rs:61,64` debounce + watcher | `lspkit-live::watcher` + `lspkit-live::scheduler` |
| `crates/basilisk-lsp/src/config.rs:35–100` `WorkspaceConfig` | `lspkit-config::load_from_ancestor` |
| `crates/basilisk-lsp/tests/lsp/ws_test_common.rs` E2E fixture | not yet in toolkit |
