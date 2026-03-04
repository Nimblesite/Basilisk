# Basilisk LSP & VSIX — Full IDE Feature Specification

> **Goal**: Compete with Pylance. Every feature that makes a Python IDE useful.

---

## Current State

### What Works
- Diagnostics publishing (97 error codes, full parse→resolve→check pipeline)
- Hover: type signatures for functions, classes, variables, parameters, attributes + diagnostic info
- Go to Definition (F12) — definition site and call-site/reference lookup
- Document Symbols (Outline panel) — hierarchical class/function/variable tree
- Signature Help — parameter hints on `(` and `,`
- Find All References — all usages in current file
- Rename Symbol (F2) — prepareRename + rename across current file
- Inlay Hints — inferred variable types
- Semantic Tokens — function, class, parameter, variable, property, decorator classification
- Code actions: quick fixes for BSK-E0001 (`: Any`) and BSK-E0002 (`-> None`)
- Completion: symbol + dot + imports + 78 builtins
- Full document sync, DashMap document store, UTF-16 position handling
- VS Code extension: status bar, restart command, show output, auto-restart, error recovery, organize imports (Ruff)

### What's Missing (Future Work)
- Workspace Symbols (Ctrl+T search)
- Call/Type Hierarchy
- Folding Ranges
- Selection Ranges
- Format Document (Ruff delegation)
- Cross-module Go to Definition / References / Rename
- Auto-import suggestions
- Inlay hints: parameter name labels at call sites, function return types

---

## Architecture

### Three-Phase Pipeline

```
Source Text
    │
    ▼
basilisk-parser::parse_source() → ParsedModule (Ruff AST)
    │
    ▼
basilisk-resolver::resolve() → ResolvedModule (symbol table)
    │
    ▼
basilisk-checker::check() → Vec<Diagnostic>
```

### ResolvedModule — The Data That Powers Everything

`ResolvedModule` (defined in `crates/basilisk-resolver/src/scope.rs`) contains:

| Field | Type | Powers |
|-------|------|--------|
| `functions` | `Vec<FunctionInfo>` | Hover, completion, signature help, go-to-def, outline |
| `classes` | `Vec<ClassInfo>` | Hover, completion, go-to-def, outline, type hierarchy |
| `module_vars` | `Vec<VariableInfo>` | Hover, completion, go-to-def, inlay hints |
| `imports` | `Vec<ImportInfo>` | Completion, go-to-def, outline |
| `calls` | `Vec<CallSite>` | Inlay hints (param names), call hierarchy |
| `module_attr_accesses` | `Vec<ModuleAttrAccessInfo>` | Find references |
| `typevar_calls` | `Vec<TypeVarCallInfo>` | Semantic tokens |
| `source` | `String` | Extracting annotation text from spans |

**FunctionInfo** carries: `name`, `name_span`, `def_span`, `parameters` (with `name_span`, `annotation_span`), `return_annotation`, `decorators`, `class_name`, `return_stmts`, `local_vars`.

**ClassInfo** carries: `name`, `name_span`, `def_span`, `bases`, `attributes` (with `name_span`, `annotation_span`), `method_names`, `is_dataclass`, `is_typed_dict`, etc.

Every symbol has a `Span` (byte start/end) for precise positioning.

### Server Module Structure (Target)

```
crates/basilisk-lsp/src/
  server.rs          — LspServer struct + tower-lsp trait impl (dispatch only)
  lib.rs             — re-exports run_server, check_source
  util.rs            — find_symbol_at_offset, position conversion, format_type_signature
  hover.rs           — type-aware hover + diagnostic hover
  definition.rs      — go-to-definition
  references.rs      — find all references + rename
  symbols.rs         — document symbols + workspace symbols
  completion.rs      — symbol + dot + import completions (extract from server.rs)
  signature.rs       — signature help
  inlay_hints.rs     — inferred types + parameter names
  semantic_tokens.rs — token classification
  code_actions.rs    — quick fixes (extract + expand from server.rs)
  formatting.rs      — Ruff delegation
```

Each module exports pure functions: `(resolved: &ResolvedModule, source: &str, ...) → LSP response type`.

