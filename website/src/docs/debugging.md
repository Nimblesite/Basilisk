---
layout: layouts/docs.njk
title: Debugging
description: Integrated Python debugging with Basilisk — set breakpoints, step through code, inspect variables, and evaluate expressions. No separate debug extension needed.
keywords: basilisk, debugging, python, debugpy, breakpoints, step through, variables, watch, debug console, vs code, zed, dap
eleventyNavigation:
  key: Debugging
  order: 4
---

# Debugging

![PLACEHOLDER: VS Code debug session showing breakpoint hit with Basilisk — Variables pane, call stack, and debug toolbar visible](debugging-vscode-overview.png)

Basilisk includes a fully integrated Python debugger. Set breakpoints, step through code, inspect variables, and evaluate expressions — all without installing a separate debug extension. The Basilisk LSP server manages the debug session end to end.

## How it works

When you press **F5**, the following happens:

1. VS Code calls the **Basilisk debug adapter factory**
2. The factory asks the **Basilisk LSP** (Rust) to spawn `debugpy` on a free TCP port
3. The LSP spawns `debugpy.adapter --port <port>` and waits for it to be ready
4. The LSP returns the host and port to VS Code
5. VS Code connects its **DAP (Debug Adapter Protocol)** client directly to debugpy
6. You debug normally — breakpoints, stepping, variables, watch expressions

The key insight: **Basilisk brokers the connection** but does not proxy debug traffic. VS Code talks directly to debugpy over TCP, so there is no performance overhead from the LSP layer.

## Prerequisites

You need **debugpy** installed in your Python environment:

```bash
pip install debugpy
```

Basilisk targets **Python 3.12**. Make sure your environment uses Python 3.12 or later.

## Quick start

### 1. Open a Python file

Open any `.py` file in VS Code with the Basilisk extension active.

### 2. Set a breakpoint

Click in the gutter (left of line numbers) to set a red breakpoint dot. You can set breakpoints on any executable line.

### 3. Press F5

VS Code will use the Basilisk debug configuration automatically. If you don't have a `launch.json`, Basilisk provides a default one.

### 4. Debug

The debugger stops at your breakpoint. From here you can:

- **F10** — Step over (execute current line, move to next)
- **F11** — Step into (enter a function call)
- **Shift+F11** — Step out (finish current function, return to caller)
- **F5** — Continue (run until next breakpoint)
- **Shift+F5** — Stop debugging

## launch.json configuration

Basilisk registers the `basilisk-debug` debug type. A typical configuration:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Python: Current File (Basilisk)",
      "type": "basilisk-debug",
      "request": "launch",
      "program": "${file}",
      "console": "internalConsole",
      "redirectOutput": true,
      "justMyCode": true
    }
  ]
}
```

### Configuration options

| Option | Type | Default | Description |
|---|---|---|---|
| `program` | `string` | `${file}` | Path to the Python file to debug |
| `args` | `string[]` | `[]` | Command-line arguments passed to the program |
| `cwd` | `string` | `${workspaceFolder}` | Working directory |
| `console` | `string` | `"internalConsole"` | Where to show output: `internalConsole`, `integratedTerminal`, or `externalTerminal` |
| `redirectOutput` | `boolean` | `true` | Redirect stdout/stderr to the Debug Console |
| `justMyCode` | `boolean` | `true` | Only debug your code, skip library internals |
| `stopOnEntry` | `boolean` | `false` | Break on the first line of the program |
| `python` | `string` | auto-detect | Path to the Python interpreter |

### Console modes

- **`internalConsole`** (recommended) — Program output appears in the **Debug Console** panel. You can also evaluate expressions there.
- **`integratedTerminal`** — Output goes to the VS Code **Terminal** tab. Use this if your program needs interactive stdin input.
- **`externalTerminal`** — Opens a system terminal window.

## Python interpreter resolution

Basilisk resolves the Python interpreter in this order:

1. **`python`** field in `launch.json` — explicit path per debug config
2. **`basilisk.python`** VS Code setting — global setting for the workspace
3. **Auto-detect** — the LSP server looks for a `.venv` in the workspace root, then checks `BASILISK_PYTHON` env var, then falls back to `python3` on PATH

To set the Python path globally for Basilisk:

```json
// .vscode/settings.json
{
  "basilisk.python": ".venv/bin/python"
}
```

## Inspecting variables

When stopped at a breakpoint, the **Variables** pane in the Debug sidebar shows:

- **Locals** — all local variables in the current scope
- **Globals** — module-level variables
- **Return value** — shown after stepping out of a function

Complex objects (dicts, lists, dataclass instances) can be expanded to inspect their contents.

### Watch expressions

Add expressions to the **Watch** panel to monitor them across steps:

- `len(my_list)` — track collection size
- `type(obj)` — check runtime type
- `obj.__dict__` — inspect all attributes
- `[x for x in items if x > 10]` — evaluate comprehensions

### Debug Console (REPL)

While paused, type any Python expression in the **Debug Console** to evaluate it in the current scope:

```
>>> sum(fibonacci_numbers)
88
>>> classified["even"]
[0, 2, 8, 34]
>>> exc.args[0]
'must be >= 0'
```

## Exception handling

Basilisk's debugger handles exceptions naturally. Set a breakpoint inside an `except` block to inspect the exception:

```python
try:
    data = load_config("missing.toml")
except FileNotFoundError as exc:
    print(f"Error: {exc}")  # set breakpoint here
