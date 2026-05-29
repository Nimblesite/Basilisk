<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/vscode-extension/images/basilisk-logo.png" alt="Basilisk" width="140">
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

---

## What Basilisk does

Basilisk is a **complete Python language server and VS Code extension** that replaces Pylance and Pyright. It is not just a type checker — it provides autocomplete, go-to-definition, hover, code actions, refactoring, integrated debugging, and profiling. All fully open source.

Other type checkers default to permissive and hope you opt into strictness. Basilisk **starts strict** and stays strict. If your code isn't typed, it's an error.

```
error[BSK-E0001]: Missing parameter type annotation for `name`
  --> greet.py:1:10
   |
 1 | def greet(name):
   |           ^^^^
   |
   = help: Add a type annotation: `name: <type>`

error[BSK-E0002]: Missing return type annotation
  --> greet.py:1:1
   |
 1 | def greet(name):
   | ^^^^^^^^^^^^^^^^
```

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

Profile Python code with py-spy directly from the editor. View heatmaps and identify bottlenecks without leaving VS Code.

### Activity panel

The Basilisk sidebar provides three panels accessible from the activity bar:

**Module Explorer** — Browse your workspace's Python module tree. Each module shows its top-level symbols (functions, classes, variables) with type annotation status. Right-click to copy import paths. Toggle between tree and flat views. Filtered by glob patterns.

**Type Health** — Workspace-wide type coverage dashboard. Shows annotated vs unannotated symbol counts, error/warning tallies, and per-module coverage percentages with color-coded progress bars. Sort by worst-first, best-first, or alphabetical.

**Basilisk Info** — Feature status toggles (type checking, inlay hints, Ruff, debugger, test explorer, uv), quick actions (restart server, organize imports), and server info (version, binary path, analysis mode).

All panels update automatically when files change (debounced 300ms). The Module Explorer and Type Health panels appear when a workspace is open; the Info panel is always visible.

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
| `BSK-E0010` | Import from untyped module |
| `BSK-E0011` | Implicit `Any` |
| `BSK-E0012` | Argument type mismatch |
| `BSK-E0013` | Return type mismatch |
| `BSK-E0014` | Assignment type mismatch |
| `BSK-E0015` | Wrong number of type arguments |
| `BSK-E0016` | Incompatible method override |
| `BSK-E0017` | Incompatible class variable override |
| `BSK-E0018` | Undefined name |
| `BSK-E0019` | Used before assignment |
| `BSK-E0020` | `@overload` missing implementation |
| `BSK-E0021` | Overlapping `@overload` signatures |
| `BSK-E0022` | Unhashable dict key |
| `BSK-E0023` | Non-exhaustive `match` |
| `BSK-E0024` | Invalid type expression |
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
| `basilisk.inlayHints.parameterNames` | `true` | Show parameter name hints at call sites |
| `basilisk.inlayHints.variableTypes` | `true` | Show inferred types for unannotated variables |
| `basilisk.ruff.enabled` | `true` | Enable Ruff integration |
| `basilisk.ruff.executablePath` | `"ruff"` | Path to the ruff binary |

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
| `Basilisk: Copy Import Path` | Copy `from x import y` for the selected symbol |
| `Basilisk: Refresh Type Health` | Refresh type coverage stats |
| `Basilisk: Sort Type Health` | Cycle sort order (worst/best/alpha) |
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

To make this extension use a CLI you installed separately, set `basilisk.executablePath` or `basilisk.binaries.basilisk` to the absolute path of the binary. Building from source also works (`cargo build --release` from the repository — Rust 1.87+ required).

---

## Part of Basilisk

This is the VS Code extension for the [Basilisk](https://github.com/Nimblesite/Basilisk) project. Basilisk also supports [Neovim](https://github.com/Nimblesite/Basilisk/tree/main/basilisk.nvim) and [Zed](https://github.com/Nimblesite/Basilisk/tree/main/basilisk-zed).

## License

MIT or Apache-2.0, at your option.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