### Performance: Cache ResolvedModule

Currently every hover/completion/code-action re-parses the entire document. Fix:

```rust
struct DocumentState {
    text: String,
    resolved: Option<Arc<ResolvedModule>>,  // cached from last did_change/did_open
}
```

Update `resolved` on `did_change`/`did_open`. Reuse cached result for all feature handlers.

---

## Phase 0: Fix Server Crash + Extension Robustness

### 0.1 Remove stdout pollution

**Root cause**: 10 `println!("DEBUG: ...")` calls in `crates/basilisk-checker/src/rules/e0080.rs` (lines 69, 92, 101, 113, 115, 119, 128, 158, 163, 178, 187). When the LSP server runs the checker, these write raw text to stdout, corrupting the JSON-RPC Content-Length framing.

**Fix**: Delete all `println!` calls from `e0080.rs`. Audit all other production crates for stray `println!`/`print!` calls. Add a CI check: `grep -rn 'println!' crates/*/src/ | grep -v '#\[cfg(test)\]'` must return empty.

### 0.2 E2E startup test

Add to `crates/basilisk-lsp/tests/lsp_e2e_tests.rs`:

```rust
#[test]
fn test_lsp_first_output_is_content_length() {
    // Spawn `basilisk lsp`, send initialize request,
    // assert first bytes of response start with "Content-Length:"
}
```

### 0.3 Extension error recovery

In `vscode-extension/src/extension.ts`:

```typescript
// Error handling on client start
client.start().catch((error) => {
    vscode.window.showErrorMessage(
        `Basilisk: Failed to start language server. Is '${executablePath}' installed? ${error.message}`
    );
});

// Auto-restart on crash (up to 3 times)
const clientOptions: LanguageClientOptions = {
    // ...existing...
    errorHandler: {
        error: (_error, _message, count) => {
            if (count && count < 3) return { action: ErrorAction.Continue };
            return { action: ErrorAction.Shutdown };
        },
        closed: () => ({ action: CloseAction.Restart }),
    },
};
```

### 0.4 Status bar

Persistent status bar item:
- `$(check) Basilisk` — green, server running, no errors in current file
- `$(warning) Basilisk (3)` — errors in current file
- `$(error) Basilisk` — server failed/not running
- `$(sync~spin) Basilisk` — analyzing

Update on `onDidChangeActiveTextEditor` and diagnostic count changes.

### 0.5 New commands

Register in `package.json`:

```json
"commands": [
    { "command": "basilisk.restartServer", "title": "Basilisk: Restart Language Server" },
    { "command": "basilisk.showOutput", "title": "Basilisk: Show Output" }
]
```

### Files to modify
- `crates/basilisk-checker/src/rules/e0080.rs` — delete all `println!`
- `crates/basilisk-lsp/tests/lsp_e2e_tests.rs` — startup byte test
- `vscode-extension/src/extension.ts` — error recovery, status bar, commands
- `vscode-extension/package.json` — commands

---

## Phase 1: Core Navigation

### Shared Infrastructure: `find_symbol_at_offset`

Central symbol lookup function reused by hover, go-to-def, references, rename:

```rust
// crates/basilisk-lsp/src/util.rs

pub enum SymbolHit<'a> {
    Function(&'a FunctionInfo),
    Class(&'a ClassInfo),
    Variable(&'a VariableInfo),
    Parameter { func: &'a FunctionInfo, param: &'a ParameterInfo },
    Attribute { class: &'a ClassInfo, attr: &'a AttributeInfo },
    Import(&'a ImportInfo),
}

/// Find the symbol at a byte offset by checking all name_spans in the ResolvedModule.
pub fn find_symbol_at_offset(resolved: &ResolvedModule, offset: usize) -> Option<SymbolHit<'_>>
```

Logic: iterate `resolved.functions` (check `name_span`, then each `param.name_span`), `resolved.classes` (check `name_span`, then each `attr.name_span`), `resolved.module_vars`, `resolved.imports`. Return first where `span.start <= offset < span.end`.

