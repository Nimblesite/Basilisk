<p align="center">
  <img src="images/basilisk-logo.png" alt="Basilisk" width="140">
</p>

<h1 align="center">Basilisk for VS Code</h1>

<p align="center">
  <strong>Strict-by-default Python type checking. No escape hatches.</strong><br>
  Every parameter typed. Every return declared. <code>Any</code> is always explicit.
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev">Website</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/quick-start/">Quick Start</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/rules/">Rules</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/MelbourneDeveloper/Basilisk">GitHub</a>
</p>

---

## What Basilisk does

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

### Inlay hints

See inferred types and parameter names directly in your editor:

- **Parameter names** at call sites
- **Variable types** for unannotated locals

### Rustc-style error output

Clear, actionable diagnostics with source spans, help text, and direct links to rule documentation. No cryptic messages.

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
| `basilisk.executablePath` | `"basilisk"` | Path to the basilisk binary |
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

---

## Installation

**No manual setup required.** When you first activate the extension, it detects whether the `basilisk` binary is on your PATH. If not, it offers to download the correct pre-built binary for your platform directly from [GitHub Releases](https://github.com/MelbourneDeveloper/Basilisk/releases).

The downloaded binary is stored in the extension's global storage directory — no system paths are modified.

### Supported platforms

| OS | Architecture |
|----|-------------|
| macOS | Apple Silicon (aarch64) |
| macOS | Intel (x86_64) |
| Linux | x86_64 |
| Linux | aarch64 |
| Windows | x86_64 |

### Manual install (optional)

If you prefer to manage the binary yourself:

```sh
# Download from GitHub Releases
# https://github.com/MelbourneDeveloper/Basilisk/releases

# Or build from source (Rust 1.87+)
cargo install basilisk
```

Then set `basilisk.executablePath` in your VS Code settings if it's not on PATH.

---

## License

MIT License. Copyright (c) 2026 NIMBLESITE PTY LTD.
