# Contributing to Basilisk

<p align="center"><strong>English</strong> · <a href="CONTRIBUTING.zh.md">简体中文</a></p>

Basilisk is built by a **human + AI partnership**, and the work is split on purpose.
AI agents do the bulk of the mechanical, verifiable engineering. Humans do the things
that need taste, judgment, accountability, and trust — the things AI can't (yet) own.

This guide has two sections. Pick the one that's you.

- [**For Humans**](#for-humans) — judgment, taste, trust, and everything an agent can't be held accountable for.
- [**For AI**](#for-ai) — the technical execution, under a strict set of standing rules.

> The same split runs through the roadmap. Every TODO in
> [`docs/plans/ROADMAP-NEXT-STEPS-PLAN.md`](docs/plans/ROADMAP-NEXT-STEPS-PLAN.md) is tagged:
>
> | Tag | Meaning |
> |---|---|
> | `[AGENT]` | Mechanical, verifiable code/test/docs work an agent drives end-to-end. |
> | `[HUMAN]` | Needs human discretion — accounts, secrets, money, brand voice, strategy, native-speaker judgment. |
> | `[HYBRID]` | Agent drafts and prepares; a human reviews, approves, or supplies credentials. |

---

## For Humans

You don't need to write Rust to make Basilisk better. Basilisk's north stars are public and
non-negotiable: be the **most conformant** *and* the **fastest** Python type checker — and never
trade one for the other ([CHKARCH-TESTING-BENCH-RATCHET]). Both are *measured numbers*, so the
highest-leverage human contributions are the ones that keep those numbers — and everything around
them — **honest**: the calls an agent shouldn't be trusted to sign off on alone. In rough order of
impact:

### 1. Test it for real — on real, large codebases

Automated tests prove the code does what we told it to. They can't tell you whether the product
*feels* right, holds up against a million lines somebody else wrote, or breaks on a machine we never
tried. **Point Basilisk at the real world:**

- **Run it on large, real production and open-source codebases** — CPython's `Lib/`, Django, pandas,
  Home Assistant, SymPy, Sentry, *and your own company's biggest repos*. Fixtures are tidy; real code
  is not, and that's exactly where false positives, crashes, slow paths, and missed errors surface.
  (This doubles as scale/perf evidence — see §5.)
- **Install a published artifact** (not a dev build) on a clean machine, open a real Python project,
  and confirm diagnostics, hover, go-to-definition, debugging, and profiling all light up — in
  **each** editor. UX rough edges and platform-specific breakage are found by humans driving the real
  UI, not by CI.
- **Get your team using it every day and harvest their feedback.** Dogfooding is the highest-signal
  test there is: put Basilisk in front of real Python developers, watch where they hit friction, and
  turn every "this fired on perfectly good code" or "this missed an obvious bug" into an issue (§6)
  and a failing test. The goal is real-world adoption, not green fixtures.

### 2. Audit conformance-score accuracy

**Make sure the way we score conformance is as close to the official
PEP scoring as possible, and reuses the official script as much as possible.** Our number only means
something if it's computed the way the Python typing community computes every checker's — so the less
home-grown scoring logic sits between us and the official tool, the more trustworthy our figure is.

Audit against the official `python/typing` conformance suite:

- the full [test suite](https://github.com/python/typing/tree/main/conformance/tests) (`conformance/tests/`) — are we running **every** fixture the PEP system demands, with nothing quietly skipped or excluded? A high percentage over a subset of the tests is not a real percentage.
- the [scoring methodology](https://github.com/python/typing/blob/main/conformance/README.md) (`conformance/README.md`) — does our methodology match it?
- the [Python scoring script](https://github.com/python/typing/blob/main/conformance/src/main.py) (`conformance/src/main.py`) — are we running *that*, unmodified at a pinned commit, rather than reimplementing it? Every place we diverge is a place our number can drift from reality.
- the published [results table](https://github.com/python/typing/blob/main/conformance/results/results.html) — the scoreboard we want to be listed on; diff our standing against what the official tool actually reports.

Flag any gap between what we claim and what the official script says, and push our harness to lean on
that script wherever it currently doesn't. The score only moves **up**, and only because we genuinely
got more conformant — target **100%** — never because we changed how we count.

**Disabling any conformance rule to move the score is absolutely forbidden — a punishable offence.**
PEP conformance MUST run the `basilisk` binary with **every rule enabled**: no `basilisk.json`, no
per-rule override, no "spec-conformance mode", no skipped fixtures, no exceptions. The score is
exactly what a real user gets out of the box. If a strict default fires on spec-valid code, **fix the
checker** so it stops firing — never silence the rule to inflate the number. This is precisely how the
number was once faked: house-style rules were switched off at score time to report a hollow 100% while
the false positives were merely hidden, not eliminated. That disabling has been removed; the only
legitimate path to 100% is making the checker smarter with every rule still on ([CHKARCH-CONFORMANCE]).

### 3. Maintain and improve code quality

Review AI-authored PRs against the bar in [`CLAUDE.md`](CLAUDE.md): *code here should
comfortably pass review at a top-tier engineering organization.* Catch over-engineering,
premature abstraction, duplicated logic, and the subtly-wrong-but-plausible. An agent will
happily ship something that compiles and passes tests but reads badly or hides a landmine —
your job is to say so.

### 4. Improve test metrics and the mutation score

Coverage percentage is the floor, not the goal. Judge whether assertions actually *prove*
something or just execute lines. Push for stronger assertions, widen the mutation-testing
scope ([CHKARCH-TESTING-MUTATION-RATCHET]), and call out tests that would still pass if the
code were broken. **Both ratchets only move one way** — coverage and mutation score up,
never down.

### 5. Guard the performance numbers

"Fastest" is the other half of the promise, and it's just as easy to fool yourself on. Re-run the
benchmarks on real hardware against the committed baseline (`benchmarks/status/<machine>.csv`),
confirm no fixture regressed past the gate, and make sure a `BENCH_NO_GATE=1` baseline reset was
genuinely a fixture-set change and not a quiet slowdown waved through. A conformance fix that blows
the benchmark gate isn't done — and a benchmark "win" that cost conformance isn't either. Both
ratchets hold at once.

### 6. Report GitHub issues

You're the one running real-world Python through Basilisk. When something is wrong — a false
positive, a missed error, a crash, a slow path, a clumsy editor interaction — file a precise,
reproducible issue with the smallest snippet that triggers it. A good bug report is a gift; it
becomes a failing test, which becomes a fix.

### 7. Check plans and specs against reality

Specs and plans are the fabric of this repo (see [`docs/INDEX.md`](docs/INDEX.md)). Audit them:
does every spec section have a non-numeric, hierarchical spec ID? Does the implementing code
actually reference that ID? Does the implementation *match* the spec, or has it drifted? Are the
plans still accurate, or do they describe a world that no longer exists? The `/spec-check` workflow
helps, but the judgment call — *is this spec still telling the truth?* — is yours.

### 8. Ensure feature parity across IDE extensions

The promise is **one seamless experience in every editor**: VS Code (plus Cursor/Windsurf via
Open VSX), Zed, and Neovim. A feature that lands in one extension but not the others is a parity
bug. Audit the extensions side by side, find the gaps, and file them. Remember the architecture
rule: the **LSP drives functionality** — extensions only react to what the LSP advertises.

### 9. Security auditing

Threat-model the checker, the LSP, the editor extensions, the release pipeline, and the
dependency tree. Review what `/security-review` and Dependabot surface with a human's sense of
*what actually matters*. Single binary, no runtime, no telemetry is a security posture — help us
keep it true.

### 10. Improve the AI instructions

This is the highest-**compounding** human lever. Better instructions produce better AI output on
every future task. Tighten [`CLAUDE.md`](CLAUDE.md), the specs, and the skills under `.claude/`.
When you watch an agent go wrong, the fix usually isn't the code — it's the instruction that
allowed it.

### 11. Everything humans are simply best at

Brand voice and naming. Outreach, relationships, and community. Strategic prioritization — *what
should we even build next?* Native-speaker and design judgment. Anything involving accounts,
secrets, tokens, or money. If it can't be checked by a test, it's probably your call.

### How to contribute as a human

1. **Open an issue** for a bug, a parity gap, a spec drift, or a conformance discrepancy. Be specific and reproducible.
2. **Open a PR** for fixes or docs — fill out the [pull request template](.github/pull_request_template.md) honestly. "Tests pass" is not an acceptable answer to *how do the tests prove it works?*
3. **Review PRs** — the review itself is a first-class contribution, often the most valuable one.

---

## For AI

You do most of the technical work. The standing rules that govern it live in
[**`CLAUDE.md`**](CLAUDE.md), and they **override default behavior** — read that file first and
follow it exactly. This section is a map, not a restatement (we don't duplicate).

**Before you touch anything:**

- Read [`CLAUDE.md`](CLAUDE.md) in full. Then orient via [`docs/INDEX.md`](docs/INDEX.md) and the
  source-of-truth spec [`docs/specs/LSP-ARCHITECTURE-SPEC.md`](docs/specs/LSP-ARCHITECTURE-SPEC.md).
- Register with the **too-many-cooks** coordinator and **lock files** before editing them. Don't
  edit a locked file.

**The non-negotiables** (full detail in `CLAUDE.md`):

- **Git is off-limits unless explicitly asked.** Never push to `main`, never list an agent as a
  co-author, never use worktrees, work on exactly one branch.
- **Spec IDs are the fabric.** Every spec section has a non-numeric, hierarchical ID; every piece
  of code references it (`// Implements [LSP-…]`); every test cross-references both. If you find a
  link missing, fix it.
- **DRY, ruthlessly.** Use the `deslop` MCP (`find-similar` before writing, `top-offenders` after).
  Merge duplicates. Search for existing code before adding new code.
- **The ratchets only move one way.** Conformance score up; false positives and benchmark
  regressions down; coverage up; mutation score up. A conformance fix that blows the benchmark gate
  isn't done.
- **Never disable a conformance rule to move the score — a punishable offence.** PEP conformance runs
  the `basilisk` binary with **every rule enabled**: no `basilisk.json`, no per-rule override, no
  "spec-conformance mode", no skipped fixtures, no exceptions. The score is exactly what a real user
  gets out of the box. If a strict default fires on spec-valid code, **fix the checker** so it stops
  firing — never silence the rule to inflate the number ([CHKARCH-CONFORMANCE]).
- **`make` is the interface.** `make build | test | lint | fmt | clean | ci | setup` — exactly seven
  targets, don't add more. `make test` is fail-fast and enforces the coverage threshold from
  `coverage-thresholds.json`.
- **Rust quality bar:** no `unwrap`, `panic!`, `todo!`, `unimplemented!`, `unsafe`, or
  `allow(clippy::…)`. `Result`/`Option` everywhere, small pure functions, files under 500 LOC.
- **No CI artifacts.** Storage is billed even on this public repo — see [GITHUB-NO-ARTIFACTS].

**How you work:**

- **Test-driven, always.** Write a failing test → confirm it fails *for the right reason* → fix the
  code (never the test) → confirm it passes. Coarse e2e tests only; no unit tests. Never delete a
  failing test or weaken an assertion.
- **Use judgment; don't stop to ask.** Make the call and proceed.
- **Pick up `[AGENT]` work** from [`docs/plans/ROADMAP-NEXT-STEPS-PLAN.md`](docs/plans/ROADMAP-NEXT-STEPS-PLAN.md).
  Leave `[HUMAN]` work for a human. Draft the agent half of `[HYBRID]` items and hand them off.
- **Defer to the human signals** in this guide. When a human reports an issue, that report becomes
  your failing test.