Also: `pub fn format_type_signature(hit: &SymbolHit, source: &str) -> String` — builds hover markdown for any symbol kind.

### 1a. Hover with Type Information

**LSP method**: `textDocument/hover` (capability already advertised)

**Current behavior**: Only shows diagnostic message when cursor is on an error span. Returns `None` for clean code.

**New behavior**: Show type signatures for any symbol, with diagnostics as secondary:

```
(function) def greet(name: str) -> str

---
**BSK-E0003** — Variable assignment missing type annotation
```

| Symbol Kind | Display Format |
|------------|---------------|
| Function | `(function) def name(param: Type, ...) -> ReturnType` |
| Method | `(method) def ClassName.name(self, param: Type, ...) -> ReturnType` |
| Class | `(class) ClassName(Base1, Base2)` |
| Variable | `(variable) name: Type` or `(variable) name = <inferred type>` |
| Parameter | `(parameter) name: Type` |
| Attribute | `(property) ClassName.name: Type` |

**Data sources**:
- Function signature: `FunctionInfo.parameters` + `return_annotation` + source text via `annotation_span`
- Class bases: `ClassInfo.bases`
- Variable type: `VariableInfo.annotation_span` (annotated) or `infer_rhs(&var.rhs_kind)` (unannotated)
- Parameter type: `ParameterInfo.annotation_span` + source text

### 1b. Go to Definition

**LSP method**: `textDocument/definition`

**Capability**: Add `definitionProvider: true` to `ServerCapabilities`

**Behavior**: Ctrl+Click / F12 on a symbol jumps to its definition.

| Cursor on | Jumps to |
|-----------|----------|
| Function call `greet(...)` | `FunctionInfo.name_span` of `greet` |
| Class instantiation `Dog(...)` | `ClassInfo.name_span` of `Dog` |
| Variable reference `x` | `VariableInfo.name_span` of `x` |
| `self.attr` | `AttributeInfo.name_span` in enclosing class |
| `ClassName.attr` | `AttributeInfo.name_span` in named class |
| Parameter reference | `ParameterInfo.name_span` in function |

**Implementation**:
1. Get byte offset from cursor position
2. Extract identifier at offset from source text
3. Search `ResolvedModule` for a definition with that name
4. Return `Location` with URI (same file) and `name_span` converted to LSP range

**Scope**: Single-file only in Phase 1. Cross-module requires workspace module resolver (Phase 5).

### 1c. Document Symbols (Outline)

**LSP method**: `textDocument/documentSymbol`

**Capability**: Add `documentSymbolProvider: true`

**Behavior**: Outline panel shows hierarchical tree of all definitions:

```
▼ MyClass                          (class)
    name: str                      (field)
    age: int                       (field)
    ▶ __init__(self, name, age)    (method)
    ▶ greet(self) -> str           (method)
▶ helper(x: int) -> int           (function)
  MAX_SIZE: int                    (variable)
```

**Implementation**:
1. Build top-level symbols from `functions` (where `class_name.is_none()`), `classes`, `module_vars`, `imports`
2. Nest under classes: functions where `class_name == Some(class_name)`, class `attributes`
3. Map to `DocumentSymbol` with:
   - `name`: symbol name
   - `kind`: `SymbolKind::Class`, `Function`, `Method`, `Variable`, `Field`
   - `range`: `def_span` converted to LSP range
   - `selectionRange`: `name_span` converted to LSP range
   - `detail`: type annotation string (optional)
   - `children`: nested symbols for classes

### Files
- New: `crates/basilisk-lsp/src/util.rs`
- New: `crates/basilisk-lsp/src/hover.rs`
- New: `crates/basilisk-lsp/src/definition.rs`
- New: `crates/basilisk-lsp/src/symbols.rs`
- Modify: `crates/basilisk-lsp/src/server.rs` — add capabilities, delegate to new modules
- Modify: `crates/basilisk-lsp/src/lib.rs` — declare modules
- Add E2E tests to `crates/basilisk-lsp/tests/lsp_e2e_tests.rs`

---

## Phase 2: Productivity Features

### 2a. Signature Help

**LSP method**: `textDocument/signatureHelp`

