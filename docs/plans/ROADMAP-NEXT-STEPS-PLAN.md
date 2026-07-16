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
  advise `cargo install basilisk-cli` ([NVIM-BINARY-UPGRADE-ASSETS]).
- [ ] **`[HYBRID]`** Confirm the Neovim mirror tag flow on a release and decide
  whether the mirror also needs GitHub Release objects; plugin managers consume
  tags directly.
- [ ] **`[HUMAN]`** Smoke-test diagnostics, navigation, debugging, profiling,
  formatting, and binary updates from published artifacts rather than dev builds.

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
  — non-conformance inference and flow-analysis improvements.
- [LSP-AI-PLAN.md](LSP-AI-PLAN.md),
  [COMPILER-ARCHITECTURE-PLAN.md](COMPILER-ARCHITECTURE-PLAN.md), and
  [CHECKER-ADVANCED-FEATURES-PLAN.md](CHECKER-ADVANCED-FEATURES-PLAN.md) — larger,
  optional product tracks.

## LSP follow-ups without a dedicated plan {#NEXTSTEPS-LSP}

- [ ] **`[AGENT]`** Add module-index depth control for large workspaces.
- [ ] **`[AGENT]`** Add `textDocument/implementation` and pytest-fixture
  completion/navigation only after their protocol behavior is specified.
- [ ] **`[AGENT]`** Decide with benchmark evidence whether routing one-shot CLI
  checks through Salsa is worthwhile; preserve byte-for-byte diagnostic parity.
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

- [ ] **`[AGENT]`** Specify an MCP server only when its tool surface and packaging
  are concrete; reuse the workspace index rather than duplicating analysis.
- [ ] **`[HYBRID]`** Implement a real AI provider behind the existing optional
  interface, with hermetic provider-boundary tests and user-supplied credentials.
- [ ] **`[AGENT]`** Add translation-drift detection before adding more locales.
- [ ] **`[HYBRID]`** Add Japanese and Brazilian Portuguese content, followed by
  native-speaker review before publication.
- [ ] **`[HUMAN]`** Own launch copy, community outreach, marketplace accounts,
  credentials, budgets, and publication timing.
