# Basilisk VS Code Extension

## Goal

A first-class VS Code extension that connects to the `basilisk lsp` binary. The primary integration. Open source. No Microsoft proprietary dependencies.

**CRITICAL: AIMING FOR FEATURE PARITY BETWEEN VS CODE, ZED, AND NEOVIM EXTENSIONS**

All LSP features, DAP integration, custom commands, configuration settings, and binary resolution are defined in **`LSP-SPEC.md`** — the single source of truth. This spec only documents **VS Code-specific implementation details**.

## Critical Docs

- [VS Code Extension API](https://code.visualstudio.com/api)
- [VS Code Language Extensions](https://code.visualstudio.com/api/language-extensions/overview)
- [VS Code Testing API](https://code.visualstudio.com/api/extension-guides/testing)
- [VS Code Debug Adapter Protocol](https://code.visualstudio.com/api/extension-guides/debugger-extension)
- [Debug Adapter Protocol Specification](https://microsoft.github.io/debug-adapter-protocol/)

---

## Architecture

```
┌──────────────────────────┐         ┌──────────────────────────┐
│   VS Code                │         │  basilisk binary (Rust)  │
│                          │         │                          │
│  LanguageClient          │◄───────►│  basilisk lsp            │
│  (vscode-languageclient) │  stdio  │  (JSON-RPC)              │
│                          │  JSON   │                          │
│  All 21 LSP features     │  RPC    │  See LSP-SPEC.md   │
│  via LanguageClient      │         │  for all features        │
│                          │         │                          │
│  DAP Proxy (TypeScript)  │◄───────►│  debugpy (spawned by     │
│  DebugAdapterInline      │  TCP    │  basilisk/startDebug)    │
│                          │         │                          │
│  TestController          │         │                          │
│  Status Bar Item         │         │                          │
│  Output Channels         │         │                          │
└──────────────────────────┘         └──────────────────────────┘
```

- VSIX bundles a pre-compiled LSP server binary per platform
- No Node.js dependency for the server (the extension activation layer uses VS Code's extension API)
- Configuration exposed via VS Code settings with JSON schema validation

---

## Extension Structure

```
vscode-extension/
├── src/
│   ├── extension.ts        # Activation, LanguageClient setup, command registration
│   └── dap-proxy.ts        # DebugAdapterProxy (TypeScript, in-process)
├── package.json            # Commands, settings, keybindings, debugger contribution
├── tsconfig.json
└── .vscode-test.mjs
```

---

## LSP Client Configuration

> See `LSP-SPEC.md` for all LSP features, custom commands, and shared configuration settings.

```typescript
const serverOptions: ServerOptions = {
  command: resolvedBinaryPath,  // resolved via binary resolution cascade (LSP-SPEC.md)
  args: ["lsp"],
};

const clientOptions: LanguageClientOptions = {
  documentSelector: [{ scheme: "file", language: "python" }],
  synchronize: { configurationSection: "basilisk" },
  initializationOptions: readBasiliskSettings(),
};

client = new LanguageClient("basilisk", "Basilisk Type Checker", serverOptions, clientOptions);
client.start();
```

---

## Commands (`package.json` contribution)

```json
"commands": [
    { "command": "basilisk.restartServer", "title": "Basilisk: Restart Language Server" },
    { "command": "basilisk.showOutput", "title": "Basilisk: Show Output" },
    { "command": "basilisk.organizeImports", "title": "Basilisk: Organize Imports" },
    { "command": "basilisk.runTests", "title": "Basilisk: Run Tests" },
    { "command": "basilisk.runTestFile", "title": "Basilisk: Run Tests in Current File" },
    { "command": "basilisk.debugTest", "title": "Basilisk: Debug Test" },
    { "command": "basilisk.debugFile", "title": "Basilisk: Debug Current File" },
    { "command": "basilisk.toggleTypeBreakpoints", "title": "Basilisk: Toggle Type Mismatch Breakpoints" }
]
```

---

## Configuration Settings (`package.json` contribution)

> Shared settings (sent to LSP server) are defined in `LSP-SPEC.md` § Shared Configuration Settings. Below is their `package.json` schema representation plus VS Code-only settings.

```json
{
    "basilisk.executablePath": {
        "type": "string", "default": "",
        "description": "Path to the basilisk binary. Leave empty to auto-detect."
    },
    "basilisk.python": {
        "type": "string", "default": "",
        "description": "Path to the Python interpreter. Leave empty to auto-detect."
    },
    "basilisk.enabled": {
        "type": "boolean", "default": true,
        "description": "Enable/disable the Basilisk type checker."
    },
    "basilisk.useLsp": {
        "type": "boolean", "default": true,
        "description": "Use LSP mode (true) or subprocess mode (false)."
    },
    "basilisk.analysisMode": {
        "type": "string",
        "enum": ["openFilesOnly", "wholeModule", "crossModule"],
        "default": "wholeModule",
        "description": "Analysis scope."
    },
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
    },
    "basilisk.trace.server": {
        "type": "string",
        "enum": ["off", "messages", "verbose"],
        "default": "off",
        "description": "Trace LSP communication."
    },
    "basilisk.testExplorer.enabled": {
        "type": "boolean", "default": true,
        "description": "Enable Python test discovery and execution in Test Explorer."
    },
    "basilisk.testExplorer.framework": {
        "type": "string",
        "enum": ["pytest", "unittest", "auto"],
        "default": "auto",
        "description": "Test framework to use. 'auto' detects from project config."
    },
    "basilisk.testExplorer.pytestPath": {
        "type": "string", "default": "pytest",
        "description": "Path to the pytest executable."
    },
    "basilisk.testExplorer.args": {
        "type": "array",
        "items": { "type": "string" },
        "default": [],
        "description": "Additional arguments passed to the test runner."
    },
    "basilisk.testExplorer.autoDiscoverOnSave": {
        "type": "boolean", "default": true,
        "description": "Re-discover tests when test files are saved."
    },
    "basilisk.debugger.enabled": {
        "type": "boolean", "default": true,
        "description": "Enable Basilisk Python debugger."
    },
    "basilisk.debugger.typeChecking": {
        "type": "boolean", "default": false,
        "description": "Enable type assertion breakpoints during debugging."
    },
    "basilisk.debugger.debugpyPath": {
        "type": "string", "default": "debugpy",
        "description": "Path to the debugpy module."
    }
}
```

### VS Code-Only Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `basilisk.useLsp` | `true` | Use LSP mode vs subprocess fallback |
| `basilisk.trace.server` | `"off"` | LSP communication trace level |

All other settings are shared across editors (see `LSP-SPEC.md`).

---

## Status Bar

Persistent item showing server state and diagnostic count:
- `$(check) Basilisk` — green, server running, no errors
- `$(warning) Basilisk (3)` — errors in current file
- `$(error) Basilisk` — server failed/not running
- `$(sync~spin) Basilisk` — analyzing

Additional indicators (future):
- Type completeness indicator: `"87% typed"`
- Migration dashboard (sidebar panel)
- Ownership visualization (gutter icons: borrowed/owned/inout)

---

## Error Recovery

- `errorHandler` on `LanguageClient` for auto-restart (max 3 attempts, exponential backoff)
- User-visible error message when server fails to start
- `basilisk.restartServer` command for manual recovery
- Subprocess fallback mode when `basilisk.useLsp` is `false`

---

## Test Explorer Integration

> See `LSP-SPEC.md` § Shared Configuration Settings for test explorer settings shared across all editors.

Discover and run Python tests (pytest, unittest) directly from VS Code's Test Explorer.

**Architecture**:
- Implement `TestController` via VS Code's `vscode.tests` API
- Parse Python test files to discover test functions, classes, and methods
- Use the resolver to find `def test_*` functions, classes inheriting `unittest.TestCase`, and `@pytest.mark` decorated items
- Execute tests via `pytest` subprocess (similar to how formatting delegates to `ruff`)
- Stream results back to Test Explorer as pass/fail/skip/error

**Test Discovery**:
- Scan workspace for `test_*.py` and `*_test.py` files
- Parse with `basilisk-parser` to extract test items without importing
- Detect pytest fixtures, parametrize markers, and unittest setUp/tearDown
- Auto-refresh on file save

**Test Item Hierarchy**:
```
tests/
    test_api.py
        test_login
        test_signup -- AssertionError: expected 200, got 401
        TestUserEndpoints
            test_get_user
            test_delete_user
            test_update_user
    test_models.py
        test_create_widget
        test_slow_query (skipped)
```

**Features**:
- **Auto-discovery**: finds pytest and unittest tests from AST (no import needed)
- **Run/debug individual tests**: click play on any test function or class
- **Run all**: run entire test suite from Test Explorer root
- **Inline failure messages**: show assertion errors and tracebacks inline
- **Go to test**: click any test item to navigate to its source
- **Re-run failed**: quick action to re-run only failed tests
- **pytest integration**: honours `pytest.ini`, `pyproject.toml [tool.pytest]`, conftest fixtures
- **Type-checked tests**: Basilisk diagnostics run on test files too
- **Coverage overlay**: integrate with `pytest-cov` to show coverage gutters

---

## Python Debugger (DAP)

> See `LSP-SPEC.md` § Custom LSP Commands for `basilisk/startDebugSession` and `basilisk/stopDebugSession`.
> See `LSP-SPEC.md` § DapTcpProxy for the shared proxy specification that all editors implement.

### VS Code-Specific DAP Architecture

```
VS Code  <-->  DAP Proxy (TypeScript, in-process)  <-->  debugpy.adapter (TCP)
                   |
          DebugAdapterInlineImplementation
```

The LSP server spawns `debugpy.adapter --port <free-port>` via `basilisk/startDebugSession`. The proxy connects to that port and relays DAP messages bidirectionally, intercepting specific message patterns.

### Debug Adapter Proxy (VS Code Implementation)

The proxy (`vscode-extension/src/dap-proxy.ts`) implements `vscode.DebugAdapter` via `DebugAdapterInlineImplementation`. It fixes four debugpy quirks:

**Quirk 1 -- stepOut lands before assignment**:
After `stepOut`, debugpy stops at the call-site line *before* the return value is assigned. The proxy detects the `stepOut` response -> first `stopped` event sequence and injects an automatic `next` request, swallowing the intermediate stop.

**Quirk 2 -- Structural line stops during stepOver**:
debugpy stops on `try:` lines during `next` (stepOver). The proxy inspects each post-step stop by requesting a `stackTrace`, reading the source, and checking if the stopped line matches `/^\s*(try\s*:)\s*(#.*)?$/`. If so, it injects another `next`. `except:` and `finally:` lines are NOT skipped.

**Quirk 3 -- Single-connection slot protection**:
`debugpy.adapter --port` accepts exactly one TCP connection. The proxy's bind-based `isPortAlive` check (attempt to bind, `EADDRINUSE` = alive) is non-destructive. In attach mode, if the port is dead, the factory respawns debugpy via the LSP.

**Quirk 4 -- Session termination timing**:
VS Code's `activeDebugSession` may not be cleared when `onDidTerminateDebugSession` fires. The proxy ensures the `exited` event is sent before `terminated`, with a minimal delay.

### DAP Features

> See `LSP-SPEC.md` § DapTcpProxy for the full shared feature list.

| Feature | Description |
|---------|-------------|
| Launch & Attach | Launch Python scripts or attach to running processes |
| Breakpoints | Line, conditional, logpoint, function, exception breakpoints |
| Step execution | Step in, step over, step out, continue, pause |
| Variable inspection | View locals, globals, closures with full type info from Basilisk |
| Watch expressions | Evaluate expressions in the current scope |
| Call stack | Full call stack with source navigation |
| Type-aware hover | Hover shows both runtime value AND static type (from LSP) |
| Type assertions | Break when a runtime type doesn't match the static annotation |

**Type-aware debugging** (unique to Basilisk):
- **Type mismatch breakpoints**: automatically break when a variable's runtime type doesn't match its annotation
- **Annotation overlay**: debug hover shows `(static: str, runtime: str)` side-by-side
- **Type narrowing visualization**: show which branch of a union type is active at a breakpoint
- **Parameter contract verification**: warn when a function receives a value that violates its annotation at runtime

### Launch Configurations

```json
{
    "type": "basilisk",
    "request": "launch",
    "name": "Basilisk: Run Current File",
    "program": "${file}",
    "python": "${command:python.interpreterPath}",
    "args": [],
    "env": {},
    "console": "integratedTerminal",
    "typeChecking": true
}
```

```json
{
    "type": "basilisk",
    "request": "attach",
    "name": "Basilisk: Attach to Process",
    "connect": { "host": "localhost", "port": 5678 },
    "typeChecking": true
}
```

---

## Binary Resolution

> See `LSP-SPEC.md` § Binary Resolution Order for the shared cascade.

VS Code-specific resolution order:
1. VS Code setting: `basilisk.executablePath`
2. Environment variable: `BASILISK_EXECUTABLE_PATH`
3. Well-known locations: `~/.cargo/bin/basilisk`, `/usr/local/bin/basilisk`, `/opt/homebrew/bin/basilisk`
4. Fall back to bare `"basilisk"` on PATH

---

## Output Channels

- `"Basilisk"` — main output channel for server messages
- `"Basilisk LSP Trace"` — LSP communication trace (when `basilisk.trace.server` is enabled)
- File log sink: `/tmp/basilisk-debug-trace.log` for debug-level logging

---

## Binary Distribution

The VSIX bundles pre-compiled `basilisk` binaries per platform:
- `basilisk-x86_64-apple-darwin`
- `basilisk-aarch64-apple-darwin`
- `basilisk-x86_64-unknown-linux-gnu`
- `basilisk-aarch64-unknown-linux-gnu`
- `basilisk-x86_64-pc-windows-msvc`
