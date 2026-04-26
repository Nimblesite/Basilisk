# Basilisk LSP — Feature Specification

> **Goal**: Compete with Pylance. Every feature that makes a Python IDE useful.

This is the **single source of truth** for all LSP features, DAP integration, custom commands, configuration settings, and binary resolution. Editor-specific specs (VS Code, Zed, Neovim) MUST reference this document rather than duplicating LSP details.

- **VS Code**: `VSIX-SPEC.md`
- **Zed**: `ZED-SPEC.md`
- **Neovim**: `NEOVIM-SPEC.md`
- **uv Integration**: `LSP-UV-INTEGRATION-SPEC.md` — environment detection, lock file intelligence, package commands

⚠️ KEY DESIGN PRINCIPLE: LSP DRIVES THE FUNCTIONALITY - NOT THE IDE EXTENSION
⚠️ IDE EXTENSIONS LISTEN FOR THINGS LIKE COMMANDS FROM THE LSP AND ADJUST ACCORDINGLY
⚠️ THE IDE EXTENSIONS NEVER REGISTERS COMMANDS ETC THAT THE LSP DOESN'T ADVERTISE

---

## Binary Invocation

```bash
basilisk lsp [--transport stdio|ws] [--port 8765]
```

- Default transport: `stdio` (JSON-RPC over stdin/stdout)
- WebSocket transport: `--transport ws --port 8765`
- Logging: `BASILISK_LOG=debug basilisk lsp` (default level: `warn`, written to stderr)

## Binary Resolution Order (all editors)

Every editor extension MUST resolve the `basilisk` binary using this cascade:

1. User-configured path (editor setting)
2. `BASILISK_PATH` environment variable
3. `~/.cargo/bin/basilisk`
4. `/usr/local/bin/basilisk`
5. `/opt/homebrew/bin/basilisk`
6. Fall back to OS PATH search

## Shared Configuration Settings (all editors)

These settings are sent to the LSP server via `workspace/configuration` under the `basilisk` key. Every editor MUST support them:

| Setting Key | Type | Default | Description |
|------------|------|---------|-------------|
| `basilisk.python` | `string` | `""` (auto-detect) | Path to Python interpreter |
| `basilisk.executablePath` | `string` | `""` (auto-detect) | Path to basilisk binary |
| `basilisk.enabled` | `boolean` | `true` | Enable/disable type checker |
| `basilisk.analysisMode` | `enum` | `"wholeModule"` | `openFilesOnly` / `wholeModule` / `crossModule` |
| `basilisk.inlayHints.parameterNames` | `boolean` | `true` | Show parameter name hints at call sites |
| `basilisk.inlayHints.variableTypes` | `boolean` | `true` | Show inferred type hints for unannotated variables |
| `basilisk.ruff.enabled` | `boolean` | `true` | Enable Ruff integration (formatting + import org) |
| `basilisk.ruff.executablePath` | `string` | `"ruff"` | Path to the ruff binary |
| `basilisk.debugger.enabled` | `boolean` | `true` | Enable debugger |
| `basilisk.debugger.typeChecking` | `boolean` | `false` | Enable type assertion breakpoints |
| `basilisk.debugger.debugpyPath` | `string` | `"debugpy"` | Path to debugpy module |
| `basilisk.testExplorer.*` | — | — | See `LSP-TEST-INTEGRATION-SPEC.md` § Configuration Settings |
| `basilisk.uv.enabled` | `boolean` | `true` | Enable uv integration (auto-detected, see `LSP-UV-INTEGRATION-SPEC.md`) |
| `basilisk.uv.executablePath` | `string` | `""` (auto-detect) | Path to `uv` binary (only needed for commands, not detection) |
| `basilisk.uv.autoSync` | `boolean` | `false` | Auto-run `uv sync` when `pyproject.toml` changes |
| `basilisk.uv.stubSuggestions` | `boolean` | `true` | Suggest installing type stub packages |
| `basilisk.uv.dependencyDiagnostics` | `boolean` | `false` | Enable BSK-W0011/W0012/W0013 dependency hygiene warnings |

## Command Registration Rule

⚠️ FOLLOW THIS DOCUMENTATION TO THE LETTER
https://code.visualstudio.com/api/references/vscode-api#commands

**The LSP server is the single source of truth for commands.** The server advertises every command it handles via `executeCommandProvider` in its `initialize` response. This is an ironclad rule:

