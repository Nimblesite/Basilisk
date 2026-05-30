# Documentation Index

## Specs

Specifications define the target behavior and architecture. They are the source of truth.

| File | Description |
|---|---|
| [CHECKER-ARCHITECTURE-SPEC.md](specs/CHECKER-ARCHITECTURE-SPEC.md) | Core type checker architecture — type system, diagnostic codes, error ranges, and design philosophy. |
| [CHECKER-TYPE-INFERENCE-SPEC.md](specs/CHECKER-TYPE-INFERENCE-SPEC.md) | Bidirectional type inference — variable/collection/generic inference, type narrowing, redundant annotation principle (W0050). |
| [CHECKER-STUB-RESOLUTION-SPEC.md](specs/CHECKER-STUB-RESOLUTION-SPEC.md) | PEP 561 stub resolution, typeshed bundling, type provenance tracking, suppression system, auto-stub generation. |
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

## Plans

Implementation roadmaps tracking phasing, priorities, and progress.

| File | Description |
|---|---|
| [ROADMAP-NEXT-STEPS-PLAN.md](plans/ROADMAP-NEXT-STEPS-PLAN.md) | Post-launch aggregation roadmap — editor releases, scale testing, i18n, MCP server, AI integration, marketing. Rough overview + agent/human-split TODO. |
| [LSP-PLAN.md](plans/LSP-PLAN.md) | Overall LSP roadmap — seven phases from core features through cross-module analysis. |
| [CHECKER-CROSS-MODULE-PLAN.md](plans/CHECKER-CROSS-MODULE-PLAN.md) | Cross-file LSP features, type provenance, Salsa integration, auto-stub generation. |
| [CHECKER-PEP-CONFORMANCE-PLAN.md](plans/CHECKER-PEP-CONFORMANCE-PLAN.md) | PEP conformance push — target 85%, tiered task list by complexity and impact. |
| [LSP-AI-PLAN.md](plans/LSP-AI-PLAN.md) | AI provider abstraction — model-agnostic hooks for fixes, completions, refactoring. |
| [LSP-PROFILING-PLAN.md](plans/LSP-PROFILING-PLAN.md) | Embed py-spy profiler into LSP for CPU profiling and hotspot visualization. |
| [LSP-UV-INTEGRATION-PLAN.md](plans/LSP-UV-INTEGRATION-PLAN.md) | uv project detection, lock file parsing, dependency intelligence. |
| [CHECK-ELIMINATE-FALSE-POSITIVES.md](plans/CHECK-ELIMINATE-FALSE-POSITIVES.md) | Eliminate conformance suite false positives — rule-specific fixes and engine work. |
| [CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md](plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md) | Type narrowing and full inference engine — NarrowingEngine, expression inference, ConstraintSolver, class hierarchy subtyping. |
| [ZED-PLAN.md](plans/ZED-PLAN.md) | Zed extension — LSP scaffolding, tree-sitter queries, DAP support. |
| [LSP-TEST-INTEGRATION-PLAN.md](plans/LSP-TEST-INTEGRATION-PLAN.md) | Test integration rollout — Rust library, LSP protocol, VS Code/Neovim/Zed integration, coverage. |
| [EXTENSION-ACTIVITY-PANEL-PLAN.md](plans/EXTENSION-ACTIVITY-PANEL-PLAN.md) | Activity panel rollout — LSP backend, VS Code panels, Zed slash commands, Neovim buffers. |
