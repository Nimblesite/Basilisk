<p align="center">
  <img src="https://basilisk-python.dev/assets/images/favicon.png" alt="Basilisk" width="140">
</p>

<h1 align="center">Basilisk for VS Code</h1>

<p align="center">
  <strong>The open-source Pylance replacement for VS Code.</strong><br>
  Complete language server: diagnostics, autocomplete, hover, go-to-definition,<br>
  refactoring, debugging, profiling. Strict by default. Built in Rust.
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev">Website</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/quick-start/">Quick Start</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/rules/">Rules</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/Nimblesite/Basilisk">GitHub</a>
</p>

<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

---

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in VS Code" width="900">
</p>

## What Basilisk does

Basilisk is a **complete Python language server and VS Code extension** that replaces Pylance and Pyright. It is not just a type checker — it provides autocomplete, go-to-definition, hover, code actions, refactoring, integrated debugging, and profiling. All fully open source.

Other type checkers default to permissive and hope you opt into strictness. Basilisk **starts strict** and stays strict. If your code isn't typed, it's an error — exactly as the screenshot above shows.

Fix it once. Ship it typed forever:

```python
def greet(name: str) -> str:
    return "Hello " + name
```

---

## Features

### Real-time diagnostics

Errors appear inline as you type — powered by the Basilisk LSP server with sub-10ms incremental analysis via the Salsa framework (same tech as rust-analyzer).

### Autocomplete, hover, go-to-definition

Full language intelligence — completions, hover documentation, go-to-definition, find references, rename symbol.

### Code actions and refactoring

Extract function/variable, rename, move symbol, inline, organize imports — all built into the LSP.

### Integrated debugging

Press F5 to debug Python. Basilisk spawns debugpy and brokers the DAP connection — breakpoints, stepping, variable inspection, watch expressions. No separate debug extension needed.

### Integrated profiling

Sample CPU and track memory directly from the editor — run & profile the current file, attach to a running process from the Python Processes panel, or profile the active debug session. Results land as an inline heat map on your source, a flame graph, and a memory dashboard with leak detection. (Sampling uses py-spy on Linux/Windows and an injected in-process sampler on macOS; memory tracking uses tracemalloc via the debugger.)

### Activity panel

The Basilisk sidebar provides two panels accessible from the activity bar:

**Modules** — Browse your workspace's Python module tree with type health folded in. Each module shows a coverage bar, coverage percentage, error/warning tallies, and an `[adopted]` badge, with its icon tinted green/yellow/red by coverage; expand a module to see its top-level symbols (functions, classes, variables) with annotation status. The workspace-wide coverage summary appears in the panel's title (message + numeric badge). Right-click to copy import paths. Toggle between tree and flat views; in flat view, sort by worst-first, best-first, or alphabetical. Filter by glob patterns. While the server is running, the toolbar also offers **Fix All**, **Organize Imports**, and **Restart Server**.

**Basilisk Info** — Feature toggles (type checking, uv integration) plus compact read-only server info (version, analysis mode, Python, uv — with auto-sync and stub-suggestion details in the tooltip — and binary path). The live server state lives in the status bar, whose click opens the Basilisk output log; uv actions (sync/add/lock/create-env) are in the command palette.

Both panels update automatically when files change (debounced 300ms). The Modules panel appears when a workspace is open; the Info panel is always visible.

### Inlay hints

See inferred types and parameter names directly in your editor:

- **Parameter names** at call sites
- **Variable types** for unannotated locals

### Ruff integration

Built-in support for Ruff formatting and import organization. One extension, two tools.

### Single binary, zero dependencies

Basilisk ships as one Rust binary. No Python runtime, no Node.js, no pip, no npm. Install it and go.

---

## Diagnostic rules

All rules are **on by default**. There is no way to relax them globally.

### Annotation rules

| Code | What it catches |
|------|----------------|
| `BSK-E0001` | Parameter has no type annotation |
| `BSK-E0002` | Function missing return type |
| `BSK-E0003` | Variable missing type annotation |
| `BSK-E0004` | `*args` / `**kwargs` not annotated |
| `BSK-E0005` | Class attribute not annotated |

### Type correctness

