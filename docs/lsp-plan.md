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
| **6** | Cross-module, Auto-import, Incremental Sync, Salsa | Future | NOT STARTED |

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

## Phase 6 — Future

| Task | Description |
|------|-------------|
| 6.1 | Cross-module Go to Definition — workspace module resolver |
| 6.2 | Cross-module Find All References + Rename |
| 6.3 | Auto-import suggestions — suggest imports from workspace index |
| 6.4 | Incremental text sync — FULL → INCREMENTAL |
| 6.5 | Salsa integration — memoized incremental computation |
| ~~6.6~~ | ~~Go to Declaration / Go to Type Definition~~ | DONE |

---

## Pylance Feature Parity

> **Reference**: [Pylance README](https://github.com/microsoft/pylance-release/blob/main/README.md) · [Pyright docs](https://microsoft.github.io/pyright/#/)

### LSP Protocol Methods

| LSP Method | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| `textDocument/publishDiagnostics` | ✅ | ✅ DONE | — |
| `textDocument/completion` | ✅ | ✅ DONE | — |
| `textDocument/hover` — diagnostic info | ✅ | ✅ DONE | — |
| `textDocument/hover` — type signatures | ✅ | ✅ DONE | **1** |
| `textDocument/definition` | ✅ | ✅ DONE | **1** |
| `textDocument/declaration` | ✅ | ✅ DONE | **5** |
| `textDocument/typeDefinition` | ✅ | ✅ DONE | **5** |
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

### Completion Quality

| Sub-feature | Pylance | Basilisk | Phase |
|-------------|---------|---------|-------|
| Symbol completions (local scope) | ✅ | ✅ DONE | — |
| Dot-access (attribute) completions | ✅ | ✅ DONE | — |
| Import path completions | ✅ | ✅ DONE | — |
| Built-in completions (78 builtins) | ✅ | ✅ DONE | — |
| Auto-import suggestions | ✅ | ☐ TODO | 6 |
| Completion documentation (docstrings) | ✅ | ✅ DONE | **5** |
| Completion item resolve | ✅ | ☐ TODO | 6 |
| Keyword argument completions | ✅ | ✅ DONE | **2** |
| Override stub completions | ✅ | ☐ TODO | 6 |

### Code Actions & Refactoring

| Action | Pylance | Basilisk | Phase |
|--------|---------|---------|-------|
| Add parameter annotation (E0001) | ✅ | ✅ DONE | — |
| Add return annotation (E0002) | ✅ | ✅ DONE | — |
| Add variable annotation (E0003) | ✅ | ✅ DONE | **2** |
| Suppress with `# type: ignore` | ✅ | ✅ DONE | **2** |
| Organize imports (Ruff) | ✅ | ✅ DONE | **2** |
| Expand wildcard import | ✅ | ☐ TODO | 6 |
| Extract variable | ✅ | ☐ TODO | 6 |
| Extract method | ✅ | ☐ TODO | 6 |
| Convert import style | ✅ | ☐ TODO | 6 |
| Implement abstract methods | ✅ | ☐ TODO | 6 |
| Add `__all__` declaration | ✅ | ☐ TODO | 6 |
| Move symbol to another file | ✅ | ☐ TODO | 6 |

### Inlay Hints

| Hint Kind | Pylance | Basilisk | Phase |
|-----------|---------|---------|-------|
| Variable inferred types | ✅ | ✅ DONE | **3** |
| Function return types | ✅ | ✅ DONE | **3** |
| Parameter name labels at call sites | ✅ | ✅ DONE | **3** |
| Generic type parameter hints | ✅ | ☐ TODO | 6 |

### Type Checking & Diagnostics

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Missing parameter annotation | ✅ | ✅ DONE | — |
| Missing return annotation | ✅ | ✅ DONE | — |
| Type mismatch / incompatible assignment | ✅ | ✅ partial | — |
| Unknown / unresolved imports | ✅ | ✅ DONE | — |
| Undefined variables | ✅ | ✅ partial | — |
| Full type inference (generics, unions, narrowing) | ✅ | ☐ TODO | 6 |
| `pyrightconfig.json` / `pyproject.toml` config | ✅ | ☐ TODO | 6 |
| Stub file (`.pyi`) support | ✅ | ☐ TODO | 6 |
| Third-party type stubs (typeshed, `py.typed`) | ✅ | ☐ TODO | 6 |

### Cross-File & Workspace

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Single-file analysis | ✅ | ✅ DONE | — |
| Cross-file Go to Definition | ✅ | ☐ TODO | 6 |
| Cross-file Find All References | ✅ | ☐ TODO | 6 |
| Cross-file Rename | ✅ | ☐ TODO | 6 |
| Module-level auto-import index | ✅ | ☐ TODO | 6 |
| Multi-root workspace support | ✅ | ☐ TODO | 6 |

### Extension / UX

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Status bar (server state + error count) | ✅ | ✅ DONE | **0** |
| Restart Language Server command | ✅ | ✅ DONE | **0** |
| Show Output command | ✅ | ✅ DONE | **0** |
| Auto-restart on crash (max 3) | ✅ | ✅ DONE | **0** |
| Error message on server failure | ✅ | ✅ DONE | **0** |
| Color picker for hex color strings | ✅ | ☐ TODO | 6 |

---

## Rules

- Build must stay GREEN at all times
- No `.unwrap()` in server code
- No `println!` in production code (LSP stdout is sacred)
- `cargo clippy` must pass after every task
- E2E tests for every feature — no unit test theatre
- Do NOT delete failing tests — add more