```

When stopped, the **Variables** pane shows:
- `exc` — the exception object
- `exc.args` — the argument tuple
- `exc.__cause__` — chained exception (if `raise ... from ...`)
- Custom attributes on exception subclasses

### Break on raised exceptions

Use VS Code's **Breakpoints** panel (bottom of the Debug sidebar) to configure exception breakpoints:
- Check **Raised Exceptions** to break whenever any exception is raised
- Check **Uncaught Exceptions** to break only on unhandled exceptions

## Attach mode

To debug a running process, start debugpy in your script:

```python
import debugpy
debugpy.listen(5678)
debugpy.wait_for_client()  # pause until VS Code connects
```

Then use an attach configuration:

```json
{
  "name": "Attach (Basilisk)",
  "type": "basilisk-debug",
  "request": "attach",
  "connect": {
    "host": "localhost",
    "port": 5678
  }
}
```

This is useful for debugging web servers (Django, Flask, FastAPI) and long-running processes.

## Verifying the debug chain

To confirm debugging is going through Basilisk (not another extension), open the **Basilisk** output channel (**View > Output > Basilisk**). When you start a debug session, you will see:

```
[Basilisk Debug] createDebugAdapterDescriptor called — type=basilisk-debug, request=launch, program=/path/to/file.py
[Basilisk Debug] Requesting LSP to spawn debugpy (python: auto-detect)...
[Basilisk Debug] LSP spawned debugpy → connecting VS Code DAP client to localhost:57356 (session: dbg-11a1cb40)
```

These messages prove the full chain: VS Code → Basilisk extension → Basilisk LSP (Rust) → debugpy → your code.

## Troubleshooting

### debugpy not installed

```
Basilisk Debug: debugpy is not installed. Run: pip install debugpy
```

Install debugpy in the Python environment Basilisk is using:

```bash
pip install debugpy
```

### No Python interpreter found

```
Basilisk Debug: No Python interpreter found.
```

Set the interpreter explicitly:

```json
// .vscode/settings.json
{
  "basilisk.python": "/path/to/python3.12"
}
```

### Debug Console shows no output

If `print()` output doesn't appear in the Debug Console, check your `launch.json`:

- Set `"console": "internalConsole"` (not `"integratedTerminal"`)
- Set `"redirectOutput": true`

With `integratedTerminal`, output goes to the Terminal tab instead.

### ECONNREFUSED on debug start

If you see `connect ECONNREFUSED 127.0.0.1:<port>`, the debugpy process may have failed to start. Check:

1. Is debugpy installed? `python -m debugpy --version`
2. Is the Python path correct? Check `basilisk.python` setting
3. Check the Basilisk output channel for error details

## Debugging in Zed

Basilisk's debugger works in Zed via the same DAP (Debug Adapter Protocol) mechanism. The Zed extension registers the `basilisk-debug` debug adapter, which connects to debugpy over TCP.

### Prerequisites

Install debugpy in your Python environment:

```bash
pip install debugpy
```

### Starting a debug session

1. Open a Python file in Zed
2. Set breakpoints by clicking in the gutter
3. Start debugging from the command palette or debug panel

### Console output

By default, Basilisk uses **`integratedTerminal`** mode in Zed. This means program output (from `print()`, etc.) appears in the **Terminal** tab, not the Console tab.

If you want output in the debug console instead, override the `console` setting in your debug configuration:

```json
{
  "program": "${file}",
  "console": "internalConsole"
}
```

### Console modes in Zed

| Mode | Where output appears | When to use |
|---|---|---|
| `integratedTerminal` (default) | **Terminal** tab | Programs that need stdin input, or when you want output separate from debug controls |
| `internalConsole` | **Console** tab | When you want output alongside debug expression evaluation |

### Debug configuration

The Zed extension supports these options in the debug launch configuration:

| Option | Type | Default | Description |
|---|---|---|---|
| `program` | `string` | — | Path to the Python file to debug (required) |
| `args` | `string[]` | `[]` | Command-line arguments passed to the program |
| `cwd` | `string` | workspace root | Working directory |
| `python` | `string` | auto-detect | Path to the Python interpreter |
| `justMyCode` | `boolean` | `true` | Only debug your code, skip library internals |
| `stopOnEntry` | `boolean` | `false` | Break on the first line of the program |
| `console` | `string` | `integratedTerminal` | Where to show output: `integratedTerminal` or `internalConsole` |

### Verifying the debug chain in Zed

Check the Zed log (`Cmd+Shift+P` → **zed: open log**) for messages confirming the debug adapter connection:

```
[basilisk-debug] Spawning debugpy on port 57356
[basilisk-debug] DAP client connected
```

This confirms: Zed → Basilisk extension → Basilisk LSP (Rust) → debugpy → your code.

## Architecture

```
┌──────────────┐    LSP (JSON-RPC)    ┌──────────────┐    spawns    ┌──────────┐
│  VS Code /   │◄────────────────────►│ Basilisk LSP │────────────►│  debugpy │
│     Zed      │                      │   (Rust)     │             │ adapter  │
└──────┬───────┘                      └──────────────┘             └────┬─────┘
       │                                                                │
       │              DAP (TCP, direct connection)                      │
       └────────────────────────────────────────────────────────────────┘
```

The LSP server's only role is spawning debugpy and returning the TCP port. All DAP traffic flows directly between the editor and debugpy with zero overhead from the Rust process.