**Capability**: `signatureHelpProvider: { triggerCharacters: ["(", ","] }`

**Behavior**: When typing `greet(` or pressing `,` inside a call:

```
greet(name: str, greeting: str) -> str
      ^^^^^^^^^^
      parameter 1 of 2
```

**Implementation**:
1. Scan backwards from cursor for unmatched `(`
2. Extract callee name (text before the `(`)
3. Look up `FunctionInfo` by name in `ResolvedModule`
4. Count commas before cursor position to determine `activeParameter`
5. Build `SignatureInformation` with `ParameterInformation` for each param
6. For methods: skip `self`/`cls` in display, adjust `activeParameter` index

**Data**: `FunctionInfo.parameters` (name, annotation_span), `return_annotation`

### 2b. Find All References

**LSP method**: `textDocument/references`

**Capability**: `referencesProvider: true`

**Behavior**: Right-click → Find All References shows every use of a symbol in the file.

**Implementation**:
1. Use `find_symbol_at_offset` to identify the target symbol name
2. Collect all locations where that name appears:
   - Definition sites: `name_span` from the symbol's info struct
   - Call sites: `calls[].callee == name` → use `call.span`
   - Attribute accesses: `module_attr_accesses` where attr matches
   - Text scan: find all word-boundary matches of the identifier in source, filter out strings/comments
3. If `params.context.include_declaration`, include the definition site
4. Return `Vec<Location>`

### 2c. Rename Symbol

**LSP methods**: `textDocument/prepareRename`, `textDocument/rename`

**Capability**: `renameProvider: { prepareProvider: true }`

**Behavior**: F2 renames a symbol everywhere in the current file.

**Implementation**:
1. `prepareRename`: verify cursor is on a renameable symbol (function, class, variable, parameter, attribute). Return `Range` + placeholder text.
2. `rename`: use same reference-finding as 2b, return `WorkspaceEdit` with `TextEdit` for each occurrence.
3. Single-file scope for now.

### 2d. Expanded Code Actions

Extend beyond BSK-E0001/E0002:

| Diagnostic | Action | Transformation |
|-----------|--------|----------------|
| BSK-E0001 | Smart type annotation | Infer type from usage/default value instead of always `: Any` |
| BSK-E0002 | Smart return type | Infer from `return_stmts[].rhs_kind` instead of always `-> None` |
| BSK-E0003 | Add variable annotation | Insert `: <inferred_type>` |
| (any) | Suppress with `# type: ignore` | Append comment to line |
| (source) | Organize imports | Delegate to `ruff check --select I --fix` |

Register `codeActionKinds`: `[QUICKFIX, SOURCE_ORGANIZE_IMPORTS, REFACTOR]`

### Files
- New: `crates/basilisk-lsp/src/signature.rs`
- New: `crates/basilisk-lsp/src/references.rs`
- Modify: `crates/basilisk-lsp/src/code_actions.rs` (extracted from server.rs + expanded)
- Add E2E tests

---

## Phase 3: Inlay Hints + Semantic Tokens

### 3a. Inlay Hints

**LSP method**: `textDocument/inlayHint`

**Capability**: `inlayHintProvider: true`

**What the user sees**:

```python
x = 42                    # ghost text:  x: int = 42
name = "hello"            # ghost text:  name: str = "hello"
greet("Alice", "Hi")      # ghost text:  greet(name= "Alice", greeting= "Hi")
```

**Two hint categories**:

1. **Variable type hints** — for unannotated variables, show inferred type after name
   - Data: `module_vars` + `functions[].local_vars` where `has_annotation == false`
   - Type: `infer_rhs(&var.rhs_kind).to_string()` from `basilisk_checker::inference`
   - Position: `InlayHintKind::TYPE` at `name_span.end`

2. **Parameter name hints** — at call sites, show parameter names before arguments
   - Data: `calls[].args` cross-referenced with `functions[].parameters`
   - Position: `InlayHintKind::PARAMETER` at each arg span start
   - Label: `"param_name ="` or `"param_name:"`