1. **The server MUST advertise ALL commands** it can handle in `executeCommandProvider.commands`. No exceptions. See `basilisk_common::commands::ALL` in `crates/basilisk-common/src/lib.rs`.
2. **No editor extension may pre-register server commands.** The LSP client library (e.g. `vscode-languageclient`) discovers commands from the server's capabilities and registers them automatically. If an extension also calls `registerCommand()` for the same command, the client crashes with "command already exists".
3. **Client-side UI logic belongs in middleware**, not in `registerCommand()`. For example, VS Code's `executeCommand` middleware injects editor URIs, shows input prompts, and displays toast messages — all without pre-registering the command.
4. **Tests must wait for LSP readiness** before asserting command availability. Server-advertised commands only exist after the LSP handshake completes.
5. **Client-only commands** (e.g. `restartServer`, `showOutput`) that have no server-side handler ARE registered client-side. These are NOT in `executeCommandProvider`.

This rule applies equally to VS Code, Neovim, and Zed extensions.

---

## Custom LSP Commands (`workspace/executeCommand`)

| Command | Arguments | Response | Description |
|---------|-----------|----------|-------------|
| `basilisk.organizeImports` | `{uri}` | `TextEdit[]` | Run Ruff import organization |
| `basilisk/startDebugSession` | `{uri, pythonPath?}` | `{host, port, sessionId}` | Spawn debugpy, return connection info |
| `basilisk/stopDebugSession` | `{sessionId}` | `{}` | Terminate debug session |
| `basilisk/profiler/start` | `{pid?}` | `{sessionId}` | Start profiling (active process or PID) |
| `basilisk/profiler/stop` | `{sessionId}` | `{results}` | Stop profiling, return results |
| `basilisk/profiler/snapshot` | `{sessionId}` | `{results}` | Snapshot without stopping |
| `basilisk/memory/start` | `{}` | `{sessionId}` | Start memory leak tracking |
| `basilisk/memory/stop` | `{sessionId}` | `{leakReport}` | Stop tracking, return leak report |
| `basilisk/memory/refs` | `{typeName}` | `{retentionPaths}` | Query retention paths for a type |
| `basilisk.uv.sync` | `{}` | `{}` | Run `uv sync` in project root (see `LSP-UV-INTEGRATION-SPEC.md`) |
| `basilisk.uv.add` | `{package}` | `{}` | Run `uv add <package>` |
| `basilisk.uv.addDev` | `{package}` | `{}` | Run `uv add --dev <package>` |
| `basilisk.uv.remove` | `{package}` | `{}` | Run `uv remove <package>` |
| `basilisk.uv.lock` | `{}` | `{}` | Run `uv lock` (resolve without installing) |
| `basilisk.uv.createEnv` | `{pythonVersion?}` | `{}` | Run `uv venv` (optionally `--python X.Y`) |
| `basilisk/workspaceModules` | `{scope?: string}` | `WorkspaceModulesResponse` | Return the workspace module tree (optionally scoped to a package/subpackage) |
| `basilisk/typeHealth` | `{module?: string}` | `TypeHealthResponse` | Return type health statistics for the workspace or a specific module |

### Custom LSP Notifications

| Notification | Direction | Params | Description |
|-------------|-----------|--------|-------------|
| `basilisk/moduleChanged` | Server → Client | `{module: ModuleNode}` | Sent when a module's symbol table changes after re-analysis. Debounced at 300ms. |

### Data Model Types

```typescript
/** A node in the workspace module tree. */
interface ModuleNode {
    name: string;              // Fully qualified module name (e.g. "mypackage.utils")
    path: string;              // Absolute filesystem path to the module file or __init__.py
    kind: "package" | "module";
    children: ModuleNode[];    // Sub-modules (non-empty only for packages)
    symbols: SymbolNode[];     // Top-level symbols exported by this module
}

/** A symbol within a module (function, class, or variable). */
interface SymbolNode {
    name: string;
    kind: "function" | "class" | "variable";
    type: string | null;       // Inferred or annotated type signature, null if unresolved
    line: number;              // 0-based line number of the definition
    children: SymbolNode[];    // Nested symbols (e.g. methods inside a class)
}

/** Response from `basilisk/workspaceModules`. */
interface WorkspaceModulesResponse {
    modules: ModuleNode[];
}

/** Aggregate health statistics for a scope (workspace or single module). */
interface HealthStats {
    totalSymbols: number;      // Total symbols in scope
    typedSymbols: number;      // Symbols with a resolved type annotation
    coveragePercent: number;   // (typedSymbols / totalSymbols) * 100, 0 when totalSymbols == 0
    errorCount: number;        // Number of BSK-E* diagnostics
    warningCount: number;      // Number of BSK-W* diagnostics
}

/** Per-module health breakdown. */
interface ModuleHealth {
    module: string;            // Fully qualified module name
    stats: HealthStats;
}

/** Response from `basilisk/typeHealth`. */
interface TypeHealthResponse {
    workspace: HealthStats;    // Rolled-up stats for the entire workspace
    modules: ModuleHealth[];   // Per-module breakdown (all modules, or single module when filtered)
}
```

