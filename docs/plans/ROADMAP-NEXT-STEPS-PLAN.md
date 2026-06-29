# Basilisk — Next Steps Roadmap (Post-Launch) {#NEXTSTEPS}

> **Nature of this doc**: an *aggregation* roadmap, not a detailed plan. It stays shallow and links
> out to the per-area specs/plans that carry the detail (or flags where one doesn't exist).
>
> **Source-of-truth specs**: `docs/specs/LSP-ARCHITECTURE-SPEC.md` (shared LSP/DAP/config/commands),
> plus per-editor specs (`VSIX-SPEC.md`, `ZED-SPEC.md`, `NEOVIM-SPEC.md`).
>
> **Last surveyed**: 2026-05-30.

---

## How to read this doc {#NEXTSTEPS-HOW-TO-READ}

Each section overviews an area with its *current state* up front; the actionable checklist lives at
the bottom under [Detailed TODO](#detailed-todo). Every TODO item is tagged:

- **`[AGENT]`** — mechanical, verifiable code/test/docs work an agent can drive end-to-end.
- **`[HUMAN]`** — requires human discretion: accounts, secrets/tokens, money, brand voice, outreach,
  strategic prioritization, or native-speaker judgment.
- **`[HYBRID]`** — agent drafts/prepares; human reviews, approves, or supplies credentials.

---

## Top 5 bang-for-buck — start here {#NEXTSTEPS-TOP-5}

> Ranked by reward ÷ effort. Sequence: **1 → 2** (publish everywhere *before* announcing).

1. **Open VSX → Cursor & Windsurf** *(TODO B)* — one `ovsx publish` CI step + an `OVSX_PAT`; the VSIX
   is already built. Puts Basilisk in Cursor & Windsurf (the site already promises them). **Do first**
   so the launch can say "VS Code, Cursor, Windsurf."

2. **Launch announcement blitz** *(TODO M)* — one coordinated push (Show HN + r/Python + r/rust +
   dev.to + X). Sequence right after #1 so arrivals can install everywhere.

3. **Get listed on the official Python typing conformance results** *(TODO H + G)* — we're at 46.6%
   (68/146, unmodified python/typing scorer, every rule enabled, no config). Even now, submitting
   results earns a spot on the [official python/typing scoreboard](https://github.com/python/typing/tree/main/conformance/results), and every file we close lifts standing.

4. **Ship Neovim + Zed for real** *(TODO A/B)* — both ~95% done. Neovim needs a tagged release +
   binary auto-download; Zed needs the registry-publish PR.

5. **Publishable benchmark vs Pyright on large real codebases** *(TODO E)* — turns "fast" into
   headline numbers feeding #2 and #3.

---

## 1. Editor releases (the critical path) {#NEXTSTEPS-EDITOR-RELEASES}

The code is largely done, but **nothing ships to users yet**: every extension is stamped
`0.0.0-PLACEHOLDER`, and `release.yml` builds the core binaries but has no marketplace-publish steps.

**Shared release plumbing (do first):**
- Version stamping: replace `0.0.0-PLACEHOLDER` across `vscode-extension/`, `basilisk-zed/`,
  `basilisk.nvim/` with a single source of truth driven from a git tag.
- Binary distribution: tagged GitHub releases with per-platform binaries (extensions download these
  on first run — the Neovim plugin's auto-download + outdated-binary version check already ship).

**Per-editor state:**

| Editor | Code state | Release gap |
|---|---|---|
| **Neovim** (`basilisk.nvim`) | Feature-complete: 189 e2e tests passing, feature parity reached, binary auto-download + outdated-binary version check shipped | tagging/release mechanism (luarocks optional); nvim-lspconfig PR (`[HUMAN]`) |
| **Zed** (`basilisk-zed`) | WASM builds, DAP + slash commands declared | No Zed extension-registry publish step in CI; version stamp |
| **VS Code** (`vscode-extension`) | Full feature set, 297 tests, marketplace metadata set | No `vsce` package/publish workflow; version stamp |
| **Open VSX** (Cursor/Windsurf) | Same VSIX artifact as VS Code | No `ovsx` publish step — this is what makes us installable in Cursor & Windsurf |

Each editor needs a **real-world smoke test on a clean machine** before its first publish: install
the published artifact (not the dev build), open a Python project, confirm diagnostics, hover,
go-to-def, debug, and profile. A human sign-off step.

---

## 2. Debugging (DAP) — cross-editor verification {#NEXTSTEPS-DEBUGGING}

Spec: `docs/specs/LSP-DEBUG-INTEGRATION-SPEC.md`. The binary serves as language server and debug
adapter (embedded debugpy over TCP); implemented and tested in all three editors; the
`loop_and_accumulate` race is fixed (`VSIX-DEBUG-LOOP-TIMEOUT-PLAN.md` — done).

Remaining work is **verification, not building**: confirm breakpoint → step → inspect → continue
against a published artifact in each editor on a clean machine, and that graceful degradation holds
when the editor's DAP client (e.g. nvim-dap) is absent.

---

## 3. Profiling — smoothing off {#NEXTSTEPS-PROFILING}

Spec: `docs/specs/LSP-PROFILING-SPEC.md`. py-spy is embedded; start/stop/snapshot requests, CLI,
and per-editor UI (VS Code command, Zed `/profile`, Neovim heat map) exist and have e2e tests.

Remaining is polish: the macOS elevation prompt (handled in the debug plan) and a real-world UX pass
on inline visualization / Speedscope hand-off. Smoothing, not net-new work.

---

## 4. Competitive parity with Pyright / Pylance {#NEXTSTEPS-COMPETITIVE-PARITY}

Priorities (refine with human judgment — see TODO):

- **Conformance & correctness**: per the official `python/typing` scorer (unmodified, pinned commit
  `268d0c4e`), PEP conformance is **68/146 files PASS (46.6%, errors+warnings strictest)**, with
  **265 false positives** and **0 missed required errors** — every failing fixture is false positives
  from strict-by-default house-style rules (require-annotation E0001/E0002/E0004, missing-@override
  E0025, explicit-Any W0014, redundant-annotation W0050) firing on spec-valid code where the spec
  treats unannotated as inferred. Run with **every rule enabled** — no config, no `basilisk.json`, no
  "spec-conformance mode" (no such mode exists — see CHKARCH-CONFORMANCE-MODE). *(History: honest
  score was 40.4% / 285 FPs at PR #183; PRs #184/#185/#191 inflated it to a fake 100% via a
  `basilisk.json` disabling those 6 house rules at score time — now removed and forbidden; genuine
  progress 40.4% → 46.6%.)* Failing files cluster in Protocols, Callables, TypeVarTuple, ParamSpec,
  TypedDicts. The only legitimate path to 100% is fixing the checker so its strict defaults stop
  firing — never by disabling a rule.
- **Latency**: sub-10ms incremental checks are the promise (Salsa). Need a published benchmark vs.
  Pyright/Pylance — see §5 for methodology.
- **Editor UX parity**: audit completions, signature help, semantic tokens, inlay hints,
  organize-imports, workspace symbol search against Pylance; note gaps.
- **Trust signals**: listing on the official
  [Python type-checker conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html)
  is a concrete conformance target.

---

## 5. Scale & resource testing (large real-world codebases) {#NEXTSTEPS-SCALE-TESTING}

Prove "fast" on real large codebases, not just our fixtures. An objective-measurement exercise:

- **Corpus**: well-known large codebases (e.g. CPython's `Lib/`, Django, pandas, Home Assistant,
  SymPy, Salt, Sentry); run full + incremental checks against each.
- **Measurements**: peak RSS, CPU time, full-check wall-clock, incremental-check latency
  (p50/p95/p99), and any panics/crashes/hangs.
- **Comparison**: run the same corpus through Pyright where feasible; tabulate side-by-side.
- **Output**: a reproducible benchmark harness + a publishable results table (feeds §4 and §12).
  Fixtures must not be committed to the repo (large, external).

---

## 6. Typing-philosophy survey (strict-by-default + the downgrade path) {#NEXTSTEPS-TYPING-PHILOSOPHY}

The promise is **strict-by-default with a graceful off-ramp**. A survey + proof task, not new features:

- **Confirm the default is strict.** Severity is encoded in the rule prefix (`BSK-E` = error,
  `BSK-W` = warning). Verify no hidden lenient default.
- **Confirm the off-ramp works.** Downgrades exist at four levels: per-rule
  (`[tool.basilisk.rules."BSK-E…"] = "warning"` in `pyproject.toml`), per-path, gradual-adoption file
  (`adoptions.toml`, auto-generated by mass-fix — see `docs/specs/LSP-MASS-AUTOFIX-SPEC.md`), and
  `# type: ignore` line/file. Verify each end-to-end.
- **Prove it with tests.** Add e2e tests asserting (a) a fresh project is strict by default, and
  (b) each downgrade mechanism lowers severity. Deliverable: a short survey backed by named, passing
  tests.

---

## 7. MCP server — semantic codebase navigation for agents {#NEXTSTEPS-MCP-SERVER}

A **Basilisk MCP server** exposing the checker's semantic understanding (symbols, resolved types,
signatures, call/import graph, type health) to AI agents — the same index we compute for the LSP,
surfaced over MCP.

State: no Basilisk product MCP server exists yet (the `deslop` MCP in this repo is a dev-time dedup
tool, not this). Needs a spec + plan first. Likely tools: semantic symbol search, "what type is
this", "who calls this", "what implements this protocol", module/type-health summaries.

---

## 8. Deeper AI integration (nimblesite.ai → autofixes) {#NEXTSTEPS-AI-INTEGRATION}

Specs/plans exist: `docs/specs/LSP-AI-SPEC.md` + `docs/plans/LSP-AI-PLAN.md` define a **model-agnostic**
AI layer (deterministic features work without it; AI is optional and pluggable).

Next step: wire the **nimblesite.ai agent** as a provider behind the existing AI-fix hooks to drive
autofixes (later, completions/refactoring), then back it with tests. Keep the model-agnostic contract
intact — nimblesite.ai becomes *a* provider, not a hard dependency. Cover the provider boundary with
a mocked backend (hermetic) plus an opt-in integration test against the real agent.

---

## 9. Finish near-complete plans (bang for buck) {#NEXTSTEPS-FINISH-PLANS}

- **`CHECK-ELIMINATE-FALSE-POSITIVES.md`** (active): the real python/typing scorer (every rule
  enabled, no config) reports **265 false positives** to drive down — all from strict-by-default
  house-style rules firing on spec-valid code. The only legitimate fix is making the checker smarter;
  disabling a rule to hide them is forbidden (history: PRs #184/#185/#191 did exactly that to fake a
  100% score, now reverted). **Plus an open showstopper**: `generics_syntax_scoping` line-scans
  source text and misfires on docstrings containing `class`/`def` prefixes + bracketed tokens (e.g.
  our `[SPEC-ID]` convention). Re-ground the rule on the AST.
- **`CHECKER-PEP-CONFORMANCE-PLAN.md`** (active, 46.6% — 68/146, every rule enabled): clear the
  **78 failing files** toward the conformance results listing.
- **`CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md`** (~40%): the E0149 fix above is part of this; finish
  Phase 4 (wire the no-line-scanning lint into CI so the anti-pattern can't return).
- **`LSP-STUBBING-PLAN.md`** (~95%, Phase 5 deferred): shippable; decide whether the deferred Salsa
  perf work ships now or later.

---

## 10. IntelliJ / PyCharm (JetBrains) {#NEXTSTEPS-JETBRAINS}

No plugin code exists; the website says "coming soon." The largest *net-new* effort and a strategic
call: the JetBrains/PyCharm audience is huge, but a JetBrains plugin is a real project (Kotlin/Java
plugin, LSP4IJ or custom client, Marketplace process). The LSP-first architecture helps — much of
the value is already in the server.

**Decision needed (human):** commit now, or defer until the other editors ship and stabilize? If go:
start with `docs/specs/JETBRAINS-SPEC.md` + `docs/plans/JETBRAINS-PLAN.md` mirroring the per-editor docs.

---

## 11. Internationalization & translation {#NEXTSTEPS-I18N}

**Current state:** the Eleventy site runs the `eleventy-plugin-techdoc` i18n system with
`features.i18n: true` (`defaultLanguage: en`, `languages: [en, zh]`). The Simplified-Chinese content
under `website/src/zh/` is folded in and ships standard: `<html lang>`, a reciprocal `hreflang`
cluster (en/zh/x-default), per-locale canonicals, `og:locale`, a bidirectional language switcher,
translated nav/footer strings (`src/_data/i18n.json`), and all `/zh/` URLs in the sitemap. Remaining
is **content discipline, not plumbing**: zh pages are parallel files that can drift, so we need a
drift/staleness guard and native-speaker review before any locale is authoritative — and the same
wiring extended to ja and pt-BR. The product (diagnostic messages, CLI output) is still English-only.

**Target languages (ranked by community size × English-proficiency gap):**

1. **Chinese (Simplified)** — #1. Largest community, low English proficiency. (Partial content in `/zh`.)
2. **Japanese** — smaller community, low and *declining* English proficiency.
3. **Portuguese (Brazil)** — large, active Python scene (Python Brasil), low English proficiency.
4. **Spanish** — large aggregate market, moderate proficiency. **Stretch / after the top 3.**

**Scope (three surfaces — increasing difficulty):**

- **Website → top 3** (zh-Hans, ja, pt-BR). Fold the existing `/zh` content into the i18n system,
  *then* add ja and pt-BR.
- **Extension UI → top 3.** Investigate VS Code's `package.nls.<locale>.json` localization for command
  titles/settings descriptions; assess Zed/Neovim (likely limited — report findings).
- **LSP / CLI output → top 3 (the hard, high-value one).** Localize the **diagnostic messages and CLI
  text** from the Rust core. Touches every `BSK-E####`/`BSK-W####` message string, so it needs a
  message-catalog / localization layer (locale from config or `LANG`), keyed by diagnostic code.

Machine-translate each surface as a first pass, but **each locale needs native-speaker review before
go-live** — especially diagnostics, where wording precision matters.

---

## 12. Marketing & community {#NEXTSTEPS-MARKETING}

Existing assets: launch blog post (`website/src/blog/introducing-basilisk.md`), comparison/feature
docs, READMEs, OG image + logo. What's missing is *distribution* (mostly human-led — voice, accounts,
timing, relationships, budget):
- **Owned content**: polish the launch blog post; follow-up technical posts (Rust/Salsa architecture,
  strict-by-default philosophy, scale-benchmark numbers from §5, conformance-results listing).
- **Syndication**: X threads, Reddit (r/Python, r/rust), [dev.to](https://dev.to) cross-posts,
  Hacker News (Show HN). An agent drafts; a human owns posting, voice, timing.
- **Community outreach**: Python Discord/forums, Python Brasil, JP Python communities (ties into i18n).
- **Paid**: paid UGC / sponsorships — budget and vendor selection are human calls.

---

# Detailed TODO {#NEXTSTEPS-DETAILED-TODO}

> Legend: **`[AGENT]`** agent-drivable · **`[HUMAN]`** human discretion · **`[HYBRID]`** agent
> prepares, human approves/supplies credentials. Ordering within a group is rough priority.

## A. Shared release plumbing  *(do first)* {#NEXTSTEPS-RELEASE-PLUMBING}

- [ ] **`[AGENT]`** Replace `0.0.0-PLACEHOLDER` everywhere with a single git-tag-driven version source.
- [ ] **`[AGENT]`** Extend `release.yml` to publish per-platform binaries on tagged releases.
- [x] **`[AGENT]`** Neovim binary auto-download from GitHub releases + outdated-binary version check (shipped in `basilisk.nvim/lua/basilisk/binary.lua`).
- [ ] **`[HUMAN]`** Create/confirm publisher accounts and store CI secrets: VS Code Marketplace (VSCE_PAT), Open VSX (OVSX_PAT), Zed registry, (optional) luarocks.
- [ ] **`[HUMAN]`** Decide the versioning scheme (single repo-wide version vs. per-extension) and the release cadence.
- [ ] **`[AGENT]`** Website: if a version is displayed anywhere, source it dynamically from the latest GitHub release — never hardcode it in copy (hardcoded `v0.1` strings were removed 2026-05-30; `site.json` still carries `0.0.0-PLACEHOLDER`).

## B. Editor publishing {#NEXTSTEPS-EDITOR-PUBLISHING}

- [ ] **`[HYBRID]`** Add `vsce` package + VS Code Marketplace publish workflow (agent writes it; human holds the token).
- [ ] **`[HYBRID]`** Add `ovsx` publish workflow so Basilisk is installable in **Cursor & Windsurf**.
- [ ] **`[HYBRID]`** Add Zed extension-registry publish step.
- [ ] **`[AGENT]`** Verify/expand e2e + screenshot regression suites for nvim/zed/vsix before each first publish.
- [ ] **`[HUMAN]`** Clean-machine smoke test of each **published** artifact (install from marketplace, not dev build): diagnostics, hover, go-to-def, debug, profile.

## C. Debugging (DAP) verification {#NEXTSTEPS-DAP-VERIFICATION}

- [ ] **`[AGENT]`** Confirm DAP e2e tests cover breakpoint → step → inspect → continue in all three editors.
- [ ] **`[HUMAN]`** Manual debug session against a published artifact per editor; confirm graceful degradation when the editor DAP client is absent.

## D. Profiling smoothing {#NEXTSTEPS-PROFILING-SMOOTHING}

- [ ] **`[AGENT]`** Harden profiler e2e tests (start/stop/snapshot, Speedscope export integrity).
- [ ] **`[HUMAN]`** UX pass on inline visualization / heat map across editors; note rough edges (incl. macOS elevation prompt behaviour).

## E. Scale & resource testing {#NEXTSTEPS-SCALE-RESOURCE-TESTING}

- [ ] **`[AGENT]`** Build a reproducible benchmark harness that clones N large public Python repos and runs full + incremental checks (fixtures stay out of the repo).
- [ ] **`[AGENT]`** Capture objective metrics per repo: peak RSS, CPU time, full-check wall-clock, incremental latency p50/p95/p99, panic/crash/hang count.
- [ ] **`[AGENT]`** Run the same corpus through Pyright and produce a side-by-side results table.
- [ ] **`[HUMAN]`** Pick the canonical corpus and the headline numbers to publish.

## F. Typing-philosophy survey + proof {#NEXTSTEPS-TYPING-PHILOSOPHY-PROOF}

- [ ] **`[AGENT]`** Audit and document where the default severity is set; confirm no lenient default exists.
- [ ] **`[AGENT]`** Add/confirm e2e tests asserting (a) fresh project is strict by default, (b) each downgrade path (per-rule, per-path, `adoptions.toml`, `# type: ignore`) lowers severity.
- [ ] **`[AGENT]`** Write the short survey writeup ("strict AF? easy to down-shift?") linked to the named tests.

## G. Finish near-complete plans {#NEXTSTEPS-FINISH-NEAR-COMPLETE-PLANS}

- [ ] **`[AGENT]`** Fix `generics_syntax_scoping` docstring/line-scanning showstopper — re-ground the rule on the AST (`CHECK-ELIMINATE-FALSE-POSITIVES.md`).
- [ ] **`[AGENT]`** Clear the remaining 265 false positives (by making the checker smarter — every rule stays enabled; never by disabling a rule).
- [ ] **`[AGENT]`** Close the 78 failing PEP-conformance files (Protocols, Callables, TypeVarTuple, ParamSpec, TypedDicts).
- [ ] **`[AGENT]`** Finish `CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md` Phase 4 — wire the no-line-scanning lint into CI.
- [ ] **`[HUMAN]`** Decide whether `LSP-STUBBING-PLAN.md` Phase 5 (Salsa perf) ships now or later.

## H. Competitive parity (Pyright/Pylance) {#NEXTSTEPS-COMPETITIVE-PARITY-TODO}

- [ ] **`[HUMAN]`** Prioritize the parity gap list — which features actually move adoption.
- [ ] **`[AGENT]`** Audit editor UX features vs. Pylance (completions, signature help, semantic tokens, inlay hints, organize-imports, workspace symbols) and report gaps.
- [ ] **`[HYBRID]`** Drive toward the official Python conformance-results listing (agent does the conformance work; human handles the submission).

## I. MCP server (semantic codebase navigation) {#NEXTSTEPS-MCP-SERVER-TODO}

- [ ] **`[AGENT]`** Draft `docs/specs/MCP-SERVER-SPEC.md` + plan — tool surface, transport, how it reuses the existing LSP/workspace index.
- [ ] **`[AGENT]`** Implement the server (semantic symbol search, type-of, callers, protocol implementers, module/type-health summaries) with e2e tests.
- [ ] **`[HUMAN]`** Decide packaging/distribution (bundled in the binary vs. separate) and which agent ecosystems to target first.

## J. Deeper AI integration (nimblesite.ai) {#NEXTSTEPS-AI-INTEGRATION-TODO}

- [ ] **`[AGENT]`** Wire nimblesite.ai as a provider behind the existing model-agnostic AI-fix hooks (`LSP-AI-SPEC.md`), preserving the optional/pluggable contract.
- [ ] **`[AGENT]`** Add hermetic provider-boundary tests (mocked backend) plus an opt-in integration test against the real agent.
- [ ] **`[HUMAN]`** Provide nimblesite.ai credentials/config and decide default-on vs. opt-in.

## K. IntelliJ / PyCharm {#NEXTSTEPS-JETBRAINS-TODO}

- [ ] **`[HUMAN]`** Go/no-go decision on a JetBrains plugin now vs. deferring.
- [ ] **`[AGENT]`** *(if go)* Draft `docs/specs/JETBRAINS-SPEC.md` + `docs/plans/JETBRAINS-PLAN.md` mirroring the per-editor docs (LSP-first, LSP4IJ vs. custom client).

## L. Internationalization & translation {#NEXTSTEPS-I18N-TODO}

- [x] **`[AGENT]`** Build a real Eleventy i18n system; fold existing `/zh` content into it (replace the manual parallel pages). *(Done — `eleventy-plugin-techdoc` i18n with `features.i18n: true`; zh ships with standard hreflang/switcher/sitemap. See §11.)*
- [ ] **`[AGENT]`** Add a drift/staleness guard so a translated page that falls behind its English source is flagged (English is canonical; mark each locale page with the source commit/date it was synced from).
- [ ] **`[AGENT]`** Extend the existing i18n wiring to the next two languages (ja, pt-BR): add their `i18n.json` strings + `src/<lang>/` content. Machine-translate as a first pass.
- [ ] **`[AGENT]`** Investigate VSIX localization (`package.nls.<locale>.json`) and report what's feasible for Zed/Neovim; then translate command/setting strings into the top 3.
- [ ] **`[AGENT]`** Add a message-catalog / localization layer to the Rust core (locale from config/`LANG`, keyed by diagnostic code) and translate **LSP/CLI diagnostic output** into the top 3.
- [ ] **`[HUMAN]`** Native-speaker review of every locale before go-live — especially diagnostics (consider hiring reviewers).
- [ ] **`[HUMAN]`** Decide if/when Spanish (#4) gets added after the top 3.

## M. Marketing & community {#NEXTSTEPS-MARKETING-TODO}

- [ ] **`[AGENT]`** Polish the launch blog post; draft 2–3 follow-up technical posts (architecture, strict-by-default philosophy, scale-benchmark numbers, conformance milestone).
- [ ] **`[AGENT]`** Draft X threads, Reddit (r/Python, r/rust), dev.to, and Show HN posts for human review.
- [ ] **`[HUMAN]`** Own posting: accounts, brand voice, timing, and engagement.
- [ ] **`[HUMAN]`** Community outreach (Python Discord/forums, Python Brasil, JP communities) — ties into i18n.
- [ ] **`[HUMAN]`** Evaluate paid UGC / sponsorships — budget and vendor selection.