**Extension settings** (new):
```json
"basilisk.inlayHints.parameterNames": { "type": "boolean", "default": true },
"basilisk.inlayHints.variableTypes": { "type": "boolean", "default": true }
```

### 3b. Semantic Tokens

**LSP method**: `textDocument/semanticTokens/full`

**Capability**: `semanticTokensProvider` with legend

**Token type legend**:

| Token Type | Applied To |
|-----------|-----------|
| `function` | Function names at definition and call sites |
| `method` | Method names (functions with `class_name.is_some()`) |
| `class` | Class names at definition and reference sites |
| `parameter` | Parameter names in function signatures |
| `variable` | Module-level and local variable names |
| `property` | Class attribute names |
| `decorator` | Decorator names (`@staticmethod`, `@override`, etc.) |
| `type` | Type annotation identifiers |
| `typeParameter` | TypeVar names, PEP 695 type params |

**Token modifier legend**: `declaration`, `definition`, `readonly`, `static`, `deprecated`

**Data**: All `name_span`s from `ResolvedModule` — each symbol's span is classified by its semantic role.

### Files
- New: `crates/basilisk-lsp/src/inlay_hints.rs`
- New: `crates/basilisk-lsp/src/semantic_tokens.rs`
- Modify: `vscode-extension/package.json` — inlay hint settings
- Modify: `vscode-extension/src/extension.ts` — pass settings to server via middleware
- Add E2E tests

---

## Phase 4: Workspace Features + Formatting

### 4a. Workspace Symbols

**LSP method**: `workspace/symbol`

**Capability**: `workspaceSymbolProvider: true`

**Behavior**: Ctrl+T opens symbol search across all open documents. Aggregate `DocumentSymbol` data from all entries in `DashMap`, filter by query string. Return `Vec<SymbolInformation>`.

### 4b. Format Document (Ruff Delegation)

**LSP method**: `textDocument/formatting`

**Capability**: `documentFormattingProvider: true`

**Implementation**: Spawn `ruff format --stdin-filename <path> -` with document text on stdin. Capture stdout. Return single `TextEdit` replacing entire document content.

**Extension settings** (new):
```json
"basilisk.ruff.enabled": { "type": "boolean", "default": true },
"basilisk.ruff.executablePath": { "type": "string", "default": "ruff" }
```

### 4c. Folding Ranges

**LSP method**: `textDocument/foldingRange`

**Capability**: `foldingRangeProvider: true`

**Implementation**: Emit `FoldingRange` for each:
- Function definition (`def_span` start line to end line)
- Class definition (`def_span` start line to end line)
- Import block (consecutive import statement lines grouped)
- Multiline strings/comments

### 4d. Selection Ranges

**LSP method**: `textDocument/selectionRange`

**Capability**: `selectionRangeProvider: true`

**Behavior**: Smart Select (Shift+Alt+Right) expands: identifier → parameter → parameter list → function body → function → class → module.

**Implementation**: Build nested range tree from `ResolvedModule` spans. For each cursor position, return the chain from innermost containing span to outermost.

### Files
- New: `crates/basilisk-lsp/src/formatting.rs`
- Modify: `crates/basilisk-lsp/src/symbols.rs` — add workspace symbols
- Modify: `crates/basilisk-lsp/src/server.rs` — add capabilities + handlers
- Modify: `vscode-extension/package.json` — Ruff settings
- Add E2E tests

---

## Phase 5: Advanced (Future)

### 5a. Call Hierarchy
- `textDocument/prepareCallHierarchy`, `callHierarchy/incomingCalls`, `callHierarchy/outgoingCalls`
- Data: `calls[].callee` for outgoing. For incoming: search all functions' call sites for target name.

### 5b. Type Hierarchy
- `textDocument/prepareTypeHierarchy`, `typeHierarchy/supertypes`, `typeHierarchy/subtypes`
- Data: `classes[].bases` for supertypes. For subtypes: search all classes whose `bases` contains target.

### 5c. Cross-Module Go to Definition
- Requires workspace-level module resolver: map `import foo` to file path, parse that file, resolve symbols.
- New infrastructure: `WorkspaceIndex` that maps module names to file paths and caches `ResolvedModule` per file.

