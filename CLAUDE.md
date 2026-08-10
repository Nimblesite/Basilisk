# CLAUDE.md

## Where Basilisk stands

Basilisk's type checker was producing incorrect results. We asked for it to be removed from the `python/typing` results, and it was ([python/typing#2330](https://github.com/python/typing/pull/2330)). The code responsible is not isolated, and we cannot say how many rules are affected.

A code-quality tool that does not produce correct results is worse than useless.

**So: unlist first, no rebuilding until after the public delisting is complete.** Basilisk is being unlisted everywhere and then rebuilt from the ground up as a new product — not fixed, not audited, not salvaged. It will ship only what can be trusted, which most likely will not include type checking. If type checking ever returns, it goes through external auditing before release.

Full messaging, including every approved word of public copy: [`docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md`](docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md). It is the single source of truth. Copy from it; never write your own version.

## The current job

1. **Unlist** every channel — Marketplace, Open VSX, Zed, PyPI, Homebrew, Scoop.
2. **Strip** every public surface to the messaging: READMEs, website, listings.
3. **Make the CLI inert** ([WITHDRAWAL-INERT](docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-INERT)).

Nothing else is in scope until that is done.

While the eventual plan is to rebuild a Python dev experience tool from the ground up again, you are not currently allowed to contribute to that end.

## Do not

- **Do not fix, improve, audit, or extend the type checker.** Not a rule, not a diagnostic, not a false positive. That code is finished. Deleting is fine; repairing is not.
- **Do not touch conformance.** Don't run it, quote it, restore it, or resubmit. Never publish a conformance or benchmark figure, in any tense.
- **Do not extract "the good parts" yet.** The code is too contaminated to separate; an extraction now carries the problem into the new product.
- **Do not market anything** — no feature lists, rule counts, or per-rule docs, including for parts that never touched the checker.
- **Do not reassure about scope.** Never "only a few rules", "the language server is unaffected, keep using it". We cannot scope it; saying so is the point.
- **Do not quote the apology.** Link it, neutrally, and nothing more.
- **Do not blame anyone outside the project. Do not give a timeline.**

## Still standing

- **Honesty is the product.** Every external claim carries an inline link to its source. Self-measured numbers state their method or don't exist. Screenshots are real captures or absent.
- Internal specs, plans, and [`docs/CONFORMANCE-INTEGRITY-AUDIT.md`](docs/CONFORMANCE-INTEGRITY-AUDIT.md) are the record of what went wrong. Keep them; they are not marketing surfaces.
- Spec IDs stay: `[GROUP-TOPIC-DETAIL]`, cited from code, indexed in [`docs/INDEX.md`](docs/INDEX.md).
- Rust: no `unsafe`, no `unwrap()`, no `panic!`/`todo!`. `Result`/`Option`, early `?`. Clippy and fmt clean. Files under 500 LOC, functions under 20 lines.
- Structured logging via `tracing` only — never `println!`/`eprintln!`, never PII. (The inert CLI notice is the one deliberate direct write to stderr.)
- Never delete a failing test, weaken an assertion, or skip a test to go green.
- Don't use git unless asked. Never push to `main`; never list an agent as co-author; no worktrees; one branch.
- Use your judgment — don't stop to ask questions.
- Never kill a VS Code process.
