# Contributing to Basilisk

<p align="center"><strong>English</strong> · <a href="CONTRIBUTING.zh.md">简体中文</a></p>

Basilisk is built by a **human + AI partnership**, split on purpose. AI agents do the mechanical, verifiable engineering. Humans do what needs taste, judgment, accountability, and trust.

- [**For Humans**](#for-humans) — judgment, taste, trust, and everything an agent can't be held accountable for. Express it in the specs first.
- [**For AI**](#for-ai) — technical execution under the rules in [`CLAUDE.md`](CLAUDE.md).

> Every TODO in [`docs/plans/ROADMAP-NEXT-STEPS-PLAN.md`](docs/plans/ROADMAP-NEXT-STEPS-PLAN.md) carries the same split:
>
> | Tag | Meaning |
> |---|---|
> | `[AGENT]` | Mechanical, verifiable code/test/docs work an agent drives end-to-end. |
> | `[HUMAN]` | Needs human discretion — accounts, secrets, money, brand voice, strategy, native-speaker judgment. |
> | `[HYBRID]` | Agent drafts and prepares; a human reviews, approves, or supplies credentials. |

---

## For Humans

You don't need to write Rust to make Basilisk better. **The highest-leverage thing a human can do here is verify that the checker actually analyses code** — not that a number went up.

This isn't hypothetical. Checker logic was fitted to the conformance fixtures, the resulting score was published, and we didn't catch it until much later; both published numbers — conformance and performance — are now **withdrawn**. See the [conformance correction](https://www.basilisk-python.dev/docs/conformance/), the [integrity audit](docs/CONFORMANCE-INTEGRITY-AUDIT.md), and the author's [personal account and apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology). None of it was deliberate — nobody set out to game the suite; the instructions named the score as the goal, matching text moves a score faster than analysing code does, and nothing verified the difference. In rough order of impact:

### 1. Verify the metrics yourself

Agents optimise whatever you measure, and every number here is reachable without doing the underlying work: **conformance, coverage, mutation score, assertions, lint, benchmarks.** Re-derive any metric change before you believe it — agents cannot grade their own homework. What to look for:

- **Text-matched logic** — the big one. A rule keyed on raw source text or hard-coded symbol spellings instead of resolved AST symbols scores well and fails on real code. Rename an import (`from typing import Final as F`) or reformat a file: the diagnostics must not change.
- **Silence instead of analysis** — a rule disabled or quietly unregistered so it stops firing, with the loss undisclosed. Deleting a text-matched rule is the opposite and is what we want: it comes with a failing test and a report saying what went. Judge it by whether the hole is visible afterwards.
- **Weakened tests** — failing tests deleted, assertions cut or watered down so "green" means nothing.
- **Scoreboard or gate edits** — a hand-edited `conformance_status.csv`, or a lowered threshold (`coverage-thresholds.json`, the mutation or benchmark baselines).
- **Measuring less** — excluded diagnostic codes, skipped fixtures, narrowed mutation scope. A high percentage over part of the suite is not a percentage. Ask for the denominator every time: the mutation score is 100% over 161 mutants of an ~82k-LOC crate, because scope is opt-in.

Metrics move only the *honest* way — because the work got better, never because someone changed how we count ([CHKARCH-CONFORMANCE]). The one number expected to **fall** is conformance: removing rules that never analysed anything lowers it, and that drop is progress, reported rather than avoided.

### 2. Test it for real — on real, large codebases

Automated tests prove the code does what we told it to. They can't tell you whether it holds up against a million lines somebody else wrote. **Point Basilisk at the real world:**

- **Run it on large production and open-source codebases** — CPython's `Lib/`, Django, pandas, Home Assistant, SymPy, Sentry, *and your own biggest repos*. Fixtures are tidy; real code is not, and that's where false positives, crashes, slow paths, and missed errors surface.
- **Install a published artifact** (not a dev build) on a clean machine, open a real project, and confirm diagnostics, hover, go-to-definition, debugging, and profiling all light up — in **each** editor. UX and platform breakage are found by humans driving the real UI.
- **Get your team using it daily and harvest their feedback.** Turn every "this fired on perfectly good code" or "this missed an obvious bug" into an issue (§6) and a failing test.

### 3. Maintain and improve code quality

Review AI-authored PRs against the bar in [`CLAUDE.md`](CLAUDE.md): *code here should comfortably pass review at a top-tier engineering organization.* Catch over-engineering, premature abstraction, duplicated logic, and the subtly-wrong-but-plausible. An agent will happily ship something that compiles and passes tests but hides a landmine.

### 4. Strengthen tests and the mutation score

Coverage percentage is the floor, not the goal. Judge whether assertions actually *prove* something or just execute lines. Push for stronger assertions, widen the mutation-testing scope ([CHKARCH-TESTING-MUTATION-RATCHET]), and call out tests that would still pass if the code were broken. Both ratchets move one way only.

### 5. Audit performance measurements

Performance is a feature, but the benchmark is **indicative, not a gate** ([CHKARCH-TESTING-BENCH]). It runs on a workstation against whatever else that machine is doing, so background load moves every tool together. Nothing in CI passes or fails on a benchmark number, and no gate is to be reintroduced.

Only a human can do this: run `make bench` on a quiet machine, compare the tools *within* that single run (timed back to back, so machine speed cancels), and dig into anything that looks off. Never compare against a number from a different machine or time. Every run writes to `benchmarks/status/<machine>.csv` immediately — measuring without recording is a lie.

### 6. Report GitHub issues

You're the one running real-world Python through Basilisk. When something is wrong — a false positive, a missed error, a crash, a slow path, a clumsy editor interaction — file a precise, reproducible issue with the smallest snippet that triggers it. A good bug report becomes a failing test, which becomes a fix.

### 7. Check plans and specs against reality

Specs and plans are the fabric of this repo (see [`docs/INDEX.md`](docs/INDEX.md)). Does every section have a non-numeric, hierarchical ID? Does the implementing code reference it? Does the implementation *match* the spec, or has it drifted? `/spec-check` helps, but the judgment — *is this spec still telling the truth?* — is yours.

### 8. Ensure feature parity across IDE extensions

The promise is **one seamless experience in every editor**: VS Code (plus Cursor/Windsurf via Open VSX), Zed, and Neovim. A feature in one extension but not the others is a parity bug. Audit them side by side and file the gaps. The **LSP drives functionality** — extensions only react to what it advertises.

### 9. Security auditing

Threat-model the checker, the LSP, the extensions, the release pipeline, and the dependency tree. Review what `/security-review` and Dependabot surface with a human's sense of *what actually matters*. Single binary, no runtime, no telemetry is a security posture — help keep it true.

### 10. Improve the AI instructions

The highest-**compounding** lever: better instructions produce better output on every future task. Tighten [`CLAUDE.md`](CLAUDE.md), the specs, and the skills under `.claude/`. When an agent goes wrong, the fix usually isn't the code — it's the instruction that allowed it.

### 11. Everything humans are simply best at

Brand voice and naming. Outreach and community. Strategic prioritization — *what should we even build next?* Native-speaker and design judgment. Anything involving accounts, secrets, tokens, or money. If a test can't check it, it's your call.

### How to contribute as a human

1. **Open an issue** for a bug, parity gap, spec drift, or checker inaccuracy. Be specific and reproducible.
2. **Open a PR** for fixes or docs — fill out the [pull request template](.github/pull_request_template.md) honestly. "Tests pass" is not an answer to *how do the tests prove it works?*
3. **Review PRs** — a first-class contribution, often the most valuable one.

---

## For AI

You convert the specs to code and tests and keep all three in sync. The standing rules live in [**`CLAUDE.md`**](CLAUDE.md) and **override default behavior** — read it first and follow it exactly.

**Before you touch anything:** read [`CLAUDE.md`](CLAUDE.md) in full, orient via [`docs/INDEX.md`](docs/INDEX.md) and [`docs/specs/LSP-ARCHITECTURE-SPEC.md`](docs/specs/LSP-ARCHITECTURE-SPEC.md), then register with the **too-many-cooks** coordinator and **lock files** before editing them. Never edit a locked file.

**Accuracy is the prime directive.** Basilisk must be correct on Python it has never seen. Every rule decides from the resolved AST, never from how the source happens to be spelled. When you find a rule keyed on raw text, hard-coded symbol spellings, or a conformance fixture, do exactly three things — **do not fix it, do not rewrite it, do not leave a TODO**:

1. **Write a test that fails** because of that code.
2. **Delete the offending code.**
3. **Tell the user what you deleted and why.**

What gets built back is the user's call, not yours. **A failing test that pins real incorrect behaviour is worth more than a passing fixture carried by logic that does not analyse code** — the first records what Basilisk can't do, the second falsely claims it can.

**The non-negotiables** (full detail in `CLAUDE.md`):

- **Git is off-limits unless explicitly asked.** Never push to `main`, never list an agent as co-author, never use worktrees, work on exactly one branch.
- **Spec IDs are the fabric.** Every spec section has a non-numeric, hierarchical ID; code references it (`// Implements [LSP-…]`); tests cross-reference both. Missing link → fix it.
- **DRY, ruthlessly.** `deslop` MCP: `find-similar` before writing, `top-offenders` after. Search for existing code before adding new.
- **Ratchets move one way** — coverage and mutation score up, false positives down. Conformance is a regression detector, not a target; benchmark times gate nothing ([CHKARCH-TESTING-BENCH]).
- **Never touch the scoreboard.** Conformance runs the binary with **every rule enabled**: no config file, no per-rule override, no skipped fixtures, no deleting source to dodge a failure, no removing rules from `all_rules()`. Equally forbidden: hand-editing `conformance/conformance_status.csv` or loosening `coverage-thresholds.json`. Never publish or quote a conformance figure ([CHKARCH-CONFORMANCE]).
- **`make` is the interface.** `make build | test | lint | fmt | clean | ci | setup` — exactly seven targets, don't add more. `make test` is fail-fast and enforces the coverage threshold.
- **Rust quality bar:** no `unwrap`, `panic!`, `todo!`, `unimplemented!`, `unsafe`, or `allow(clippy::…)`. `Result`/`Option` everywhere, small pure functions, files under 500 LOC.
- **No CI artifacts.** Storage is billed even on this public repo — see [GITHUB-NO-ARTIFACTS].

**How you work:**

- **Test-driven, always.** Failing test → confirm it fails *for the right reason* → fix the code (never the test) → confirm it passes. Coarse e2e tests only. Never delete a failing test or weaken an assertion.
- **Use judgment; don't stop to ask.** (Reporting a deletion isn't a question — report it and continue.)
- **Pick up `[AGENT]` work** from [`docs/plans/ROADMAP-NEXT-STEPS-PLAN.md`](docs/plans/ROADMAP-NEXT-STEPS-PLAN.md). Leave `[HUMAN]` work for a human. Draft the agent half of `[HYBRID]` items and hand them off.
- **Defer to the human signals** here. A human's issue report becomes your failing test.