### 5d. Auto-Import Suggestions
- When a name is unresolved, suggest imports from the workspace index.
- Requires cross-module symbol index from 5c.

### 5e. Incremental Text Sync
- Switch from `TextDocumentSyncKind::FULL` to `INCREMENTAL`
- Apply `TextDocumentContentChangeEvent` patches to stored text
- Reduces data transferred on each keystroke

### 5f. Salsa Integration
- Wire `basilisk-db` crate with Salsa framework for memoized incremental computation
- Target: <10ms incremental checks, <5s cold start on 100K LOC

---

## VS Code Extension Enhancements

### New Commands

```json
{
    "commands": [
        { "command": "basilisk.restartServer", "title": "Basilisk: Restart Language Server" },
        { "command": "basilisk.showOutput", "title": "Basilisk: Show Output" },
        { "command": "basilisk.organizeImports", "title": "Basilisk: Organize Imports" }
    ]
}
```

### New Configuration Settings

```json
{
    "basilisk.inlayHints.parameterNames": {
        "type": "boolean", "default": true,
        "description": "Show parameter name hints at call sites."
    },
    "basilisk.inlayHints.variableTypes": {
        "type": "boolean", "default": true,
        "description": "Show inferred type hints for unannotated variables."
    },
    "basilisk.ruff.enabled": {
        "type": "boolean", "default": true,
        "description": "Enable Ruff integration for formatting and import organization."
    },
    "basilisk.ruff.executablePath": {
        "type": "string", "default": "ruff",
        "description": "Path to the ruff binary."
    }
}
```

### Status Bar Item

Persistent item showing server state and diagnostic count. Updates on:
- `LanguageClient.onDidChangeState` — server started/stopped/crashed
- `vscode.languages.onDidChangeDiagnostics` — error count changed
- `vscode.window.onDidChangeActiveTextEditor` — switched files

### Error Recovery

- `errorHandler` on `LanguageClient` for auto-restart (max 3 attempts)
- User-visible error message when server fails to start (show stderr)
- `basilisk.restartServer` command for manual recovery

---

## Pylance Feature Parity

