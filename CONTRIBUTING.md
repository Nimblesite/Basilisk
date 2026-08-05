# Contributing to Basilisk

<p align="center"><strong>English</strong> · <a href="CONTRIBUTING.zh.md">简体中文</a></p>

Basilisk is built by a **human + AI partnership**, and the work is split on purpose.
AI agents do the bulk of the mechanical, verifiable engineering. Humans do the things
that need taste, judgment, accountability, and trust — the things AI can't (yet) own.

This guide has two sections. Pick the one that's you.

- [**For Humans**](#for-humans) — judgment, taste, trust, and everything an agent can't be held accountable for. Express in the specs first and foremost.
- [**For AI**](#for-ai) — the technical execution, under a strict set of rules that conform to the Basilisk specs.

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

You don't need to write Rust to make Basilisk better. The **single highest-leverage thing a human can
do on this project is keep the agents honest**. Basilisk's former 100% conformance claim was
retracted because parts of the checker had been fitted to the exact text of the upstream fixtures;
the result did not survive AST-preserving mutations. Basilisk was removed from the
[official results table](https://github.com/python/typing/blob/main/conformance/results/results.html)
at the project's request. Current conformance is temporarily unknown while the affected logic is
rebuilt from the typing specification, and all published benchmark figures and rankings are also
withdrawn pending audit.

Agents do the bulk of the mechanical engineering. Humans own the judgment, accountability, and trust
an agent cannot be held to. In rough order of impact:

### 1. Keep the agents honest — raw fixture scores are not proof

Running the unmodified upstream harness is necessary, but it is not sufficient evidence of
conformance. A checker can score well by recognising the fixtures rather than implementing the
specification. **Every conformance change must pass all of these independent checks:**

- Run the complete official [`python/typing` suite](https://github.com/python/typing/tree/main/conformance/tests)
  with its [own scoring harness](https://github.com/python/typing/blob/main/conformance/src/main.py),
  unmodified and pinned to the reported commit. Do not skip fixtures, diagnostic codes, or required
  errors.
- Apply AST-preserving mutations to the affected fixtures, including consistent renames of type
  variables and symbols. Semantically equivalent input must produce equivalent diagnostics; a score
  that moves because spelling changed is invalid.
- Add off-suite positive, negative, and boundary tests derived independently from the typing
  specification. Do not copy, lightly rephrase, or merely permute the upstream fixture.
- Inspect the implementation for source-text substrings, filename checks, exact prefixes, fixture
  vocabulary, or branches that map one-for-one to upstream test lines. Delete fitted logic and
  reimplement the rule from the AST and specification.
- Exercise the default user-facing binary. A future published result must describe what users
  actually receive, not a special scoring mode or configuration.

Treat every metric change as a possible integrity failure until it has been independently re-derived.
The common dodges are:

- **Silence instead of fix** — disabling, deleting, or unregistering a rule merely so it stops
  firing, instead of implementing its intended behaviour.
- **Weaken the test** — deleting failures, cutting assertions, or watering them down so "green"
  means nothing.
- **Edit the scoreboard or gate** — hand-editing `conformance_status.csv`, lowering coverage or
  mutation thresholds, or discarding inconvenient benchmark results.
- **Measure less** — excluding diagnostics, skipping fixtures, narrowing mutation scope, or grading
  only a favourable subset.
- **Fit to the test** — encoding the fixture's spelling or shape instead of the general rule it is
  meant to exercise.

A raw upstream percentage, on its own, is not publishable evidence. Until the clean implementation
passes the upstream harness, AST-preserving mutation tests, and independent off-suite tests, the
honest conformance answer remains **temporarily unknown** ([CHKARCH-CONFORMANCE]).

### 2. Test it for real — on real, large codebases

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

### 5. Audit performance measurements

All published benchmark figures and performance rankings are withdrawn pending audit. Do not call
Basilisk the fastest checker or treat the committed timings as current evidence. Re-run benchmarks
on quiet, real hardware; compare tools or revisions within the same run; verify release builds,
tool versions, fixture selection, warm/cold definitions, and complete result recording. Publish slow
results as readily as fast ones. Benchmarks are indicative measurements, not a pass/fail gate, and
new public figures require a reproducible methodology that has survived independent review.

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

You convert the specs to code and tests, and keep all three in sync. The standing rules that govern it live in
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
- **A raw conformance score is necessary, never sufficient.** Every affected rule needs
  AST-preserving mutation coverage and independently derived off-suite tests. Current conformance is
  temporarily unknown; do not publish a replacement number until that evidence exists.
- **The valid ratchets only move one way.** Coverage and mutation strength go up; false positives go
  down. Never improve a metric by weakening its test, scope, or baseline.
- **Never disable, delete, or unregister a conformance rule merely to move the score.**
  PEP conformance runs the `basilisk` binary with **every rule enabled**: no Basilisk config file, no
  per-rule override, no "spec-conformance mode", no skipped fixtures, **no deleting rule source files,
  no removing rules from `all_rules()`**, no exceptions. Equally forbidden: hand-editing
  `conformance/conformance_status.csv` or loosening the `coverage-thresholds.json` gate to match a
  faked run. Also forbidden: raw-source matching, filename checks, or fixture-specific branches.
  If a strict default fires on spec-valid code, **reimplement the checker logic** so it understands
  the construct — never silence a rule or transcribe the test to inflate the number
  ([CHKARCH-CONFORMANCE]).
- **Benchmark claims are withdrawn.** Record every run honestly, but do not claim a ranking or treat
  benchmark timings as a gate until the methodology audit is complete.
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
