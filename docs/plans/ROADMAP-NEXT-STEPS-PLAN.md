# Basilisk Roadmap {#NEXTSTEPS-ROADMAP}

This file contains only cross-cutting work that does not belong to a focused
implementation plan. Specs remain authoritative for behavior; focused plans own
the engineering detail.

Current baseline:

- The unmodified `python/typing` harness passes 141/141 files with zero missed
  errors and zero false positives. Conformance is now a permanent ratchet, not a
  project plan.
- Tagged-release automation builds the binaries and editor artifacts, stamps
  placeholder versions, publishes VSIX packages to the Microsoft and Open VSX
  registries, and refreshes the Neovim and Zed mirror repositories.
- VS Code, Neovim, and Zed share the Rust LSP. Editor-specific manual publication
  and clean-install validation remain.

## Coverage beyond the upstream suite {#NEXTSTEPS-BEYOND-CONFORMANCE}

Conformance remains the prime directive and both ratchets stand. But a batch of
user-reported typing puzzles (2026-08-01, issues
[#378](https://github.com/Nimblesite/Basilisk/issues/378)–[#383](https://github.com/Nimblesite/Basilisk/issues/383),
[#371](https://github.com/Nimblesite/Basilisk/issues/371)) established something
we should hold onto: **every one of those defects coexisted with a clean 141/141
run**, and each reproduced on the CLI as well as the playground.

The suite is a floor, not a ceiling. Two concrete blind spots it does not cover:

- `conformance/tests/aliases_recursive.py` contains **zero** PEP 695 `type`
  statements — every recursive case upstream uses the legacy spelling — so
  rejecting every non-generic recursive `type` alias scored 100%.
- Nothing upstream pins "return a `str` literal from a function annotated with
  an alias-of-`int`", so skipping assignability for every nominal annotation
  scored 100%.

Neither is an upstream flaw to route around: a syntax the suite omits is *our*
responsibility to cover.

- [ ] **`[AGENT]`** Own a Basilisk-side regression suite for constructs the
  upstream suite omits, starting with a PEP 695 `type`-statement counterpart of
  every recursive case in `aliases_recursive.py`. It grows whenever a
  user-reported defect turns out to be uncovered upstream; it never substitutes
  for the live harness.
- [ ] **`[AGENT]`** For each user-reported defect, record whether the upstream
  suite covered the construct. A "no" is a coverage-gap ticket in its own right,
  not just a bug fix.
- [ ] **`[AGENT]`** Treat "an entire rule family went silent" as its own defect
  class. Both [#380](https://github.com/Nimblesite/Basilisk/issues/380) (aliased
  decorator) and [#381](https://github.com/Nimblesite/Basilisk/issues/381)
  (call not in outermost position) disable real rules with no signal, which no
  pass-percentage metric can surface.

## Distribution follow-ups {#NEXTSTEPS-DISTRIBUTION}

- [ ] **`[HYBRID]`** Validate a tagged release end to end with the real
  `VSCODE_MARKETPLACE_PAT`, `OPEN_VSX_PAT`, and mirror credentials, then install
  each published artifact on a clean machine.
- [ ] **`[HYBRID]`** Submit the standalone `Nimblesite/basilisk-zed` mirror to
  `zed-industries/extensions`; automation already renders and tests that mirror.
- [ ] **`[HUMAN]`** Submit the prepared
  `basilisk.nvim/lspconfig/basilisk.lua` definition upstream.
- [ ] **`[HYBRID]`** Submit `basilisk` to the upstream
  [mason-registry](https://github.com/mason-org/mason-registry) so
  `:MasonInstall basilisk` / `:MasonUpdate` work; the release assets it needs
  already exist ([NVIM-BINARY-UPGRADE]). Do not document Mason support until
  the registry entry is merged.
- [ ] **`[HUMAN]`** Decide whether `release.yml` should add an
  `x86_64-apple-darwin` build if Intel-mac demand appears; until then
  `platform_asset_name()` deliberately returns `nil` there and the flows
  advise a from-source build, `cargo install --git
  https://github.com/Nimblesite/Basilisk basilisk-cli`
  ([NVIM-BINARY-UPGRADE-ASSETS]).
- [ ] **`[HYBRID]`** Confirm the Neovim mirror tag flow on a release and decide
  whether the mirror also needs GitHub Release objects; plugin managers consume
  tags directly.
- [ ] **`[HUMAN]`** Smoke-test diagnostics, navigation, debugging, profiling,
  formatting, and binary updates from published artifacts rather than dev builds.
- [ ] **`[HUMAN]`** Confirm `BREW_SCOOP_PAT` can write to the
  `Nimblesite/basilisk.nvim` mirror, dry-run the `publish-nvim` job on the next
  tag, and install the plugin via `vim.pack.add()` on a clean machine
  ([#197](https://github.com/Nimblesite/Basilisk/issues/197),
  [NVIM-DISTRIBUTION-RELEASE]). The job itself is shipped; only the
  credential check and the live rehearsal remain.
- [ ] **`[LATER]`** Decide whether to add the optional Neovim secondary
  channels — a LuaRocks package and the `nvim-lspconfig` upstream PR
  ([#197](https://github.com/Nimblesite/Basilisk/issues/197),
  [NVIM-DISTRIBUTION-SECONDARY-LSPCONFIG-PR]).

### Shipwright deployment contract {#NEXTSTEPS-SHIPWRIGHT}

Conformity work against the Shipwright contract
([#86](https://github.com/Nimblesite/Basilisk/issues/86) is the parent
checklist). These are hardening items with no active exploit; run the
`shipwright-compliance` skill for the full per-channel audit.

- [ ] **`[AGENT]`** Bundle the server binary in the VSIX and verify it before
  startup instead of continuing on a missing or mismatched binary
  ([#87](https://github.com/Nimblesite/Basilisk/issues/87), `[SWR-IDE-RESOLUTION]`,
  `[SWR-VSIX-VERIFY]`).
- [ ] **`[AGENT]`** Declare the profiler helper as a component in the product
  `shipwright.json` ([#88](https://github.com/Nimblesite/Basilisk/issues/88),
  `[SWR-ARCH-LIBRARIES]`).
- [ ] **`[AGENT]`** Make the Zed extension read `shipwright.json`, resolve
  `expectedVersion` for the `basilisk` component, and compare it against
  `InitializeResult.serverInfo.version` — Zed cannot preflight `--version`, so
  the handshake is the only verification point. On mismatch, surface expected,
  found, and path, and refuse the server; keep the existing
  `is_newer_version` GitHub-release check as a separate informational warning
  ([#35](https://github.com/Nimblesite/Basilisk/issues/35),
  [#89](https://github.com/Nimblesite/Basilisk/issues/89), `[SWR-VERSION-LSP]`,
  `[SWR-IDE-ERROR]`, `[SWR-COMPAT]`).
- [ ] **`[AGENT]`** Supply-chain controls, all from
  [#86](https://github.com/Nimblesite/Basilisk/issues/86): SHA-pin every actions
  `uses:` plus `.github/dependabot.yml` (`[SWR-SEC-ACTION-PINNING]`); top-level
  `permissions: contents: read` with per-job escalation
  (`[SWR-SEC-TOKEN-PRIVILEGE]`); frozen installs (`[SWR-SEC-FROZEN-INSTALL]`);
  build provenance and a CycloneDX SBOM per artifact (`[SWR-SEC-PROVENANCE]`,
  `[SWR-SEC-SBOM]`); one cosign-signed `SHA256SUMS` replacing the bare per-asset
  `.sha256` files, verified by every downloader before execution
  (`[SWR-SEC-CHECKSUM]`); OIDC trusted publishing in place of long-lived
  registry tokens (`[SWR-SEC-OIDC-PUBLISH]`).
- [ ] **`[HUMAN]`** Developer ID sign, notarize, and staple both macOS binaries —
  `basilisk` and the profiler helper — keeping cosign provenance alongside
  ([#90](https://github.com/Nimblesite/Basilisk/issues/90),
  `[SWR-SIGN-APPLE-WORKFLOW]`).
- [ ] **`[HUMAN]`** Windows Authenticode remains deliberately unsolved: a fresh
  certificate has no SmartScreen reputation and reputation cannot be bought.
  Until Azure Trusted Signing is evaluated, Windows users install via Scoop or
  Homebrew and every Windows binary ships cosign provenance. Revisit rather than
  forget ([#91](https://github.com/Nimblesite/Basilisk/issues/91),
  `[SWR-SIGN-WINDOWS]`).

### Permissive-only license footprint {#NEXTSTEPS-LICENSES}

Hygiene, not a publishing blocker — the shipped `NOTICES` file is already
legally sufficient ([#48](https://github.com/Nimblesite/Basilisk/issues/48)).
The goal is a runtime tree with no weak copyleft at all, so downstream
packagers and vendoring users have nothing to reason about.

- [ ] **`[AGENT]`** Replace `colored` (MPL-2.0, workspace dep) with
  `nu-ansi-term` or `owo-colors`; audit every `use colored` site in
  `basilisk-cli`.
- [ ] **`[AGENT]`** Remove `inferno` (CDDL-1.0, direct dep in `basilisk-lsp` and
  transitive via `py-spy`) — either drop flamegraph SVG emission, find a
  maintained permissive replacement, or clean-room the SVG writer. Confirm the
  `py-spy` transitive path is prunable by feature flag.
- [ ] **`[AGENT]`** Gate it: `cargo tree` filtered for `CDDL|MPL|GPL|LGPL` must
  come back empty, then delete those `NOTICES` sections and update the
  third-party audit doc.

## Focused engineering plans {#NEXTSTEPS-ENGINEERING}

These plans are active and should be removed when their acceptance criteria are
complete:

- [LSP-CONFIGURATION-EDITOR-PLAN.md](LSP-CONFIGURATION-EDITOR-PLAN.md) — typed,
  transactional rule configuration and the thin VS Code editor.
- [LSP-FORMATTING-PLAN.md](LSP-FORMATTING-PLAN.md) — the VS Code
  default-formatter opt-in and published-artifact verification.
- [EXTENSION-ACTIVITY-PANEL-PLAN.md](EXTENSION-ACTIVITY-PANEL-PLAN.md) — real
  server-side feature toggles plus remaining accessibility and performance work.
- [CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md](CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md)
  — replace Python-structure reconstruction from raw lines with Ruff AST data.
- [CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md](CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md)
  — the bidirectional/constraint-based inference-engine upgrade (fixes
  inference-gap issues like #317, PEP 827 readiness), flow analysis, and
  shared subtyping.
- [LSP-AI-PLAN.md](LSP-AI-PLAN.md) and
  [CHECKER-ADVANCED-FEATURES-PLAN.md](CHECKER-ADVANCED-FEATURES-PLAN.md) — larger,
  optional product tracks.

## LSP follow-ups without a dedicated plan {#NEXTSTEPS-LSP}

- [ ] **`[AGENT]`** Add module-index depth control for large workspaces.
- [ ] **`[AGENT]`** Add `textDocument/implementation` and pytest-fixture
  completion/navigation only after their protocol behavior is specified.
- [ ] **`[AGENT]`** Decide with benchmark evidence whether routing one-shot CLI
  checks through Salsa is worthwhile; preserve byte-for-byte diagnostic parity.
- [ ] **`[AGENT]`** Make the server trace controllable instead of
  firehose-or-nothing ([#85](https://github.com/Nimblesite/Basilisk/issues/85)).
  Today the only knob is the stock `basilisk.trace.server`, which governs LSP
  message tracing and not the server's own `tracing` output. Needed: a
  `basilisk.log.level` setting applied to the subscriber; `EnvFilter`-style
  per-category mutes (`diagnostics`, `commands`, `profiler`, `workspace-scan`,
  `dap`); a summary line rather than a full `publishDiagnostics` payload at
  `info`, with the dump behind `debug`/`trace`; and change-only or `trace`-level
  logging for the ~2 s `basilisk.profiler.processes` poll. Serializing megabytes
  of JSON per keystroke is a plausible CPU cost, so measure before and after.
- [ ] **`[AGENT]`** Decide whether the persistent result cache should serve the
  editor ([#367](https://github.com/Nimblesite/Basilisk/issues/367)). `cache =
  true` is CLI-only today, yet the VS Code configuration editor presents it
  under a *Caching* panel, so checking the box changes nothing an editor does
  and `.basilisk/` is never created. A cold LSP start is exactly when a
  cross-process cache pays off — every input is byte-identical to the last run.
  Either wire the LSP through `basilisk-db` or stop offering the setting in the
  editor; documenting the surprise is not a resolution.
- [ ] **`[AGENT]`** Salsa v2 incremental computation
  ([#198](https://github.com/Nimblesite/Basilisk/issues/198)), deferred until the
  `DashMap + Arc` stubbing layer and the content-addressed CLI cache were stable
  — both now are. Scope: add `salsa` to `basilisk-lsp`, define the database in
  `salsa_db.rs`, migrate parse → resolve → check to `#[salsa::tracked]` queries
  with cross-file invalidation ([STUBRES-*] Phase 5), then replace lazy
  dependency re-verification with watcher-driven sub-file invalidation once
  `basilisk-db` is Salsa-backed ([CHKCACHE-*]). Target: the sub-10 ms
  incremental check in the architecture spec.
- [ ] **`[AGENT]`** Investigate finer-than-module incremental granularity only if
  profiling identifies module-level recomputation as a bottleneck.
- [ ] **`[AGENT]`** Specify notebook/cell semantics before adding Jupyter support.

## Scale evidence {#NEXTSTEPS-SCALE}

- [ ] **`[AGENT]`** Build a reproducible harness that clones an approved corpus
  of public Python repositories outside this repository.
- [ ] **`[AGENT]`** Record peak RSS, CPU time, cold full-check time, incremental
  p50/p95/p99 latency, and crash/hang counts with machine and tool versions.
- [ ] **`[AGENT]`** Run the same corpus through current competitor releases using
  equivalent settings and publish the methodology with the results.
- [ ] **`[HUMAN]`** Approve the corpus and any comparative claims before they are
  used on the website.

## Ecosystem expansion {#NEXTSTEPS-ECOSYSTEM}

- [ ] **`[AGENT]`** Specify any future MCP analysis tools before widening the
  shipped status-only [checker MCP service](../specs/CHECKER-MCP-SPEC.md); reuse
  the workspace index rather than duplicating analysis.
- [ ] **`[HYBRID]`** Implement a real AI provider behind the existing optional
  interface, with hermetic provider-boundary tests and user-supplied credentials.
- [ ] **`[HUMAN]`** Decide yes or no on Cython
  ([#291](https://github.com/Nimblesite/Basilisk/issues/291)). `.pyx` is a
  different language with its own grammar and C-level type declarations, so this
  is a new front end, not a rule — the answer belongs on the record either way
  rather than sitting unanswered in the tracker.
- [ ] **`[AGENT]`** Add translation-drift detection before adding more locales.
- [ ] **`[HYBRID]`** Add Japanese and Brazilian Portuguese content, followed by
  native-speaker review before publication.
- [ ] **`[HUMAN]`** Own launch copy, community outreach, marketplace accounts,
  credentials, budgets, and publication timing.
