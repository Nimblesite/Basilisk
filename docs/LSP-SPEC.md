# Basilisk LSP & VSIX — Feature Specification

> **Goal**: Compete with Pylance. Every feature that makes a Python IDE useful.

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

### Server Module Structure

```
crates/basilisk-lsp/src/
  server.rs          — LspServer struct + tower-lsp trait impl (dispatch only)
  lib.rs             — re-exports run_server, check_source
  util.rs            — find_symbol_at_offset, position conversion, format_type_signature
  hover.rs           — type-aware hover + diagnostic hover
  definition.rs      — go-to-definition
  references.rs      — find all references + rename
  symbols.rs         — document symbols + workspace symbols
  completion.rs      — symbol + dot + import + kwarg completions
  signature.rs       — signature help
  inlay_hints.rs     — inferred types, parameter names, return types
  semantic_tokens.rs — token classification
  code_actions.rs    — quick fixes (E0001-E0003, suppress, organize imports)
  formatting.rs      — Ruff delegation
  highlight.rs       — document highlight (symbol occurrences)
  call_hierarchy.rs  — incoming/outgoing call navigation
  type_hierarchy.rs  — supertype/subtype navigation
  code_lens.rs       — reference count lenses
  folding.rs         — folding ranges
  selection.rs       — selection ranges (Smart Select)
```

Each module exports pure functions: `(resolved: &ResolvedModule, source: &str, ...) → LSP response type`.

### Performance: Cache ResolvedModule

```rust
struct DocumentState {
    text: String,
    resolved: Option<Arc<ResolvedModule>>,  // cached from last did_change/did_open
    diagnostics: Vec<Diagnostic>,
}
```

Update `resolved` on `did_change`/`did_open`. Reuse cached result for all feature handlers.

---

## LSP Features

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

Also: `pub fn format_type_signature(hit: &SymbolHit, source: &str) -> String` — builds hover markdown for any symbol kind.

### Hover (`textDocument/hover`)

Show type signatures for any symbol, with diagnostics as secondary:

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

### Go to Definition (`textDocument/definition`)

Ctrl+Click / F12 on a symbol jumps to its definition.

| Cursor on | Jumps to |
|-----------|----------|
| Function call `greet(...)` | `FunctionInfo.name_span` of `greet` |
| Class instantiation `Dog(...)` | `ClassInfo.name_span` of `Dog` |
| Variable reference `x` | `VariableInfo.name_span` of `x` |
| `self.attr` | `AttributeInfo.name_span` in enclosing class |
| `ClassName.attr` | `AttributeInfo.name_span` in named class |
| Parameter reference | `ParameterInfo.name_span` in function |

Single-file scope. Cross-module requires workspace module resolver.

### Document Symbols (`textDocument/documentSymbol`)

Hierarchical outline tree:

```
▼ MyClass                          (class)
    name: str                      (field)
    age: int                       (field)
    ▶ __init__(self, name, age)    (method)
    ▶ greet(self) -> str           (method)
▶ helper(x: int) -> int           (function)
  MAX_SIZE: int                    (variable)
```

### Signature Help (`textDocument/signatureHelp`)

Trigger on `(` and `,`. Shows parameter hints with active parameter tracking. Skips `self`/`cls` for methods.

### Find All References (`textDocument/references`)

Whole-word text scan with word boundary checks, filtering strings/comments. Respects `include_declaration`.

### Rename Symbol (`textDocument/prepareRename` + `textDocument/rename`)

Validates symbol is renameable, returns `WorkspaceEdit` with `TextEdit` for each occurrence. Single-file scope.

### Completion (`textDocument/completion`)

- **Symbol completions**: functions, classes, variables from resolved module
- **Dot-access completions**: `self.attr`, `ClassName.attr` — class members
- **Import completions**: module names from import statements
- **Builtin completions**: 78 Python builtins (functions, constants, exceptions)
- **Keyword argument completions**: `param_name=` inside function calls

### Code Actions (`textDocument/codeAction`)

| Diagnostic | Action | Transformation |
|-----------|--------|----------------|
| BSK-E0001 | Add parameter annotation | Insert `: Any` |
| BSK-E0002 | Add return annotation | Insert `-> None` |
| BSK-E0003 | Add variable annotation | Insert `: <inferred_type>` |
| (any) | Suppress with `# type: ignore` | Append comment to line |
| (source) | Organize imports | Delegate to `ruff check --select I --fix` |

