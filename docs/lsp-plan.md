# LSP Implementation Plan

> **Spec**: `docs/LSP-SPEC.md` — read before touching any code.

---

## Phase Summary

| Phase | Features | Priority | Status |
|-------|----------|----------|--------|
| **0** | Fix server crash, extension error recovery, status bar, restart command | **Critical** | DONE |
| **1** | Hover (type info), Go to Definition, Document Symbols, Completion, Code Actions | **P1** | DONE |
| **2** | Signature Help, Find References, Rename, Expanded Code Actions, Kwarg Completions | **P2** | DONE |
| **3** | Inlay Hints (types + params + return), Semantic Tokens | **P3** | DONE |
| **4** | Workspace Symbols, Format Document, Folding, Selection Ranges | **P4** | DONE |
| **5** | Document Highlight, Call Hierarchy, Type Hierarchy, Code Lens | **P5** | DONE |
| **6** | Single-file polish: declaration, typeDefinition, docstrings, code actions | **P6** | PARTIAL (3/8) |
| **7** | Cross-module foundation: workspace resolver, Salsa, stubs, config | **P7** | NOT STARTED |
| **8** | Cross-module features: cross-file nav, auto-import, multi-root | **P8** | NOT STARTED |
| **9** | Advanced refactoring: full inference, extract, move, abstract methods | **P9** | NOT STARTED |

---

## Phase 0 — Fix Server Crash + Extension Robustness (DONE)

| Task | File(s) | Description | Status |
|------|---------|-------------|--------|
| 0.1 | `crates/basilisk-checker/src/rules/e0080.rs` | Delete all `println!("DEBUG: ...")` | DONE |
| 0.2 | `crates/basilisk-lsp/tests/lsp_e2e_tests.rs` | E2E startup test | DONE (implicit) |
| 0.3 | `vscode-extension/src/extension.ts` | Error recovery, auto-restart (max 3), status bar | DONE |
| 0.4 | `vscode-extension/package.json` | Register commands: restartServer, showOutput, organizeImports | DONE |

---

## Phase 1 — Core Navigation (DONE)

| Task | File(s) | Description | Status |
|------|---------|-------------|--------|
| 1.0 | `src/util.rs` | `find_symbol_at_offset`, `format_type_signature`, position utils | DONE |
| 1.1 | `src/hover.rs` | Type-aware hover for all symbol kinds | DONE |
| 1.2 | `src/definition.rs` | Go to Definition (F12) | DONE |
| 1.3 | `src/symbols.rs` | Document Symbols (hierarchical outline) | DONE |
| 1.4 | `src/completion.rs` | Symbol + dot + import + builtin completions | DONE |
| 1.5 | `src/code_actions.rs` | Quick fixes extracted from server.rs | DONE |
| 1.6 | `src/server.rs` | Delegate to modules, add capabilities, cache ResolvedModule | DONE |
| 1.7 | E2E + WS tests | Hover, go-to-def, document symbols, completion tests | DONE |

---

## Phase 2 — Productivity Features (DONE)

| Task | File(s) | Description | Status |
|------|---------|-------------|--------|
| 2.1 | `src/signature.rs` | Signature Help with active parameter tracking | DONE |
| 2.2 | `src/references.rs` | Find All References + Rename Symbol | DONE |
| 2.3 | `src/code_actions.rs` | E0003 fix, suppress `# type: ignore`, organize imports via Ruff | DONE |
| 2.4 | `src/completion.rs` | Keyword argument completions | DONE |
| 2.5 | `src/server.rs` | `workspace/executeCommand` for organizeImports | DONE |
| 2.6 | E2E + WS tests | Signature help, references, rename, code actions, kwarg completions | DONE |

---

## Phase 3 — Inlay Hints + Semantic Tokens (DONE)

| Task | File(s) | Description | Status |
|------|---------|-------------|--------|
| 3.1 | `src/inlay_hints.rs` | Variable type hints + parameter name hints + return type hints | DONE |
| 3.2 | `src/semantic_tokens.rs` | Token classification (7+ types, 2+ modifiers) | DONE |
| 3.3 | `vscode-extension/package.json` | `basilisk.inlayHints.*` settings | DONE |
| 3.4 | `vscode-extension/src/extension.ts` | Middleware to pass inlay hint settings | DONE |
| 3.5 | E2E + WS tests | Inlay hints (3 types), semantic tokens | DONE |

---

## Phase 4 — Workspace Features + Formatting (DONE)

| Task | File(s) | Description | Status |
|------|---------|-------------|--------|
| 4.1 | `src/symbols.rs` | Workspace symbol search (Ctrl+T) | DONE |
| 4.2 | `src/formatting.rs` | Format Document via `ruff format` subprocess | DONE |
| 4.3 | `src/folding.rs` | Folding ranges (functions, classes, imports) | DONE |
| 4.4 | `src/selection.rs` | Selection ranges (Smart Select) | DONE |
| 4.5 | E2E + WS tests | Workspace symbols, formatting, folding, selection ranges | DONE |

---

## Phase 5 — Advanced Features (DONE)

