# Basilisk — Next Steps Roadmap (Post-Launch)

> **Nature of this doc**: This is an *aggregation* roadmap, not a detailed implementation plan. It
> deliberately stays shallow and links out to the per-area specs and plans that carry the detail
> (or flags where one doesn't exist yet). Treat it as the map, not the territory.
>
> **Source-of-truth specs**: `docs/specs/LSP-ARCHITECTURE-SPEC.md` (shared LSP/DAP/config/commands),
> plus the per-editor specs (`VSIX-SPEC.md`, `ZED-SPEC.md`, `NEOVIM-SPEC.md`).
>
> **Last surveyed**: 2026-05-30.

---

## How to read this doc

Each section is a rough overview of an area of work, with the *current state* noted up front so we
don't re-litigate what's already done. The actionable, divided-up checklist lives at the **bottom**
of this doc under [Detailed TODO](#detailed-todo).

Every TODO item is tagged so we know who picks it up:

- **`[AGENT]`** — mechanical, verifiable, code/test/docs work an agent can drive end-to-end.
- **`[HUMAN]`** — requires human discretion: accounts, secrets/tokens, money, brand voice, outreach
  relationships, strategic prioritization, or native-speaker judgment.
- **`[HYBRID]`** — agent drafts/prepares; human reviews, approves, or supplies credentials.

---

## Top 5 bang-for-buck — start here

> Ranked by reward ÷ effort, for the goal of broadening IDE coverage and getting discovered.
> Sequence matters: **1 → 2** (publish everywhere *before* you announce).

1. **Open VSX → Cursor & Windsurf** *(TODO B)* — **Effort: tiny. Reward: huge.** The VSIX is already
   built; this is one `ovsx publish` CI step + an `OVSX_PAT`. Instantly puts Basilisk in the two
   fastest-growing Python editors, and the site already promises them ("coming very soon"). Nothing
   else has this ratio. **Do this first** so the launch can truthfully say "VS Code, Cursor, Windsurf."

2. **Launch announcement blitz** *(TODO M)* — **Effort: low. Reward: highest for discovery.** ~14
   installs = invisible. The product already ships on VS Code; the gap is distribution. One coordinated
   push (Show HN + r/Python + r/rust + dev.to + X — "open-source Pylance replacement, strict-by-default,
   in Rust") can drive thousands of installs + stars in 48h. Sequence right after #1 so arrivals can
   install everywhere. The single biggest "people actually find out" lever.

3. **Get listed on the official Python typing conformance results** *(TODO H + G)* — **Effort: medium.
   Reward: very high.** We're at 82.9% (121/146, per the unmodified python/typing scorer, binary in spec-conformance mode); even at this
   score, submitting results earns a spot on the scoreboard the whole target audience watches (mypy sits
   at ~58%), and every failing file we close lifts our standing. Correctness + credibility + organic discovery in one (the
   Zuban/David Halter precedent proves it draws eyes).

4. **Ship Neovim + Zed for real** *(TODO A/B)* — **Effort: low-medium. Reward: high.** Both are ~95%
   done. Neovim needs a tagged release + binary auto-download; Zed needs the registry-publish PR.
   Cheaply covers two evangelist-heavy communities that amplify disproportionately.

5. **Publishable benchmark vs Pyright on large real codebases** *(TODO E)* — **Effort: medium. Reward:
   high.** Turns "fast" from a claim into headline numbers — ammunition for #2's posts, the website,
   and #3's story. Content leverage that compounds.

---

## 1. Editor releases (the critical path)

This is the single highest-leverage block: the code is largely done, but **nothing actually ships
to users yet**. Every extension is still stamped `0.0.0-PLACEHOLDER`, and `release.yml` builds the
core binaries but has no marketplace-publish steps.

**Shared release plumbing (do this first — everything else depends on it):**
- Version stamping: replace `0.0.0-PLACEHOLDER` across `vscode-extension/`, `basilisk-zed/`,
  `basilisk.nvim/` with a single source of truth driven from a git tag.
- Binary distribution: tagged GitHub releases with per-platform binaries (extensions download these
  on first run — Neovim's auto-download is the last gap in `NEOVIM-PLAN.md`).

**Per-editor state:**

| Editor | Code state | Release gap |
|---|---|---|
| **Neovim** (`basilisk.nvim`) | ~95%, 189 e2e tests passing, feature parity reached | Binary auto-download + outdated-binary version check; tagging/release mechanism (luarocks optional) |
| **Zed** (`basilisk-zed`) | WASM builds, DAP + slash commands declared | No Zed extension-registry publish step in CI; version stamp |
| **VS Code** (`vscode-extension`) | Full feature set, 297 tests, marketplace metadata set | No `vsce` package/publish workflow; version stamp |
| **Open VSX** (Cursor/Windsurf) | Same VSIX artifact as VS Code | No `ovsx` publish step — this is what makes us installable in Cursor & Windsurf |

Each editor needs a **real-world smoke test on a clean machine** before its first publish — install
the published artifact (not the dev build), open a Python project, confirm diagnostics, hover,
go-to-def, debug, and profile all light up. That's a human sign-off step.

---

## 2. Debugging (DAP) — cross-editor verification

Spec: `docs/specs/LSP-DEBUG-INTEGRATION-SPEC.md`. The Basilisk binary serves as both language server
and debug adapter (embedded debugpy over TCP). Implementation exists and is tested in all three
editors; the `loop_and_accumulate` race condition is fixed (`VSIX-DEBUG-LOOP-TIMEOUT-PLAN.md` — done).

Remaining work is **verification, not building**: confirm a breakpoint → step → inspect → continue
loop works against a published artifact in each editor on a clean machine, and that graceful
degradation holds when the editor's DAP client (e.g. nvim-dap) is absent.

---

## 3. Profiling — smoothing off

Spec: `docs/specs/LSP-PROFILING-SPEC.md`. py-spy is embedded; start/stop/snapshot requests, CLI,
and per-editor UI (VS Code command, Zed `/profile`, Neovim heat map) all exist and have e2e tests.

This is mostly polish: the macOS elevation prompt was a sharp edge (handled in the debug plan), and
the inline visualization / Speedscope hand-off wants a real-world pass for UX rough edges. Treat as
"smoothing," not net-new feature work.

---

## 4. Competitive parity with Pyright / Pylance

The bar to credibly displace Pylance is feature *and* correctness parity on the things people
actually feel day to day. Rough priorities (refine with human judgment — see TODO):

- **Conformance & correctness**: per the official `python/typing` scorer (run unmodified, pinned
  commit), PEP conformance is currently **121/146 files PASS (82.9%, errors+warnings strictest)**, with **24 false
  positives** and 36 missed required errors, running the binary in spec-conformance mode (basilisk's non-spec
  house-style rules off — see CHKARCH-CONFORMANCE-MODE; the honest number with them on was 40.4%). (Earlier
  "135/146 / ~18 FPs" figures came from an earlier in-repo (miscalculating) harness that excluded codes from the
  *scorer* and ignored false positives; they are superseded.) Failing files
  cluster in Protocols, Callables, TypeVarTuple, ParamSpec, TypedDicts. FPs hurt credibility more
  than missed cases — prioritize accordingly.
- **Latency**: sub-10ms incremental checks are the promise (Salsa). Need a published benchmark vs.
  Pyright/Pylance — see §5 for the scale/resource methodology.
- **Editor UX parity**: completions, signature help, semantic tokens, inlay hints, organize-imports,
  workspace symbol search — audit each against Pylance and note any gaps.
- **Trust signals**: getting listed on the official
  [Python type-checker conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html)
  is a marketing-grade credibility win and a concrete conformance target.

---

## 5. Scale & resource testing (large real-world codebases)

We claim "fast" — we need to *prove* it on real, famously large Python codebases, not just our own
fixtures. This is an objective-measurement exercise:

- **Corpus**: download a handful of well-known large codebases (e.g. CPython's `Lib/`, Django,
  pandas, Home Assistant, SymPy, Salt, Sentry) and run full + incremental checks against each.
- **Objective measurements**: peak RSS / memory high-water mark, CPU time, full-check wall-clock,
  incremental-check latency distribution (p50/p95/p99), and any panics/crashes/hangs. Monitor
  **CPU and memory** throughout — capture numbers, not vibes.
- **Comparison**: run the same corpus through Pyright where feasible and tabulate side-by-side.
- **Output**: a reproducible benchmark harness + a results table we can publish (feeds §4 and the
  marketing material in §12). Fixtures must not be committed to the repo (they're large and external).

---

## 6. Typing-philosophy survey (strict-by-default + the downgrade path)

The product promise is **strict-by-default with a graceful off-ramp** ("flick errors down to
warnings, adopt incrementally"). This section is a *survey + proof* task, not new features:

- **Confirm the default really is strict.** Severity is encoded in the rule code prefix
  (`BSK-E` = error, `BSK-W` = warning). Verify there is no hidden lenient default.
- **Confirm the off-ramp works and is easy.** Downgrades exist at four levels: per-rule
  (`[tool.basilisk.rules."BSK-E…"] = "warning"` in `pyproject.toml`), per-path, gradual-adoption file
  (`adoptions.toml`, auto-generated by mass-fix — see `docs/specs/LSP-MASS-AUTOFIX-SPEC.md`), and
  `# type: ignore` line/file. Verify each path end-to-end.
- **Prove it with tests.** We need explicit e2e tests that assert (a) a fresh project is strict by
  default, and (b) each downgrade mechanism actually lowers severity. If those assertions don't
  exist, add them. The deliverable is a short written survey ("is it strict AF, and is the
  down-shift easy?") backed by named, passing tests.

---

## 7. MCP server — semantic codebase navigation for agents

A first-class **Basilisk MCP server** that exposes the checker's semantic understanding (symbols,
resolved types, signatures, call/import graph, type health) to AI agents, so an agent navigating a
Python codebase *knows what's actually there* instead of grepping blind. This is a natural extension
of the LSP intelligence we already compute — the same index, surfaced over MCP.

State: no Basilisk product MCP server exists yet (the `deslop` MCP used in this repo is a dev-time
dedup tool, not this). Needs a spec + plan before building. Likely tools: semantic symbol search,
"what type is this", "who calls this", "what implements this protocol", module/type-health summaries.

---

## 8. Deeper AI integration (nimblesite.ai → autofixes)

Specs/plans exist: `docs/specs/LSP-AI-SPEC.md` + `docs/plans/LSP-AI-PLAN.md` define a **model-agnostic**
AI layer (deterministic features work without it; AI is optional and pluggable).

Next step: wire the **nimblesite.ai agent** in as a provider behind the existing AI-fix hooks so it
can drive autofixes (and, later, completions/refactoring), then back it with tests. Keep the
model-agnostic contract intact — nimblesite.ai becomes *a* provider, not a hard dependency. Tests
should cover the provider boundary with a deterministic/mocked backend so the suite stays
hermetic, plus an opt-in integration test against the real agent.

---

## 9. Finish near-complete plans (bang for buck)

Several of these are close enough that finishing them is cheap and visibly improves the product (the
conformance and false-positive work is larger — sized honestly below against the unmodified scorer):

- **`CHECK-ELIMINATE-FALSE-POSITIVES.md`** (active): the real python/typing scorer reports **24 false
  positives** to drive down (down from 285 — most were basilisk's non-spec house-style rules, now off in
  spec-conformance mode; the old "~18 FPs left" came from the earlier in-repo harness — a
  miscalculation — and is superseded). **Plus an open showstopper**: `BSK-E0149` line-scans source text and misfires on
  docstrings containing `class`/`def` prefixes + bracketed tokens (e.g. our own `[SPEC-ID]` convention).
  Re-ground the rule on the AST. High credibility payoff.
- **`CHECKER-PEP-CONFORMANCE-PLAN.md`** (active, 82.9% — 121/146): clear the **25 failing files** toward the
  conformance results listing.
- **`CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md`** (~40%): the E0149 fix above is part of this; finish
  Phase 4 (wire the no-line-scanning lint into CI so the anti-pattern can't return).
- **`LSP-STUBBING-PLAN.md`** (~95%, Phase 5 deferred): essentially shippable; decide whether the
  deferred Salsa perf work is worth doing now or later.
- **`NEOVIM-PLAN.md`** (~95%): the two gaps (version check + binary auto-download) overlap with §1.

---

## 10. IntelliJ / PyCharm (JetBrains)

No plugin code exists; the website already says "coming soon." This is the largest *net-new* effort
on the list and a strategic call: the JetBrains/PyCharm Python audience is huge, but a JetBrains
plugin is a real project (Kotlin/Java plugin, LSP4IJ or custom client, JetBrains Marketplace
process). The LSP-first architecture helps — much of the value is already in the server.

**Decision needed (human):** commit to a JetBrains plugin now, or defer until the other editors are
shipped and stable? If we go: start with a spec + plan doc (`docs/specs/JETBRAINS-SPEC.md`,
`docs/plans/JETBRAINS-PLAN.md`) mirroring the existing per-editor docs.

---

## 11. Internationalization & translation

**Current state:** the Eleventy site has `i18n: false`; a Chinese mirror exists under
`website/src/zh/` but it's **hand-maintained parallel pages**, not a real i18n system. That doesn't
scale to more languages. The product itself (diagnostic messages, CLI output) is English-only.

**Target languages (ranked by community size × English-proficiency gap):**

1. **Chinese (Simplified)** — non-negotiable #1. Largest community, low English proficiency. (Partial
   content already exists in `/zh`.)
2. **Japanese** — smaller community, but low and *declining* English proficiency; an English-only
   site genuinely locks them out.
3. **Portuguese (Brazil)** — large, very active Python scene (Python Brasil), low English proficiency.
4. **Spanish** — large aggregate market at moderate proficiency. **Stretch / next after the top 3.**

**Scope (three surfaces — increasing difficulty):**

- **Website → top 3** (zh-Hans, ja, pt-BR). First build a real i18n system in Eleventy and fold the
  existing `/zh` content into it, *then* add ja and pt-BR.
- **Extension UI → top 3.** Investigate VS Code's `package.nls.<locale>.json` localization for command
  titles/settings descriptions; assess whether Zed/Neovim have a comparable story (likely limited —
  report findings rather than assuming).
- **LSP / CLI output → top 3 (the hard, high-value one).** Localize the actual **diagnostic messages
  and CLI text** emitted by the Rust core. This touches every `BSK-E####`/`BSK-W####` message string,
  so it needs a message-catalog / localization layer in the checker (locale resolved from config or
  `LANG`), keyed by diagnostic code. Significant but it's what makes Basilisk genuinely usable by
  non-English developers — the diagnostics are the product.

Machine-translate every surface as a first pass, but **each locale needs native-speaker review
before it goes live** — especially diagnostics, where wording precision matters.

---

## 12. Marketing & community

Assets that exist: launch blog post (`website/src/blog/introducing-basilisk.md`), comparison/feature
docs, READMEs, OG image + logo. What's missing is *distribution*.

Rough plan (most of this is human-led — voice, accounts, timing, relationships, budget):
- **Owned content**: launch blog post polish; follow-up technical posts (the Rust/Salsa
  architecture, the strict-by-default philosophy, the scale-benchmark numbers from §5, "we got
  listed on the conformance results").
- **Syndication**: X threads, Reddit (r/Python, r/rust), [dev.to](https://dev.to) cross-posts,
  Hacker News (Show HN). An agent can *draft* these; a human owns posting, voice, and timing.
- **Community outreach**: Python Discord/forums, Python Brasil, JP Python communities (ties into i18n).
- **Paid**: paid UGC / sponsorships — budget and vendor selection are human calls.

---

# Detailed TODO

> Legend: **`[AGENT]`** agent-drivable · **`[HUMAN]`** human discretion · **`[HYBRID]`** agent
> prepares, human approves/supplies credentials. Ordering within a group is rough priority.

## A. Shared release plumbing  *(do first)*

- [ ] **`[AGENT]`** Replace `0.0.0-PLACEHOLDER` everywhere with a single git-tag-driven version source.
- [ ] **`[AGENT]`** Extend `release.yml` to publish per-platform binaries on tagged releases.
- [ ] **`[AGENT]`** Finish Neovim binary auto-download from GitHub releases + outdated-binary version check (last `NEOVIM-PLAN.md` gaps).
- [ ] **`[HUMAN]`** Create/confirm publisher accounts and store CI secrets: VS Code Marketplace (VSCE_PAT), Open VSX (OVSX_PAT), Zed registry, (optional) luarocks.
- [ ] **`[HUMAN]`** Decide the versioning scheme (single repo-wide version vs. per-extension) and the release cadence.
- [ ] **`[AGENT]`** Website: if a version is displayed anywhere, source it dynamically from the latest GitHub release — never hardcode it in copy (hardcoded `v0.1` strings were removed 2026-05-30; `site.json` still carries `0.0.0-PLACEHOLDER`).

## B. Editor publishing

- [ ] **`[HYBRID]`** Add `vsce` package + VS Code Marketplace publish workflow (agent writes it; human holds the token).
- [ ] **`[HYBRID]`** Add `ovsx` publish workflow so Basilisk is installable in **Cursor & Windsurf**.
- [ ] **`[HYBRID]`** Add Zed extension-registry publish step.
- [ ] **`[AGENT]`** Verify/expand e2e + screenshot regression suites for nvim/zed/vsix before each first publish.
- [ ] **`[HUMAN]`** Clean-machine smoke test of each **published** artifact (install from marketplace, not dev build): diagnostics, hover, go-to-def, debug, profile.

## C. Debugging (DAP) verification

- [ ] **`[AGENT]`** Confirm DAP e2e tests cover breakpoint → step → inspect → continue in all three editors.
- [ ] **`[HUMAN]`** Manual debug session against a published artifact per editor; confirm graceful degradation when the editor DAP client is absent.

## D. Profiling smoothing

- [ ] **`[AGENT]`** Harden profiler e2e tests (start/stop/snapshot, Speedscope export integrity).
- [ ] **`[HUMAN]`** UX pass on inline visualization / heat map across editors; note rough edges (incl. macOS elevation prompt behaviour).

## E. Scale & resource testing

- [ ] **`[AGENT]`** Build a reproducible benchmark harness that clones N large public Python repos and runs full + incremental checks (fixtures stay out of the repo).
- [ ] **`[AGENT]`** Capture objective metrics per repo: peak RSS, CPU time, full-check wall-clock, incremental latency p50/p95/p99, panic/crash/hang count.
- [ ] **`[AGENT]`** Run the same corpus through Pyright and produce a side-by-side results table.
- [ ] **`[HUMAN]`** Pick the canonical corpus and the headline numbers to publish.

## F. Typing-philosophy survey + proof

- [ ] **`[AGENT]`** Audit and document where the default severity is set; confirm no lenient default exists.
- [ ] **`[AGENT]`** Add/confirm e2e tests asserting (a) fresh project is strict by default, (b) each downgrade path (per-rule, per-path, `adoptions.toml`, `# type: ignore`) lowers severity.
- [ ] **`[AGENT]`** Write the short survey writeup ("strict AF? easy to down-shift?") linked to the named tests.

## G. Finish near-complete plans

- [ ] **`[AGENT]`** Fix `BSK-E0149` docstring/line-scanning showstopper — re-ground the rule on the AST (`CHECK-ELIMINATE-FALSE-POSITIVES.md`).
- [ ] **`[AGENT]`** Clear the remaining 24 false positives.
- [ ] **`[AGENT]`** Close the 25 failing PEP-conformance files (Protocols, Callables, TypeVarTuple, ParamSpec, TypedDicts).
- [ ] **`[AGENT]`** Finish `CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md` Phase 4 — wire the no-line-scanning lint into CI.
- [ ] **`[HUMAN]`** Decide whether `LSP-STUBBING-PLAN.md` Phase 5 (Salsa perf) ships now or later.

## H. Competitive parity (Pyright/Pylance)

- [ ] **`[HUMAN]`** Prioritize the parity gap list — which features actually move adoption.
- [ ] **`[AGENT]`** Audit editor UX features vs. Pylance (completions, signature help, semantic tokens, inlay hints, organize-imports, workspace symbols) and report gaps.
- [ ] **`[HYBRID]`** Drive toward the official Python conformance-results listing (agent does the conformance work; human handles the submission).

## I. MCP server (semantic codebase navigation)

- [ ] **`[AGENT]`** Draft `docs/specs/MCP-SERVER-SPEC.md` + plan — tool surface, transport, how it reuses the existing LSP/workspace index.
- [ ] **`[AGENT]`** Implement the server (semantic symbol search, type-of, callers, protocol implementers, module/type-health summaries) with e2e tests.
- [ ] **`[HUMAN]`** Decide packaging/distribution (bundled in the binary vs. separate) and which agent ecosystems to target first.

## J. Deeper AI integration (nimblesite.ai)

- [ ] **`[AGENT]`** Wire nimblesite.ai as a provider behind the existing model-agnostic AI-fix hooks (`LSP-AI-SPEC.md`), preserving the optional/pluggable contract.
- [ ] **`[AGENT]`** Add hermetic provider-boundary tests (mocked backend) plus an opt-in integration test against the real agent.
- [ ] **`[HUMAN]`** Provide nimblesite.ai credentials/config and decide default-on vs. opt-in.

## K. IntelliJ / PyCharm

- [ ] **`[HUMAN]`** Go/no-go decision on a JetBrains plugin now vs. deferring.
- [ ] **`[AGENT]`** *(if go)* Draft `docs/specs/JETBRAINS-SPEC.md` + `docs/plans/JETBRAINS-PLAN.md` mirroring the per-editor docs (LSP-first, LSP4IJ vs. custom client).

## L. Internationalization & translation

- [ ] **`[AGENT]`** Build a real Eleventy i18n system; fold existing `/zh` content into it (replace the manual parallel pages).
- [ ] **`[AGENT]`** Machine-translate the website into the top 3 (zh-Hans, ja, pt-BR) as a first pass.
- [ ] **`[AGENT]`** Investigate VSIX localization (`package.nls.<locale>.json`) and report what's feasible for Zed/Neovim; then translate command/setting strings into the top 3.
- [ ] **`[AGENT]`** Add a message-catalog / localization layer to the Rust core (locale from config/`LANG`, keyed by diagnostic code) and translate **LSP/CLI diagnostic output** into the top 3.
- [ ] **`[HUMAN]`** Native-speaker review of every locale before go-live — especially diagnostics (consider hiring reviewers).
- [ ] **`[HUMAN]`** Decide if/when Spanish (#4) gets added after the top 3.

## M. Marketing & community

- [ ] **`[AGENT]`** Polish the launch blog post; draft 2–3 follow-up technical posts (architecture, strict-by-default philosophy, scale-benchmark numbers, conformance milestone).
- [ ] **`[AGENT]`** Draft X threads, Reddit (r/Python, r/rust), dev.to, and Show HN posts for human review.
- [ ] **`[HUMAN]`** Own posting: accounts, brand voice, timing, and engagement.
- [ ] **`[HUMAN]`** Community outreach (Python Discord/forums, Python Brasil, JP communities) — ties into i18n.
- [ ] **`[HUMAN]`** Evaluate paid UGC / sponsorships — budget and vendor selection.