## DapTcpProxy (all editors)

All editors MUST implement a TCP proxy between the DAP client and debugpy to fix known stepping quirks:

1. Listen on a random local port
2. Connect to debugpy on the `{host, port}` returned by `basilisk/startDebugSession`
3. Frame DAP messages with `Content-Length` headers
4. **Intercept `stepOut`** — inject auto-`next` for structural lines (`try:`, `with:`, `if:`)
5. **Attach mode timeout** — 3s timeout with synthetic success response
6. **Inject `exited` event** before `terminated` if missing
7. **Fast disconnect** — respond immediately post-termination

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

| BSK-E0010 (uv) | Add dependency | `uv add <package>` via `basilisk.uv.add` command |
| BSK-W0010 (uv) | Install type stubs | `uv add --dev types-<package>` via `basilisk.uv.addDev` command |
| BSK-W0011 (uv) | Add dependency | `uv add <package>` — transitive dep used directly |
| BSK-W0013 (uv) | Sync environment | `uv sync` via `basilisk.uv.sync` command |

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

## uv Integration Architecture

See [LSP-UV-INTEGRATION-SPEC.md](LSP-UV-INTEGRATION-SPEC.md) for the full specification. Key architectural details:

### Detection & Registry

On startup, the LSP detects uv projects via filesystem signals (`uv.lock`, `[tool.uv]` in `pyproject.toml`, `.venv/pyvenv.cfg` with `uv = true`). If detected:

1. Parse `uv.lock` → `LockFile` (TOML, zero subprocess calls)
2. Extract `[project].dependencies` from `pyproject.toml` (PEP 508 specifier parsing)
3. Build `PackageRegistry` — HashMap keyed by normalised import name, classifying each package as `Direct`, `Dev`, or `Transitive`
4. Discover `[tool.uv.workspace]` members → add source roots to import search paths

The registry feeds into:
- **Import resolution**: `PackageDepKind` (Direct/Dev/Transitive) annotated on each `ImportInfo`
- **Diagnostics**: BSK-W0011 fires for transitive dependency imports
- **Hover**: shows dependency classification and workspace member status
- **Code actions**: `uv add`, `uv add --dev`, `uv sync` quick fixes

### Hot Reload

All uv commands trigger `rebuild_registry_and_resolve()` on success — re-parses `uv.lock`, rebuilds the registry, re-resolves all imports, and republishes diagnostics for every indexed file. The same function fires when the file watcher detects `uv.lock` or `pyproject.toml` changes. No LSP restart needed.

### uv Binary Resolution

| Priority | Source |
|----------|--------|
| 1 | `basilisk.uv.executablePath` setting |
| 2 | `UV_PATH` environment variable |
| 3 | `~/.cargo/bin/uv` |
| 4 | `~/.local/bin/uv` |
| 5 | OS PATH search |

The uv binary is only needed for **commands** (sync, add, remove). Lock file parsing and environment detection are pure filesystem operations.

### uv Diagnostic Codes

| Code | Severity | Default | Gate | Description |
|------|----------|---------|------|-------------|
| BSK-W0010 | Warning | Enabled | `uv.stubSuggestions` | Package installed but no type stubs |
| BSK-W0011 | Warning | Disabled | `uv.dependencyDiagnostics` | Import of transitive dependency not in `[project.dependencies]` |
| BSK-W0012 | Info | Disabled | `uv.dependencyDiagnostics` | Declared dependency never imported (whole-module only, skeleton) |
| BSK-W0013 | Warning | Disabled | `uv.dependencyDiagnostics` | `uv.lock` older than `pyproject.toml` (skeleton) |

---

## Stub Resolution & Type Provenance

See [CHECKER-STUB-RESOLUTION-SPEC.md](CHECKER-STUB-RESOLUTION-SPEC.md) for PEP 561 resolution order, typeshed bundling, type provenance tracking, suppression system, and auto-stub generation.

---

## Analysis Modes

See [LSP-ANALYSIS-MODES-SPEC.md](LSP-ANALYSIS-MODES-SPEC.md) for `openFilesOnly` / `wholeModule` / `crossModule` modes, workspace index, import graph, and cross-file LSP features.

---

## Editor-Specific Specs

For editor-specific implementation details (commands, UI, configuration schema, DAP proxy implementation), see:

- **VS Code**: [`VSIX-SPEC.md`](VSIX-SPEC.md)
- **Zed**: [`ZED-SPEC.md`](ZED-SPEC.md)
- **Neovim**: [`NEOVIM-SPEC.md`](NEOVIM-SPEC.md)

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
