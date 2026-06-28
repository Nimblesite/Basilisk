# Documentation Index

## Contributing

How the human + AI partnership divides the work — what humans own (testing, code-quality
review, conformance/security audits, parity, AI-instruction tuning) and what agents drive.

| File | Description |
|---|---|
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Contribution guide, split into **For Humans** (judgment, taste, trust) and **For AI** (technical execution under the `CLAUDE.md` rules). |

## Specs

Specifications define the target behavior and architecture. They are the source of truth.

| File | Description |
|---|---|
| [CHECKER-ARCHITECTURE-SPEC.md](specs/CHECKER-ARCHITECTURE-SPEC.md) | Core type checker architecture — type system, diagnostic codes, error ranges, and design philosophy. |
| [CHECKER-TYPE-INFERENCE-SPEC.md](specs/CHECKER-TYPE-INFERENCE-SPEC.md) | Bidirectional type inference — variable/collection/generic inference, type narrowing, redundant annotation principle (W0050). |
| [CHECKER-STUB-RESOLUTION-SPEC.md](specs/CHECKER-STUB-RESOLUTION-SPEC.md) | PEP 561 stub resolution, typeshed bundling, type provenance tracking, suppression system, auto-stub generation. |
| [CHECKER-CACHE-SPEC.md](specs/CHECKER-CACHE-SPEC.md) | Opt-in, content-addressed CLI result cache — correctness contract (never miss an error), read-set fingerprinting, warm/cold detection. |
| [CHECKER-RULE-TAGGING-SPEC.md](specs/CHECKER-RULE-TAGGING-SPEC.md) | Rule tagging: provenance (pep/basilisk) + PEP-category + free-form tags; conflict rules. |
| [COMPILER-ARCHITECTURE-SPEC.md](specs/COMPILER-ARCHITECTURE-SPEC.md) | Python-to-native compiler via LLVM — ownership model, memory backends, GPU support. |
| [LSP-ARCHITECTURE-SPEC.md](specs/LSP-ARCHITECTURE-SPEC.md) | Single source of truth for LSP features, DAP integration, custom commands, configuration, and binary resolution. |
| [LSP-ANALYSIS-MODES-SPEC.md](specs/LSP-ANALYSIS-MODES-SPEC.md) | Analysis modes (openFilesOnly, wholeModule, crossModule), workspace index, import graph, cross-file LSP features. |
| [LSP-AI-SPEC.md](specs/LSP-AI-SPEC.md) | Model-agnostic AI layer — AI-powered fixes, completions, refactoring. Optional; deterministic features work without it. |
| [LSP-DEBUG-INTEGRATION-SPEC.md](specs/LSP-DEBUG-INTEGRATION-SPEC.md) | Embedded debugpy — Basilisk binary serves as both language server and debug adapter via DAP over TCP. |
| [LSP-PROFILING-SPEC.md](specs/LSP-PROFILING-SPEC.md) | Embedded Python profiler using py-spy with Speedscope output and inline editor visualization. |
| [LSP-MASS-AUTOFIX-SPEC.md](specs/LSP-MASS-AUTOFIX-SPEC.md) | Batch autofix across files/workspaces and gradual adoption mode (errors → warnings per-file). |
| [LSP-UV-INTEGRATION-SPEC.md](specs/LSP-UV-INTEGRATION-SPEC.md) | Zero-config uv workspace detection, lock file parsing, package intelligence, stub resolution. |
| [VSIX-SPEC.md](specs/VSIX-SPEC.md) | VS Code extension — language intelligence, debugging, profiling, test explorer. |
| [NEOVIM-SPEC.md](specs/NEOVIM-SPEC.md) | Neovim plugin (basilisk.nvim) — LSP client, DAP proxy, command mappings. |
| [LSP-REFACTORING-SPEC.md](specs/LSP-REFACTORING-SPEC.md) | Deterministic refactoring tools — rename, extract, inline, move, convert, change signature. |
| [ZED-SPEC.md](specs/ZED-SPEC.md) | Zed extension (WASM) — LSP integration, tree-sitter grammars, DAP debugging. |
| [LSP-TEST-INTEGRATION-SPEC.md](specs/LSP-TEST-INTEGRATION-SPEC.md) | Test discovery, execution, and editor integration — pytest/unittest, TestItem model, coverage overlay. |
| [EXTENSION-ACTIVITY-PANEL-SPEC.md](specs/EXTENSION-ACTIVITY-PANEL-SPEC.md) | Cross-editor activity panel — module explorer, type health, feature dashboard (VS Code, Zed, Neovim). |
| [WEBSITE-E2E-SPEC.md](specs/WEBSITE-E2E-SPEC.md) | Website navigation/e2e smoke tests (Playwright, desktop + mobile) — top-nav resolution, docs sidebar, and the mobile docs-submenu reachability guard. |
| [WEBSITE-SCREENSHOTS-SPEC.md](specs/WEBSITE-SCREENSHOTS-SPEC.md) | Automated CLI screenshots — `npm run screenshots` runs the real binary on each documented snippet, self-verifies the diagnostic fires, and renders it in a faithful Terminal window (no manual screencapture). |
| [WEBSITE-ERROR-PAGES-SPEC.md](specs/WEBSITE-ERROR-PAGES-SPEC.md) | Per-diagnostic `/errors/BSK-XXXX/` pages generated from the checker source so every CLI `see:` link resolves — with severity, explanation, worked example, drift guard, and render verification. |
| [VSIX-EDITOR-SCREENSHOTS-SPEC.md](specs/VSIX-EDITOR-SCREENSHOTS-SPEC.md) | Automated real VS Code editor screenshots (`npm run screenshots:editor`) — drives the extension headed, captures the window over CDP (no Playwright dep), embeds diagnostics/hover/quick-fix/activity-panel on the docs. |

## Plans

Implementation roadmaps tracking phasing, priorities, and progress.

| File | Description |
|---|---|
| [ROADMAP-NEXT-STEPS-PLAN.md](plans/ROADMAP-NEXT-STEPS-PLAN.md) | Post-launch roadmap — editor releases, scale testing, i18n, MCP server, AI integration, marketing. Agent/human task split. |
| [LSP-PLAN.md](plans/LSP-PLAN.md) | Overall LSP roadmap — phases from core features through cross-module analysis and PEP conformance. |
| [CHECKER-PEP-CONFORMANCE-PLAN.md](plans/CHECKER-PEP-CONFORMANCE-PLAN.md) | PEP conformance push toward 100% — tiered task list by complexity and impact. |
| [CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md](plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md) | Type narrowing and inference engine — narrowing engine, expression inference, constraint solver, class-hierarchy subtyping. |
| [CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md](plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md) | Replace raw `source.lines()` scanning in rules with AST-driven checks; phased by severity, with a regression-guard lint. |
| [CHECK-ELIMINATE-FALSE-POSITIVES.md](plans/CHECK-ELIMINATE-FALSE-POSITIVES.md) | Eliminate conformance-suite false positives — rule-specific fixes and engine work. |
| [FP-REMAINING-NOTES.md](plans/FP-REMAINING-NOTES.md) | Working notes on remaining conformance false positives and their root causes. |
| [LSP-AI-PLAN.md](plans/LSP-AI-PLAN.md) | AI provider abstraction — model-agnostic hooks for fixes, completions, refactoring (interface + no-op default shipped). |
| [EXTENSION-ACTIVITY-PANEL-PLAN.md](plans/EXTENSION-ACTIVITY-PANEL-PLAN.md) | Activity panel rollout — LSP backend, VS Code panels, Zed slash commands, Neovim buffers. |
