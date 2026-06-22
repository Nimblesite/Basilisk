---
layout: layouts/docs.njk
title: "Quick Start — Type-Check Your First File in 5 Minutes"
description: "Get started with Basilisk in 5 minutes. Install the VS Code extension, run your first type check, and see strict-by-default Python diagnostics in action."
keywords: basilisk, quick start, python language server, type checking, tutorial, vs code
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Quick Start
  order: 3
---

# Quick Start

This guide walks through your first type check with Basilisk. Estimated time: 5 minutes.

## Step 1 — Run your first check

Create a Python file, or use the example from the repository:

```python
# bad.py
def process(data):
    return data.upper()

class User:
    def __init__(self, name, age):
        self.name = name
        self.age  = age

    def greet(self):
        return f"Hello, {self.name}"
```

Run Basilisk:

```bash
basilisk check bad.py
```

Output:

```
error[BSK-E0001]: Missing parameter type annotation for `data`
  --> bad.py:1:13
    |
1 | def process(data):
    |             ^^^^
    |
   = help: Add a type annotation: `data: <type>`
   = note: In Basilisk, all function parameters require explicit types
   = see: https://www.basilisk-python.dev/errors/BSK-E0001

error[BSK-E0002]: Missing return type annotation for function `process`
  --> bad.py:1:5
    |
1 | def process(data):
    |     ^^^^^^^^^^^^^
    |
   = help: Add a return type: `def process(...) -> <type>:`
   = note: In Basilisk, all functions require an explicit return type
   = see: https://www.basilisk-python.dev/errors/BSK-E0002

... 4 more errors (untyped `name`, `age`, `__init__`, and `greet`) ...

Found 6 diagnostics (6 errors).
```

## Step 2 — Fix the errors

Add type annotations to every parameter and return type:

```python
# good.py
def process(data: str) -> str:
    return data.upper()

class User:
    name: str
    age: int

    def __init__(self, name: str, age: int) -> None:
        self.name = name
        self.age  = age

    def greet(self) -> str:
        return f"Hello, {self.name}"
```

```bash
basilisk check good.py
```

```
All checked. No issues found.
Checked 1 file — 0 errors, 0 warnings.
```

## Step 3 — Check a directory

Basilisk recursively checks every `.py` file in a directory:

```bash
basilisk check src/
```

To check the current directory:

```bash
basilisk check
```

## Step 4 — Add to pyproject.toml

Create a `[tool.basilisk]` section in your `pyproject.toml`:

```toml
[tool.basilisk]
python-version = "3.12"
include = ["src/", "tests/"]
exclude = ["**/migrations/**"]
```

With a config file present, running `basilisk check` uses these settings automatically.

## Step 5 — Understand a diagnostic

Basilisk uses the same output format as the Rust compiler (`rustc`). Every diagnostic includes:

```
error[BSK-E0001]: Missing parameter type annotation for `data`
^^^^^            ^                                  ← severity + message
  --> bad.py:1:13                                   ← file:line:column
    |
1 | def process(data):                              ← source context
    |             ^^^^                               ← caret pointing at the issue
    |
   = help: Add a type annotation: `data: <type>`    ← actionable fix
   = note: In Basilisk, all function parameters require explicit types  ← explanation
   = see: https://www.basilisk-python.dev/errors/BSK-E0001  ← documentation link
```

- **`error[BSK-EXXXX]`** — error with its unique code (orange)
- **`-->`** — location in your file (blue)
- **`^^^^`** — exactly which token caused the error (red underline)
- **`= help:`** — the specific change that will fix it (green)
- **`= note:`** — why the rule exists
- **`= see:`** — link to full documentation

## Step 6 — Intentional suppressions

When you genuinely need to use `Any` or suppress a diagnostic, you can — but you must provide a reason:

```python
# This suppression requires a reason comment
result: Any = legacy_sdk_call()  # basilisk: ignore[BSK-E0011] -- tracked in #847
```

Suppressions without reasons are themselves flagged. This is intentional: if you need to suppress a diagnostic, you should be able to explain why.

## Step 7 — Check stats

Get a type coverage report for your project:

```bash
basilisk stats src/
```

Output includes: total functions, typed functions, type coverage percentage, files with no annotations.

## Step 8 — Profile a running script

Basilisk includes an integrated CPU and memory profiler. To try it in VS Code:

1. Run any Python script (the process must be alive)
2. Open the Command Palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) — or use the shortcut `Cmd+Shift+P Cmd+Shift+S` / `Ctrl+Shift+P Ctrl+Shift+S`
3. Run **Basilisk: Start Profiling** and pick the target process
4. Watch inline CPU heat annotations appear on hot lines as samples accumulate
5. Run **Basilisk: Stop Profiling** to open the flamegraph viewer

For memory leak hunting, use **Basilisk: Start Memory Tracking**, take two snapshots with **Basilisk: Take Memory Snapshot**, then **Basilisk: Diff Memory Snapshots** to surface leaks as diagnostics in the Problems panel.

See the [Profiler guide](/docs/profiler/) for the full workflow — flamegraphs, reference graphs, profile diffing, VS Code commands, Zed slash commands, Neovim user commands, and platform requirements.

## Next steps

- [Configuration reference](/docs/configuration/) — full `pyproject.toml` schema
- [Profiler](/docs/profiler/) — CPU heatmaps, flamegraphs, and memory leak detection
- [Debugging](/docs/debugging/) — F5 to debug, breakpoints, stepping, watch expressions
- [All rules](/docs/rules/) — every BSK-E and BSK-W code explained
- [Migration guide](/docs/migration/) — migrating from Pyright or mypy