| Task | File(s) | Description | Status |
|------|---------|-------------|--------|
| 5.1 | `src/highlight.rs` | Document Highlight — symbol occurrences | DONE |
| 5.2 | `src/call_hierarchy.rs` | Call Hierarchy — incoming/outgoing calls | DONE |
| 5.3 | `src/type_hierarchy.rs` | Type Hierarchy — supertypes/subtypes (capability injected via WS bridge) | DONE |
| 5.4 | `src/code_lens.rs` | Code Lens — "N references" above functions/classes | DONE |
| 5.5 | `src/semantic_tokens.rs` | 10 token types + 5 modifiers (decorator, type, typeParameter, static, deprecated) | DONE |
| 5.6 | `src/server.rs` | Register codeActionKinds explicitly | DONE |

---

## Phase 6 — Single-File Polish (achievable without cross-module infra)

> Items that can be done with the current single-file architecture.
> **Prerequisite**: None — these build on existing resolver/checker data.

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| ~~6.6~~ | ~~Go to Declaration / Go to Type Definition~~ | Easy | DONE |
| ~~6.7~~ | ~~Completion documentation (docstrings in hover + completions)~~ | Easy | DONE |
| 6.8 | Completion item resolve — lazy-load docs/detail on selection | Medium | TODO |
| 6.9 | Generic type parameter inlay hints | Medium | TODO |
| 6.10 | Expand wildcard import (code action) | Medium | TODO |
| 6.11 | Convert import style (code action: `import X` ↔ `from X import Y`) | Medium | TODO |
| 6.12 | Add `__all__` declaration (code action) | Easy | TODO |
| 6.13 | Color picker for hex color strings (`textDocument/documentColor`) | Easy | TODO |

## Phase 7 — Cross-Module Foundation (BLOCKING for everything below)

> **The big unlock.** Without a workspace module resolver, none of the cross-file
> features are possible. This phase builds the infrastructure.

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 7.1 | Workspace module resolver — scan workspace, resolve `import X` to file paths | Hard | TODO |
| 7.2 | Multi-file `ResolvedModule` graph — resolve across files, cache per-file | Hard | TODO |
| 7.3 | Incremental text sync — FULL → INCREMENTAL (`TextDocumentSyncKind::Incremental`) | Medium | TODO |
| 7.4 | Salsa integration — memoized incremental computation (like rust-analyzer) | Hard | TODO |
| 7.5 | Stub file (`.pyi`) support — resolve type info from `.pyi` alongside `.py` | Medium | TODO |
| 7.6 | Third-party type stubs — typeshed bundling, `py.typed` marker detection | Medium | TODO |
| 7.7 | `pyrightconfig.json` / `pyproject.toml` config — read strictness, paths, excludes | Medium | TODO |

## Phase 8 — Cross-Module Features (requires Phase 7)

> These all depend on the workspace module resolver from Phase 7.

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 8.1 | Cross-file Go to Definition | Medium | TODO |
| 8.2 | Cross-file Find All References | Medium | TODO |
| 8.3 | Cross-file Rename | Hard | TODO |
| 8.4 | Auto-import suggestions — suggest imports from workspace index | Hard | TODO |
| 8.5 | Module-level auto-import index | Hard | TODO |
| 8.6 | Multi-root workspace support | Medium | TODO |

## Phase 9 — Advanced Refactoring (requires Phase 7+8)

> Complex code actions that need full type inference and cross-module awareness.

| Task | Description | Difficulty | Status |
|------|-------------|------------|--------|
| 9.1 | Full type inference (generics, unions, narrowing) | Very Hard | TODO |
| 9.2 | Extract variable (code action) | Medium | TODO |
| 9.3 | Extract method (code action) | Hard | TODO |
| 9.4 | Implement abstract methods (code action) | Medium | TODO |
| 9.5 | Override stub completions | Medium | TODO |
| 9.6 | Move symbol to another file (code action) | Hard | TODO |

---

## Pylance Feature Parity

