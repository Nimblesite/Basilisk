# LSP Implementation Plan

> **Spec**: `docs/LSP-SPEC.md` — read before touching any code.

---

## Phase Summary

| Phase | Features | Priority |
|-------|----------|----------|
| **0** | Fix server crash, extension error recovery, status bar, restart command | **Critical** |
| **1** | Hover (type info), Go to Definition, Document Symbols | **P1** |
| **2** | Signature Help, Find References, Rename, Expanded Code Actions | **P2** |
| **3** | Inlay Hints, Semantic Tokens | **P3** |
| **4** | Workspace Symbols, Format Document, Folding, Selection Ranges | **P4** |
| **5** | Call Hierarchy, Type Hierarchy, Cross-module, Auto-import, Salsa | Future |

---

## Phase 0 — Fix Server Crash + Extension Robustness

| Task | File(s) | Description |
|------|---------|-------------|
| 0.1 | `crates/basilisk-checker/src/rules/e0080.rs` | Delete all `println!("DEBUG: ...")` — these corrupt the LSP stdout stream |
| 0.2 | `crates/basilisk-lsp/tests/lsp_e2e_tests.rs` | Add test: first bytes from `basilisk lsp` start with `Content-Length:` |
| 0.3 | `vscode-extension/src/extension.ts` | Error recovery, auto-restart, status bar, restart command |
| 0.4 | `vscode-extension/package.json` | Register `basilisk.restartServer`, `basilisk.showOutput` commands |

---

## Phase 1 — Core Navigation

**Depends on**: Phase 0

| Task | File(s) | Description |
|------|---------|-------------|
| 1.0 | `crates/basilisk-lsp/src/util.rs` (new) | `find_symbol_at_offset`, `format_type_signature`, position conversion utils |
| 1.1 | `crates/basilisk-lsp/src/hover.rs` (new) | Type-aware hover: show signatures for functions, classes, variables, params |
| 1.2 | `crates/basilisk-lsp/src/definition.rs` (new) | Go to Definition: Ctrl+Click/F12 jumps to symbol definition |
| 1.3 | `crates/basilisk-lsp/src/symbols.rs` (new) | Document Symbols: hierarchical outline of classes, functions, variables |
| 1.4 | `crates/basilisk-lsp/src/completion.rs` (new) | Extract completion logic from server.rs into its own module |
| 1.5 | `crates/basilisk-lsp/src/code_actions.rs` (new) | Extract code action logic from server.rs into its own module |
| 1.6 | `crates/basilisk-lsp/src/server.rs` | Refactor: delegate to modules, add capabilities, cache ResolvedModule |
| 1.7 | `crates/basilisk-lsp/tests/lsp_e2e_tests.rs` | E2E tests for hover, go-to-def, document symbols |

---

## Phase 2 — Productivity Features

**Depends on**: Phase 1 (needs `find_symbol_at_offset`)

| Task | File(s) | Description |
|------|---------|-------------|
| 2.1 | `crates/basilisk-lsp/src/signature.rs` (new) | Signature Help: param hints on `(` and `,` |
| 2.2 | `crates/basilisk-lsp/src/references.rs` (new) | Find All References + Rename Symbol |
| 2.3 | `crates/basilisk-lsp/src/code_actions.rs` | Expand: smart type inference, organize imports via Ruff |
| 2.4 | `crates/basilisk-lsp/tests/lsp_e2e_tests.rs` | E2E tests for signature help, references, rename, code actions |

---

## Phase 3 — Inlay Hints + Semantic Tokens

**Depends on**: Phase 1 (needs cached ResolvedModule)

| Task | File(s) | Description |
|------|---------|-------------|
| 3.1 | `crates/basilisk-lsp/src/inlay_hints.rs` (new) | Variable type hints + parameter name hints at call sites |
| 3.2 | `crates/basilisk-lsp/src/semantic_tokens.rs` (new) | Classify tokens: function, class, parameter, variable, property, decorator |
| 3.3 | `vscode-extension/package.json` | Add `basilisk.inlayHints.*` settings |
| 3.4 | `vscode-extension/src/extension.ts` | Middleware to pass inlay hint settings to server |
| 3.5 | `crates/basilisk-lsp/tests/lsp_e2e_tests.rs` | E2E tests for inlay hints and semantic tokens |

---

## Phase 4 — Workspace Features + Formatting

**Depends on**: Phase 2

| Task | File(s) | Description |
|------|---------|-------------|
| 4.1 | `crates/basilisk-lsp/src/symbols.rs` | Add workspace symbol search (Ctrl+T) |
| 4.2 | `crates/basilisk-lsp/src/formatting.rs` (new) | Format Document via `ruff format` subprocess |
| 4.3 | `crates/basilisk-lsp/src/server.rs` | Add folding ranges + selection ranges handlers |
| 4.4 | `vscode-extension/package.json` | Add `basilisk.ruff.*` settings |
| 4.5 | `crates/basilisk-lsp/tests/lsp_e2e_tests.rs` | E2E tests for workspace symbols, formatting, folding |

---

## Phase 5 — Advanced (Future)

| Task | Description |
|------|-------------|
| 5.1 | Call Hierarchy — incoming/outgoing calls using `CallSite` data |
| 5.2 | Type Hierarchy — supertypes/subtypes using `ClassInfo.bases` |
| 5.3 | Cross-module Go to Definition — workspace module resolver |
| 5.4 | Auto-import — suggest imports from workspace index |
| 5.5 | Incremental text sync — FULL → INCREMENTAL |
| 5.6 | Salsa integration — memoized incremental computation |

---

## Rules

- Build must stay GREEN at all times
- No `.unwrap()` in server code
- No `println!` in production code (LSP stdout is sacred)
- `cargo clippy` must pass after every task
- E2E tests for every feature — no unit test theatre
- Do NOT delete failing tests — add more
