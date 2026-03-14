# Documentation Index

## Specs

Specifications define the target behavior and architecture. They are the source of truth.

| File | Description |
|---|---|
| [CHECKER-ARCHITECTURE-SPEC.md](specs/CHECKER-ARCHITECTURE-SPEC.md) | Core architecture of the Basilisk type checker — type system, diagnostic codes, error ranges, testing strategy, and design philosophy. The foundational spec that all other specs reference. |
| [CHECKER-TYPE-INFERENCE-SPEC.md](specs/CHECKER-TYPE-INFERENCE-SPEC.md) | Bidirectional type inference engine — variable inference, collection inference, generic TypeVar solving, type narrowing, and the redundant annotation principle (W0050). |
| [CHECKER-WHOLE-MODULE-ANALYSIS-SPEC.md](specs/CHECKER-WHOLE-MODULE-ANALYSIS-SPEC.md) | Analysis modes (openFilesOnly, wholeModule, crossModule) governing which files are analyzed and how symbol graphs are shared across module boundaries. |
| [COMPILER-ARCHITECTURE-SPEC.md](specs/COMPILER-ARCHITECTURE-SPEC.md) | Python-to-native compiler targeting a compiled subset of Python via LLVM. Covers ownership model, memory backends, GPU support, and PEP-compliant compilation. |
| [LSP-ARCHITECTURE-SPEC.md](specs/LSP-ARCHITECTURE-SPEC.md) | Single source of truth for all LSP features, DAP integration, custom commands, configuration settings, binary resolution, and stub strategy. Editor-specific specs point back to this. |
| [LSP-AI-SPEC.md](specs/LSP-AI-SPEC.md) | Model-agnostic AI layer enhancing the LSP with AI-powered fixes, completions, refactoring, and dead code detection. Optional — deterministic features always work without it. |
| [LSP-DEBUG-INTEGRATION-SPEC.md](specs/LSP-DEBUG-INTEGRATION-SPEC.md) | Embedded debugpy architecture — the Basilisk binary serves as both language server and debug adapter via DAP over TCP. |
| [LSP-PROFILING-SPEC.md](specs/LSP-PROFILING-SPEC.md) | Embedded Python profiler using py-spy with Speedscope output and inline editor visualization. No pip install required. |
| [LSP-MASS-AUTOFIX-SPEC.md](specs/LSP-MASS-AUTOFIX-SPEC.md) | Batch application of safe type-related fixes across files/workspaces and gradual adoption mode that demotes errors to warnings per-file. |
| [LSP-UV-INTEGRATION-SPEC.md](specs/LSP-UV-INTEGRATION-SPEC.md) | Zero-config uv workspace detection, lock file parsing, package intelligence, and dependency-aware stub resolution. |
| [VSIX-SPEC.md](specs/VSIX-SPEC.md) | VS Code extension — language intelligence, debugging, profiling, and test explorer via the Basilisk LSP binary. |
| [NEOVIM-SPEC.md](specs/NEOVIM-SPEC.md) | Neovim plugin (basilisk.nvim) — LSP client setup, DAP proxy, command mappings, and Neovim-specific configuration. |
| [ZED-SPEC.md](specs/ZED-SPEC.md) | Zed extension built in WASM — LSP integration, tree-sitter grammars, DAP debugging, and slash commands. |

## Plans

Plans are implementation roadmaps. They track phasing, priorities, and progress toward spec targets.

| File | Description |
|---|---|
| [LSP-PLAN.md](plans/LSP-PLAN.md) | Overall LSP implementation roadmap with seven phases covering core features through cross-module analysis. |
| [LSP-AI-PLAN.md](plans/LSP-AI-PLAN.md) | Implementation plan for the AI provider abstraction — model-agnostic hooks into the LSP for fixes, completions, and refactoring. |
| [LSP-PROFILING-PLAN.md](plans/LSP-PROFILING-PLAN.md) | Plan to embed py-spy profiler directly into the LSP for CPU profiling and hotspot visualization. |
| [LSP-MASS-AUTOFIX-PLAN.md](plans/LSP-MASS-AUTOFIX-PLAN.md) | Phased rollout of batch autofix and gradual adoption mode across the workspace. |
| [LSP-UV-INTEGRATION-PLAN.md](plans/LSP-UV-INTEGRATION-PLAN.md) | Strategy for integrating uv project detection, lock file parsing, and dependency intelligence. |
| [CHECKER-CROSS-MODULE-ANALYSIS-PLAN.md](plans/CHECKER-CROSS-MODULE-ANALYSIS-PLAN.md) | Cross-module and whole-workspace type analysis with PEP 561 stub resolution and import graph infrastructure. |
| [ZED-PLAN.md](plans/ZED-PLAN.md) | Phased implementation of the Zed extension including LSP scaffolding, tree-sitter queries, and DAP support. |

## Standalone

| File | Description |
|---|---|
| [CHECKER-PEP-CONFORMANCE.md](CHECKER-PEP-CONFORMANCE.md) | Tracks Basilisk's accuracy against the official python/typing conformance test suite. Target: 100%. |