> **Reference**: [Pylance README](https://github.com/microsoft/pylance-release/blob/main/README.md) · [Pyright docs](https://microsoft.github.io/pyright/#/)

### LSP Protocol Methods

| LSP Method | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| `textDocument/publishDiagnostics` | ✅ | ✅ DONE | — |
| `textDocument/completion` | ✅ | ✅ DONE | — |
| `textDocument/hover` — diagnostic info | ✅ | ✅ DONE | — |
| `textDocument/hover` — type signatures + docstrings | ✅ | ✅ DONE | **1, 6** |
| `textDocument/definition` | ✅ | ✅ DONE | **1** |
| `textDocument/declaration` | ✅ | ✅ DONE | **6** |
| `textDocument/typeDefinition` | ✅ | ✅ DONE | **6** |
| `textDocument/documentSymbol` | ✅ | ✅ DONE | **1** |
| `workspace/symbol` | ✅ | ✅ DONE | **4** |
| `textDocument/signatureHelp` | ✅ | ✅ DONE | **2** |
| `textDocument/references` | ✅ | ✅ DONE | **2** |
| `textDocument/prepareRename` + `rename` | ✅ | ✅ DONE | **2** |
| `textDocument/codeAction` — quick fixes | ✅ | ✅ DONE | — |
| `textDocument/codeAction` — E0003, suppress, organize | ✅ | ✅ DONE | **2** |
| `textDocument/inlayHint` | ✅ | ✅ DONE | **3** |
| `textDocument/semanticTokens/full` | ✅ | ✅ DONE | **3** |
| `textDocument/formatting` | ✅ | ✅ DONE | **4** |
| `textDocument/foldingRange` | ✅ | ✅ DONE | **4** |
| `textDocument/selectionRange` | ✅ | ✅ DONE | **4** |
| `textDocument/documentHighlight` | ✅ | ✅ DONE | **5** |
| `textDocument/codeLens` | ✅ | ✅ DONE | **5** |
| `textDocument/prepareCallHierarchy` + calls | ✅ | ✅ DONE | **5** |
| `textDocument/prepareTypeHierarchy` + types | ✅ | ✅ DONE | **5** |
| `workspace/executeCommand` | ✅ | ✅ DONE | **2** |
| `textDocument/documentColor` | ✅ | ☐ TODO | **6** |

### Completion Quality

| Sub-feature | Pylance | Basilisk | Phase |
|-------------|---------|---------|-------|
| Symbol completions (local scope) | ✅ | ✅ DONE | — |
| Dot-access (attribute) completions | ✅ | ✅ DONE | — |
| Import path completions | ✅ | ✅ DONE | — |
| Built-in completions (78 builtins) | ✅ | ✅ DONE | — |
| Completion documentation (docstrings) | ✅ | ✅ DONE | **6** |
| Keyword argument completions | ✅ | ✅ DONE | **2** |
| Completion item resolve | ✅ | ☐ TODO | **6** |
| Auto-import suggestions | ✅ | ☐ TODO | **8** |
| Override stub completions | ✅ | ☐ TODO | **9** |

### Code Actions & Refactoring

| Action | Pylance | Basilisk | Phase |
|--------|---------|---------|-------|
| Add parameter annotation (E0001) | ✅ | ✅ DONE | — |
| Add return annotation (E0002) | ✅ | ✅ DONE | — |
| Add variable annotation (E0003) | ✅ | ✅ DONE | **2** |
| Suppress with `# type: ignore` | ✅ | ✅ DONE | **2** |
| Organize imports (Ruff) | ✅ | ✅ DONE | **2** |
| Expand wildcard import | ✅ | ☐ TODO | **6** |
| Convert import style | ✅ | ☐ TODO | **6** |
| Add `__all__` declaration | ✅ | ☐ TODO | **6** |
| Extract variable | ✅ | ☐ TODO | **9** |
| Extract method | ✅ | ☐ TODO | **9** |
| Implement abstract methods | ✅ | ☐ TODO | **9** |
| Move symbol to another file | ✅ | ☐ TODO | **9** |

### Inlay Hints

| Hint Kind | Pylance | Basilisk | Phase |
|-----------|---------|---------|-------|
| Variable inferred types | ✅ | ✅ DONE | **3** |
| Function return types | ✅ | ✅ DONE | **3** |
| Parameter name labels at call sites | ✅ | ✅ DONE | **3** |
| Generic type parameter hints | ✅ | ☐ TODO | **6** |

### Type Checking & Diagnostics

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Missing parameter annotation | ✅ | ✅ DONE | — |
| Missing return annotation | ✅ | ✅ DONE | — |
| Type mismatch / incompatible assignment | ✅ | ✅ partial | — |
| Unknown / unresolved imports | ✅ | ✅ DONE | — |
| Undefined variables | ✅ | ✅ partial | — |
| Full type inference (generics, unions, narrowing) | ✅ | ☐ TODO | **9** |
| `pyrightconfig.json` / `pyproject.toml` config | ✅ | ☐ TODO | **7** |
| Stub file (`.pyi`) support | ✅ | ☐ TODO | **7** |
| Third-party type stubs (typeshed, `py.typed`) | ✅ | ☐ TODO | **7** |

### Cross-File & Workspace

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Single-file analysis | ✅ | ✅ DONE | — |
| Cross-file Go to Definition | ✅ | ☐ TODO | **8** |
| Cross-file Find All References | ✅ | ☐ TODO | **8** |
| Cross-file Rename | ✅ | ☐ TODO | **8** |
| Module-level auto-import index | ✅ | ☐ TODO | **8** |
| Multi-root workspace support | ✅ | ☐ TODO | **8** |

### Extension / UX

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Status bar (server state + error count) | ✅ | ✅ DONE | **0** |
| Restart Language Server command | ✅ | ✅ DONE | **0** |
| Show Output command | ✅ | ✅ DONE | **0** |
| Auto-restart on crash (max 3) | ✅ | ✅ DONE | **0** |
| Error message on server failure | ✅ | ✅ DONE | **0** |
| Color picker for hex color strings | ✅ | ☐ TODO | **6** |

---

## Rules

- Build must stay GREEN at all times
- No `.unwrap()` in server code
- No `println!` in production code (LSP stdout is sacred)
- `cargo clippy` must pass after every task
- E2E tests for every feature — no unit test theatre
- Do NOT delete failing tests — add more
