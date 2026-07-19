# Documentation index

## Contributing

| File | Purpose |
|---|---|
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Contribution workflow and human/agent responsibilities. |

## Specifications

Specifications document shipped contracts. Explicitly planned behavior is labelled and
linked to an active plan.

| File | Purpose |
|---|---|
| [Checker architecture](specs/CHECKER-ARCHITECTURE-SPEC.md) | Configuration, rules, diagnostics, analysis pipeline, CLI, and quality gates. |
| [Type inference](specs/CHECKER-TYPE-INFERENCE-SPEC.md) | Expression/type inference and narrowing contracts, plus the target bidirectional/constraint architecture and its research grounding. |
| [Stub resolution](specs/CHECKER-STUB-RESOLUTION-SPEC.md) | Pinned typing-spec import order, custom typeshed, archive acquisition, bundled stdlib ZIP, provenance, and generation. |
| [Checker MCP service](specs/CHECKER-MCP-SPEC.md) | Packaged stdio lifecycle and the structured typeshed provenance/status tool. |
| [Checker cache](specs/CHECKER-CACHE-SPEC.md) | Opt-in content-addressed CLI result cache. |
| [Rule tagging](specs/CHECKER-RULE-TAGGING-SPEC.md) | Rule provenance/category/free-form tags and conflict rules. |
| [Compiler prototype](specs/COMPILER-ARCHITECTURE-SPEC.md) | Current checker-gated AST interpreter and fixture contract. |
| [LSP architecture](specs/LSP-ARCHITECTURE-SPEC.md) | Shared server protocol, analysis, commands, and capabilities. |
| [Configuration editor](specs/LSP-CONFIGURATION-EDITOR-SPEC.md) | Typed preview/apply configuration transaction and VS Code shell. |
| [Analysis modes](specs/LSP-ANALYSIS-MODES-SPEC.md) | Open-file, whole-module, and cross-module analysis. |
| [Formatting](specs/LSP-FORMATTING-SPEC.md) | Embedded Ruff formatter and native import hygiene. |
| [AI typing hook](specs/LSP-AI-SPEC.md) | Unused provider interface and no-op implementation. |
| [Debug integration](specs/LSP-DEBUG-INTEGRATION-SPEC.md) | debugpy session lifecycle and editor DAP integration. |
| [Profiling](specs/LSP-PROFILING-SPEC.md) | CPU sampling, exports, process UI, and debug-session memory inspection. |
| [Mass autofix](specs/LSP-MASS-AUTOFIX-SPEC.md) | Batch edits and active-configuration adoption commands. |
| [uv integration](specs/LSP-UV-INTEGRATION-SPEC.md) | Project detection, lock registry, commands, and known wiring limits. |
| [Refactoring](specs/LSP-REFACTORING-SPEC.md) | Deterministic rename/extract/inline/move/convert actions. |
| [Test integration](specs/LSP-TEST-INTEGRATION-SPEC.md) | Test discovery, execution, debug, and coverage protocol. |
| [Activity panel](specs/EXTENSION-ACTIVITY-PANEL-SPEC.md) | Module/health wire data and shipped VS Code views. |
| [VS Code extension](specs/VSIX-SPEC.md) | VS Code client behavior. |
| [Real-world e2e suites](specs/VSIX-REAL-WORLD-SPEC.md) | Pinned real-repo journeys with memory/CPU budgets. |
| [Neovim extension](specs/NEOVIM-SPEC.md) | `basilisk.nvim` client behavior. |
| [Zed extension](specs/ZED-SPEC.md) | Zed WASM client behavior. |
| [Editor screenshots](specs/VSIX-EDITOR-SCREENSHOTS-SPEC.md) | Automated real VS Code screenshots. |
| [Website E2E](specs/WEBSITE-E2E-SPEC.md) | Navigation and responsive smoke tests. |
| [Website screenshots](specs/WEBSITE-SCREENSHOTS-SPEC.md) | Verified CLI screenshot generation. |
| [Website error pages](specs/WEBSITE-ERROR-PAGES-SPEC.md) | Generated per-diagnostic documentation. |

## Active plans

Plans contain only unfinished work. Delete a plan when its acceptance gate passes.

| File | Remaining scope |
|---|---|
| [Roadmap](plans/ROADMAP-NEXT-STEPS-PLAN.md) | Distribution follow-ups, scale, ecosystem, and links to focused plans. |
| [Specification conformance audit](plans/SPEC-CONFORMANCE-AUDIT-PLAN.md) | Confirmed implementation/spec deviations. |
| [Configuration editor](plans/LSP-CONFIGURATION-EDITOR-PLAN.md) | Buffer safety, protocol E2E coverage, provenance/metadata, cross-editor clients, and release evidence. |
| [Formatting](plans/LSP-FORMATTING-PLAN.md) | VS Code default-formatter opt-in and published-artifact verification. |
| [AI-assisted LSP](plans/LSP-AI-PLAN.md) | First opt-in provider slice and privacy/safety gate. |
| [Activity panel](plans/EXTENSION-ACTIVITY-PANEL-PLAN.md) | Settings wiring and remaining cross-editor/test quality. |
| [Type narrowing and inference](plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md) | Bidirectional/constraint-based inference engine, flow analysis, shared subtyping, and PEP 827 readiness. |
| [Runtime typeshed acquisition](plans/CHECKER-TYPESHED-RUNTIME-PLAN.md) | Acquisition, source precedence, startup gating, and #288/#289 hovers all shipped; only the per-artifact licensing gate remains (composite LICENSE, conditional NOTICE files, and modified-file marks verified inside every binary, wheel, and VSIX). |
| [Eliminate line scanning](plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md) | Replace remaining raw-source rule scans with AST data. |
| [Advanced checker features](plans/CHECKER-ADVANCED-FEATURES-PLAN.md) | Mojo checks, plugin host, migration, and CI helpers. |
| [Native compiler](plans/COMPILER-ARCHITECTURE-PLAN.md) | HIR, backend, runtime, interop, CLI, and native acceptance. |