| Code | What it catches |
|------|----------------|
| `imports_unresolved` | Import from untyped module |
| `returns_compatibility` | Explicit `Any` annotation (warning) |
| `calls_argument_type` | Argument type mismatch |
| `returns_compatibility_2` | Return type mismatch |
| `assignment_compatibility` | Assignment type mismatch |
| `callables_annotation` | Wrong number of type arguments |
| `classes_override` | Incompatible method override |
| `classes_override_2` | Incompatible class variable override |
| `names_undefined` | Undefined name |
| `names_unbound` | Used before assignment |
| `overloads_definitions` | `@overload` missing implementation |
| `overloads_consistency` | Overlapping `@overload` signatures |
| `dict_key_hashable` | Unhashable dict key |
| `match_exhaustiveness` | Non-exhaustive `match` |
| `annotations_typeexpr` | Invalid type expression |
| `BSK-E0025` | Missing `@override` decorator |

---

## How it compares

| | Basilisk | Pyright | mypy |
|---|:---:|:---:|:---:|
| **Strict by default** | Yes | No | No |
| **Written in** | Rust | TypeScript | Python |
| **Runtime needed** | None | Node.js | Python |
| **Incremental speed** | <10ms | ~50ms | ~200ms |
| **Ownership analysis** | Yes | No | No |
| **Single binary** | Yes | No | No |

---

## Extension settings

| Setting | Default | Description |
|---------|---------|-------------|
| `basilisk.enabled` | `true` | Enable/disable the type checker |
| `basilisk.executablePath` | `""` | Explicit path to the basilisk binary. Empty uses the bundled VSIX binary |
| `basilisk.binaries.path` | `""` | Directory containing Basilisk runtime binaries |
| `basilisk.binaries.basilisk` | `""` | Explicit path to the Basilisk language server binary |
| `basilisk.useLsp` | `true` | Use LSP server (disable for subprocess fallback) |
| `basilisk.trace.server` | `"off"` | LSP trace level: `off`, `messages`, `verbose` |
| `basilisk.inlayHints.parameterNames` | `true` | Reserved — hints are always shown; the server does not yet read this |
| `basilisk.inlayHints.variableTypes` | `true` | Reserved — hints are always shown; the server does not yet read this |
| `basilisk.formatter` | `"ruff"` | Formatter engine — `"ruff"` uses the Ruff formatter embedded in the Basilisk binary (in-process; no external `ruff` binary is ever required), `"none"` disables formatting |

---

## Commands

| Command | Description |
|---------|-------------|
| `Basilisk: Restart Language Server` | Restart the LSP server |
| `Basilisk: Show Output` | Open the Basilisk output channel |
| `Basilisk: Organize Imports` | Sort and clean imports via Ruff |
| `Basilisk: Fix File` | Apply all available autofixes to the current file |
| `Basilisk: Adopt File` | Add type annotations to an untyped file |
| `Basilisk: uv sync` | Run uv sync in the workspace |
| `Basilisk: uv add` | Add a package via uv |
| `Basilisk: Refresh Module Explorer` | Refresh the module tree |
| `Basilisk: Toggle Module Explorer View` | Switch between tree and flat view |
| `Basilisk: Toggle Sort Order` | Cycle flat-view sort (worst/best/alpha) |
| `Basilisk: Copy Import Path` | Copy `from x import y` for the selected symbol |
| `Basilisk: Open Walkthrough` | Open the Basilisk getting started walkthrough |

---

## Requirements

None — the Basilisk binary is bundled with this extension for macOS (Apple Silicon), Linux (x86_64 and aarch64), and Windows (x86_64 and aarch64). Install the extension and go.

### Installing the CLI separately

If you also want the `basilisk` CLI on your PATH (for CI, scripting, or terminal use), install it with your platform's package manager:

```bash
# macOS, Linux
brew tap Nimblesite/tap
brew install basilisk
```

```powershell
# Windows
scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket
scoop install basilisk
```

Or download a pre-built binary from [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases).

To make this extension use a CLI you installed separately, set `basilisk.executablePath` or `basilisk.binaries.basilisk` to the absolute path of the binary. Building from source also works — see the [GitHub repository](https://github.com/Nimblesite/Basilisk).

---

## Part of Basilisk

This is the VS Code extension for the [Basilisk](https://github.com/Nimblesite/Basilisk) project. Basilisk also supports [Neovim](https://github.com/Nimblesite/Basilisk/tree/main/basilisk.nvim) and [Zed](https://github.com/Nimblesite/Basilisk/tree/main/basilisk-zed).

## License

MIT.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
