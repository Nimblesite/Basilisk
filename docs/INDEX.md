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
| [Withdrawal messaging](specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md) | **Single source of truth for everything Basilisk says publicly** — the message, the approved copy, what is never said, the unlisting scope, and the inert CLI contract. Every README, listing, and website page copies from it. |
| [Checker architecture](specs/CHECKER-ARCHITECTURE-SPEC.md) | Configuration, rules, diagnostics, analysis pipeline, CLI, and quality gates — including [CHKARCH-TEXT-MATCHED-LOGIC](specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TEXT-MATCHED-LOGIC), the failing-test → delete → report rule that governs any code deciding from source text. |
| [Type inference](specs/CHECKER-TYPE-INFERENCE-SPEC.md) | The bidirectional/constraint inference engine — the checker's single type oracle — its narrowing contracts, research grounding, and the condemned legacy mechanisms under demolition. |
| [Stub resolution](specs/CHECKER-STUB-RESOLUTION-SPEC.md) | Pinned typing-spec import order, custom typeshed, offline pin verification against the store, a PyPI-package wheel pin, the segregated download component, bundled stdlib ZIP, and generation. |
| [Checker MCP service](specs/CHECKER-MCP-SPEC.md) | Packaged stdio lifecycle and the structured typeshed source/status tool. |
| [Checker cache](specs/CHECKER-CACHE-SPEC.md) | Opt-in content-addressed cross-session result cache, its `[tool.basilisk]` keys, and how it differs from always-on Salsa memoization. |
| [Rule tagging](specs/CHECKER-RULE-TAGGING-SPEC.md) | Rule provenance/category/free-form tags and conflict rules. |
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
| [WASM](specs/WASM-SPEC.md) | The checker compiled for the browser: one-shot in-memory checking with no filesystem, network, or threads. |
| [Editor screenshots](specs/VSIX-EDITOR-SCREENSHOTS-SPEC.md) | Automated real VS Code screenshots. |
| [Website E2E](specs/WEBSITE-E2E-SPEC.md) | The withdrawal-contract tests: approved copy on the statement page, every retired URL still resolving, and nothing forbidden anywhere in the build. |
| [READMEs](specs/DOCS-README-SPEC.md) | One authored README per language, generated to GitHub, the VSIX (Marketplace + Open VSX), and PyPI. |
| [Repository standards](specs/REPO-STANDARDS-SPEC.md) | Root/`.github` gates: duplication budget, coverage thresholds, committed editor directories, Dependabot, CodeQL, and dependency review. |
| [Release manual verification](specs/RELEASE-MANUAL-VERIFICATION-SPEC.md) | The manual passes a release person runs before publishing and again after, against the installed artifact: where `/ci-prep` fits, the artifact-provenance gate, the responsiveness smoke test, and the full hands-on test surface. |

## Active plans

Plans contain only unfinished work. Delete a plan when its acceptance gate passes.

| File | Remaining scope |
|---|---|
| [Roadmap](plans/ROADMAP-NEXT-STEPS-PLAN.md) | Distribution follow-ups, Shipwright deployment-contract conformity, permissive-only license footprint, trace controllability, editor-side result cache, Salsa v2, scale, ecosystem, and links to focused plans. |
| [Specification conformance audit](plans/SPEC-CONFORMANCE-AUDIT-PLAN.md) | Confirmed implementation/spec deviations, each linked to its tracker issue. |
| [Configuration editor](plans/LSP-CONFIGURATION-EDITOR-PLAN.md) | Canonical fixability metadata, DTO drift test, config-projection consolidation, field provenance, protocol/adoption/suppression E2E coverage, accessibility verification, cross-editor clients, and release gates. |
| [Formatting](plans/LSP-FORMATTING-PLAN.md) | VS Code default-formatter opt-in and published-artifact verification. |
| [AI-assisted LSP](plans/LSP-AI-PLAN.md) | First opt-in provider slice and privacy/safety gate. |
| [Activity panel](plans/EXTENSION-ACTIVITY-PANEL-PLAN.md) | Settings wiring, Modules-panel context menus and multi-select, and remaining cross-editor/test quality. |
| [Type narrowing and inference](plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md) | The engine build-out and the demolition order: wire the bidirectional engine into rules that genuinely analyse code, delete the rest rather than migrating them, and record what each deletion costs. |
| [Runtime typeshed resolution](plans/CHECKER-TYPESHED-RUNTIME-PLAN.md) | Two open items: a socket-instrumented witness that checking is offline across CLI/LSP/MCP, and byte-exact per-artifact licensing verification inside the VSIX (binaries and wheels are already verified). |
| [PyPI typeshed package pin](plans/CHECKER-TYPESHED-PYPI-PLAN.md) | Pin a PyPI typeshed distribution by wheel SHA-256, verify offline, auto-resolve from `uv.lock`; suppresses the source-status advisory (issue #312). |
| [Delete checker text matching](plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md) | The inventory of rules that decide from source text, the failing-test → delete → report disposal, and the unbuilt semantics-preserving mutation harness. |
| [WASM](plans/WASM-PLAN.md) | CI wasm build and size ratchet, multi-file in-memory VFS, and the playground site. |
| [Advanced checker features](plans/CHECKER-ADVANCED-FEATURES-PLAN.md) | Dependency-hygiene rules, ownership and safety checks, plugin host, migration, and CI helpers. |

## Defect triage

| File | Contents |
|---|---|
| [Typing puzzles](puzzles/puzzles.md) | User-reported typing puzzles from X, with minimal repros, PEP-bug vs house-rule classification, and the resulting issues (#371, #378–#383). |

## Conformance integrity

| File | Contents |
|---|---|
| [Conformance integrity audit](CONFORMANCE-INTEGRITY-AUDIT.md#CHKARCH-CONFORMANCE-INTEGRITY-AUDIT) | Phase 1: the fitted alias predicates, measured impact, wider checker scan, remediation status, and process changes found by the 2026-08 audit. The public site no longer carries a conformance page; this audit is the internal record ([WITHDRAWAL-SURFACES](specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-SURFACES)). |