Register `codeActionKinds`: `[QUICKFIX, SOURCE_ORGANIZE_IMPORTS, REFACTOR]`

### Execute Command (`workspace/executeCommand`)

- `basilisk.organizeImports` — run Ruff import organization on a document

### Inlay Hints (`textDocument/inlayHint`)

1. **Variable type hints** — unannotated variables, inferred type at `name_span.end`
2. **Parameter name hints** — call sites, `"param_name="` at arg span start
3. **Function return type hints** — inferred from `return_stmts[].rhs_kind`, positioned after closing `)`

### Semantic Tokens (`textDocument/semanticTokens/full`)

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

### Document Highlight (`textDocument/documentHighlight`)

Highlight all occurrences of symbol under cursor. Definition = WRITE, usages = READ.

### Workspace Symbols (`workspace/symbol`)

Ctrl+T symbol search across all open documents. Aggregates from DashMap, filters by query.

### Format Document (`textDocument/formatting`)

Spawn `ruff format --stdin-filename <path> -` with document text on stdin. Return single `TextEdit` replacing entire document.

### Folding Ranges (`textDocument/foldingRange`)

Emit `FoldingRange` for: function `def_span`, class `def_span`, consecutive import blocks.

### Selection Ranges (`textDocument/selectionRange`)

Smart Select: identifier → parameter → parameter list → function → class → module. Nested range tree from `ResolvedModule` spans.

### Call Hierarchy (`textDocument/prepareCallHierarchy` + incoming/outgoing)

- **Prepare**: Find function/class at cursor, return `CallHierarchyItem`
- **Incoming**: Find all `CallSite`s where `callee == name`, group by enclosing function
- **Outgoing**: Find all `CallSite`s within function's `def_span`

### Type Hierarchy (`textDocument/prepareTypeHierarchy` + supertypes/subtypes)

- **Prepare**: Find `ClassInfo` at cursor
- **Supertypes**: Use `ClassInfo.bases` to find parent classes
- **Subtypes**: Find classes whose `bases` contains target class name

### Code Lens (`textDocument/codeLens`)

Show "N references" above each function and class definition.

---

## VS Code Extension

### Commands

```json
"commands": [
    { "command": "basilisk.restartServer", "title": "Basilisk: Restart Language Server" },
    { "command": "basilisk.showOutput", "title": "Basilisk: Show Output" },
    { "command": "basilisk.organizeImports", "title": "Basilisk: Organize Imports" }
]
```

### Configuration Settings

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

### Status Bar

Persistent item showing server state and diagnostic count:
- `$(check) Basilisk` — green, server running, no errors
- `$(warning) Basilisk (3)` — errors in current file
- `$(error) Basilisk` — server failed/not running
- `$(sync~spin) Basilisk` — analyzing

### Error Recovery

- `errorHandler` on `LanguageClient` for auto-restart (max 3 attempts)
- User-visible error message when server fails to start
- `basilisk.restartServer` command for manual recovery

---

## Testing Strategy

Every LSP feature gets E2E tests in `crates/basilisk-lsp/tests/lsp_e2e_tests.rs` and WS tests in `crates/basilisk-lsp/tests/lsp_ws_tests.rs`. No mocking — test the actual protocol.

| Feature | Test Cases |
|---------|-----------|
| Hover (type) | Hover on function name shows signature; hover on class shows bases; hover on variable shows type |
| Go to Definition | F12 on call site returns def_span; F12 on class ref returns class span |
| Document Symbols | File with class+functions returns hierarchical tree; methods nested under classes |
| Signature Help | Inside `greet(` returns signature; after comma selects next param; outside call returns None |
| Find References | All call sites found; definition included when requested |
| Rename | Function rename updates all call sites; class rename updates all references |
| Inlay Hints | Unannotated var gets type hint; call site gets param name hints; return type hints |
| Semantic Tokens | Function def classified as function; class def as class; parameter as parameter |
| Code Actions | E0001/E0002/E0003 quick fixes; suppress; organize imports |
| Completion | Symbol, dot, kwarg, builtin completions |

All tests must pass. `cargo clippy` must be clean. No `.unwrap()` in production code.