> **Reference**: [Pylance README](https://github.com/microsoft/pylance-release/blob/main/README.md) · [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=ms-python.vscode-pylance) · [Pyright docs](https://microsoft.github.io/pyright/#/) · [basedpyright language server settings](https://docs.basedpyright.com/v1.23.1/configuration/language-server-settings/)

This table maps every publicly documented Pylance/Pyright feature to its Basilisk implementation status and target phase.

### LSP Protocol Methods

| LSP Method | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| `textDocument/publishDiagnostics` — as-you-type errors/warnings | ✅ | ✅ DONE | — |
| `textDocument/completion` — symbol, attribute, keyword completions | ✅ | ✅ DONE | — |
| `textDocument/hover` — diagnostic info on error spans | ✅ | ✅ DONE | — |
| `textDocument/hover` — type signatures, docstrings for any symbol | ✅ | ✅ DONE | **1** |
| `textDocument/definition` — Go to Definition (F12) | ✅ | ✅ DONE | **1** |
| `textDocument/declaration` — Go to Declaration | ✅ | ☐ TODO | 5 |
| `textDocument/typeDefinition` — Go to Type Definition | ✅ | ☐ TODO | 5 |
| `textDocument/documentSymbol` — Outline panel (class/function tree) | ✅ | ✅ DONE | **1** |
| `workspace/symbol` — Ctrl+T cross-file symbol search | ✅ | ☐ TODO | **4** |
| `textDocument/signatureHelp` — parameter hints on `(` and `,` | ✅ | ✅ DONE | **2** |
| `textDocument/references` — Find All References | ✅ | ✅ DONE | **2** |
| `textDocument/prepareRename` + `textDocument/rename` — F2 rename | ✅ | ✅ DONE | **2** |
| `textDocument/codeAction` — quick fixes (BSK-E0001, BSK-E0002) | ✅ | ✅ DONE | — |
| `textDocument/codeAction` — expanded quick fixes (E0003, suppress, organize imports) | ✅ | ☐ TODO | **2** |
| `textDocument/inlayHint` — inferred variable types + parameter names | ✅ | ✅ DONE | **3** |
| `textDocument/semanticTokens/full` — semantic syntax highlighting | ✅ | ✅ DONE | **3** |
| `textDocument/formatting` — Format Document (Ruff delegation) | ✅ | ☐ TODO | **4** |
| `textDocument/foldingRange` — collapse functions, classes, imports | ✅ | ☐ TODO | **4** |
| `textDocument/selectionRange` — Smart Select (Shift+Alt+→) | ✅ | ☐ TODO | **4** |
| `textDocument/documentHighlight` — highlight all occurrences of symbol under cursor | ✅ | ☐ TODO | 5 |
| `textDocument/codeLens` — run/debug inline lens | ✅ | ☐ TODO | 5 |
| `textDocument/prepareCallHierarchy` + incoming/outgoing calls | ✅ | ☐ TODO | 5 |
| `textDocument/prepareTypeHierarchy` + supertypes/subtypes | ✅ | ☐ TODO | 5 |
| `workspace/executeCommand` — `basilisk.organizeImports` | ✅ | ☐ TODO | **2** |

### Completion Quality

| Sub-feature | Pylance | Basilisk | Phase |
|-------------|---------|---------|-------|
| Symbol completions (local scope) | ✅ | ✅ DONE | — |
| Dot-access (attribute) completions | ✅ | ✅ DONE | — |
| Import path completions | ✅ | ✅ DONE | — |
| Built-in completions (78 builtins) | ✅ | ✅ DONE | — |
| Auto-import suggestions (unresolved name → add import) | ✅ | ☐ TODO | 5 |
| Completion documentation (docstrings in popup) | ✅ | ☐ TODO | **2** |
| Completion item resolve (lazy-load details) | ✅ | ☐ TODO | **2** |
| Keyword argument completions | ✅ | ☐ TODO | **2** |
| Override stub completions (abstract method bodies) | ✅ | ☐ TODO | 5 |

### Code Actions & Refactoring

| Action | Pylance | Basilisk | Phase |
|--------|---------|---------|-------|
| Add missing parameter annotation (BSK-E0001) | ✅ | ✅ DONE | — |
| Add missing return annotation (BSK-E0002) | ✅ | ✅ DONE | — |
| Add variable annotation (BSK-E0003) | ✅ | ☐ TODO | **2** |
| Suppress with `# type: ignore` | ✅ | ☐ TODO | **2** |
| Organize imports (`ruff check --select I --fix`) | ✅ | ☐ TODO | **2** |
| Expand wildcard import (`from foo import *` → explicit names) | ✅ | ☐ TODO | 5 |
| Extract variable | ✅ | ☐ TODO | 5 |
| Extract method | ✅ | ☐ TODO | 5 |
| Convert import style (relative ↔ absolute) | ✅ | ☐ TODO | 5 |
| Implement abstract methods (generate stubs) | ✅ | ☐ TODO | 5 |
| Add `__all__` declaration | ✅ | ☐ TODO | 5 |
| Move symbol to another file | ✅ | ☐ TODO | 5 |
| Generate docstring (Copilot-assisted) | ✅ | ✗ out of scope | — |

### Inlay Hints (per [Pylance inlay hints docs](https://github.com/microsoft/pylance-release/blob/main/README.md))

| Hint Kind | Pylance | Basilisk | Phase |
|-----------|---------|---------|-------|
| Variable inferred types (`x: int`) | ✅ | ✅ DONE | **3** |
| Function return types | ✅ | ☐ TODO | **3** |
| Parameter name labels at call sites (`greet(name= "Alice")`) | ✅ | ☐ TODO | **3** |
| Pytest fixture parameter hints | ✅ | ✗ out of scope | — |
| Generic type parameter hints | ✅ | ☐ TODO | 5 |

### Type Checking & Diagnostics

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Missing parameter annotation | ✅ | ✅ DONE | — |
| Missing return annotation | ✅ | ✅ DONE | — |
| Type mismatch / incompatible assignment | ✅ | ✅ partial | — |
| Unknown / unresolved imports | ✅ | ✅ DONE | — |
| Undefined variables | ✅ | ✅ partial | — |
| Full type inference (generics, unions, narrowing) | ✅ | ☐ TODO | 5 |
| `pyrightconfig.json` / `pyproject.toml` config | ✅ | ☐ TODO | 5 |
| Type checking mode (off / basic / strict) | ✅ | strict-only by design | — |
| Stub file (`.pyi`) support | ✅ | ☐ TODO | 5 |
| Third-party type stubs (typeshed, `py.typed`) | ✅ | ☐ TODO | 5 |

### Cross-File & Workspace

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Single-file analysis | ✅ | ✅ DONE | — |
| Cross-file Go to Definition | ✅ | ☐ TODO | 5 |
| Cross-file Find All References | ✅ | ☐ TODO | 5 |
| Cross-file Rename | ✅ | ☐ TODO | 5 |
| Module-level auto-import index | ✅ | ☐ TODO | 5 |
| Multi-root workspace support | ✅ | ☐ TODO | 5 |
| Editable install (`pip install -e`) path resolution | ✅ | ☐ TODO | 5 |
| Jupyter Notebook (`.ipynb`) support | ✅ | ✗ out of scope | — |

### Extension / UX

| Capability | Pylance | Basilisk | Phase |
|------------|---------|---------|-------|
| Status bar (server state + error count) | ✅ | ✅ DONE | **0** |
| Restart Language Server command | ✅ | ✅ DONE | **0** |
| Show Output command | ✅ | ✅ DONE | **0** |
| Auto-restart on crash (max 3 attempts) | ✅ | ✅ DONE | **0** |
| Error message when server fails to start | ✅ | ✅ DONE | **0** |
| Color picker for hex color strings | ✅ | ☐ TODO | 5 |
| AI hover summaries (Copilot integration) | ✅ | ✗ out of scope | — |
| MCP tool exposure (run snippets, import analysis) | ✅ | ✗ out of scope | — |
| REPL terminal completions | ✅ | ✗ out of scope | — |

---

## Testing Strategy

Every LSP feature gets E2E tests in `crates/basilisk-lsp/tests/lsp_e2e_tests.rs`. Tests spawn `basilisk lsp` as a real subprocess and communicate via JSON-RPC. No mocking — test the actual protocol.

| Feature | Test Cases |
|---------|-----------|
| Hover (type) | Hover on function name shows signature; hover on class shows bases; hover on variable shows type; hover on clean code returns info (not None) |
| Go to Definition | F12 on call site returns def_span; F12 on self.attr returns attribute span; F12 on class ref returns class span |
| Document Symbols | File with class+functions returns hierarchical tree; methods nested under classes; empty file returns empty list |
| Signature Help | Inside `greet(` returns signature; after comma selects next param; outside call returns None |
| Find References | All call sites found; definition included when requested; attribute references found |
| Rename | Function rename updates all call sites; class rename updates all references; returns error for non-renameable positions |
| Inlay Hints | Unannotated var gets type hint; call site gets param name hints; annotated var gets no hint |
| Semantic Tokens | Function def classified as function; class def as class; parameter as parameter |

All existing tests must continue to pass. `cargo clippy` must be clean.

---

## Acceptance Criteria

- [ ] `basilisk lsp` starts without Content-Length errors
- [ ] Hovering any symbol shows its type signature
- [ ] Ctrl+Click jumps to definition (same file)
- [ ] Outline panel shows class/function/variable hierarchy
- [ ] Signature help appears when typing function calls
- [ ] Find All References finds all usages in current file
- [ ] F2 rename works across current file
- [ ] Inlay hints show inferred types and parameter names
- [ ] Semantic tokens enhance syntax highlighting
- [ ] Status bar shows server state and error count
- [ ] Restart command recovers from server crashes
- [ ] Format Document delegates to Ruff
- [ ] All E2E tests pass: `cargo test -p basilisk-lsp`
- [ ] `cargo clippy` clean, no `unwrap()` in production code
