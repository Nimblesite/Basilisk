---
layout: layouts/docs.njk
title: "Quick Start — Type-Check Your First File in 5 Minutes"
description: "Get started with Basilisk in 5 minutes. Install the VS Code extension, run your first type check, and see PEP-conformant Python diagnostics in action."
keywords: basilisk, quick start, python language server, type checking, tutorial, vs code
date: 2026-02-28
dateModified: 2026-07-14
author: The Basilisk Project
eleventyNavigation:
  key: Quick Start
  order: 3
---

# Quick Start

This guide walks through your first type check with Basilisk. Estimated time: 5 minutes.

## Step 1 — Run your first check

Create a Python file, or use the example from the repository. Every problem in
it is a genuine violation of the Python typing spec — no configuration needed:

```python
# bad.py
def greet(name: str) -> str:
    return "Hello, " + name


greet(42)


count: int = "zero"


def describe(flag: bool) -> str:
    if flag:
        label = "on"
    return label
```

Run Basilisk:

```bash
basilisk check bad.py
```

Output:

```
error[calls_argument_type]: Argument `name` of `greet` expects `str` but received an `int` literal
  --> bad.py:5:7
  |
5 | greet(42)
  |       ^^
  |
   = help: Pass a value of type `str` for parameter `name`
   = note: Basilisk checks that literal arguments are compatible with declared parameter types
   = see: https://www.basilisk-python.dev/errors/calls_argument_type

error[assignment_compatibility]: Type mismatch: `count` is annotated `int` (int) but assigned str
  --> bad.py:8:1
  |
8 | count: int = "zero"
  | ^^^^^
  |
   = help: Either change the annotation to match the value, or change the value to `int`
   = note: Basilisk requires the inferred type to be assignable to the declared type
   = see: https://www.basilisk-python.dev/errors/assignment_compatibility

error[names_unbound]: Function `describe` returns `label` but `label` may be unbound on some paths
  --> bad.py:14:12
   |
14 |     return label
   |            ^^^^^
   |
   = help: Assign `label` unconditionally before the `return`, or add a default value
   = note: Basilisk detects variables that are assigned only inside conditional branches (if/while/try) and may not be defined on every execution path
   = see: https://www.basilisk-python.dev/errors/names_unbound

Found 3 diagnostics (3 errors).
```

Out of the box, Basilisk enables the complete PEP typing-spec rule set, and
every violation is an **error**. Nothing here is house style — this is the
[Python type system specification](https://typing.python.org/en/latest/spec/index.html),
enforced.

## Step 2 — Fix the errors

```python
# good.py
def greet(name: str) -> str:
    return "Hello, " + name


greet("world")


count: int = 0


def describe(flag: bool) -> str:
    return "on" if flag else "off"
```

```bash
basilisk check good.py
```

```
All checked. No issues found.
```

## Step 3 — Turn strictness up to full

A clean pass means your code obeys the typing spec — but nothing yet *requires*
annotations. Basilisk's own strictness rules (annotations on every parameter,
return type, attribute, and more) are **opt-in and silent by default**. Enable
them as **warnings** while you migrate — the code still type-checks, and the
warnings tell you strictness isn't at full yet:

```toml
# pyproject.toml
[tool.basilisk.rules]
"BSK-0001" = "warning"  # parameters need a type where none is inferable
"BSK-0002" = "warning"  # functions need a return type where none is inferable
```

Add an unannotated function to `good.py`:

```python
def process(data):
    return data.upper()
```

```
warning[BSK-0001]: Missing parameter type annotation for `data`
  --> good.py:15:13
   |
15 | def process(data):
   |             ^^^^
   |
   = help: Add a type annotation: `data: <type>`
   = note: Basilisk requires an explicit parameter type wherever it cannot be inferred; a literal default (e.g. `timeout=30`) infers the type and needs no annotation
   = see: https://www.basilisk-python.dev/errors/BSK-0001

warning[BSK-0002]: Missing return type annotation for function `process`
  --> good.py:15:5
   |
15 | def process(data):
   |     ^^^^^^^^^^^^^
   |
   = help: Add a return type: `def process(...) -> <type>:`
   = note: Basilisk requires an explicit return type wherever it cannot be inferred; literal-only returns (e.g. `return 42`) infer the type and need no annotation
   = see: https://www.basilisk-python.dev/errors/BSK-0002

Found 2 diagnostics (0 errors).
```

Errors are spec violations; warnings are strictness you haven't finished
adopting. When the warnings hit zero, promote the rules to `"error"` and your
project is at full strictness.

In VS Code, skip the hand-editing: run **Basilisk: Open Configuration Editor**
from the Command Palette. It shows every rule with its live severity, previews
changes before applying them, and its **Strict preset** turns the complete
catalog on in one click. See the
[configuration reference](/docs/configuration/) for the full schema.

![Basilisk's tag-first VS Code configuration editor, showing live rule facets and per-rule severity controls](/assets/images/vscode-configuration-editor.png)

## Step 4 — Check a directory

Basilisk recursively checks every `.py` file in a directory:

```bash
basilisk check src/
```

To check the current directory:

```bash
basilisk check
```

## Step 5 — Add to pyproject.toml

Create a `[tool.basilisk]` section in your `pyproject.toml`:

```toml
[tool.basilisk]
include = ["src/", "tests/"]
exclude = ["**/migrations/**"]
```

With a config file present, running `basilisk check` uses these settings
automatically. Basilisk has no fixed Python-version default; set one only when
you intend to override project/interpreter evidence. Version checks follow the
pinned typing directives
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)).

## Step 6 — Understand a diagnostic

Basilisk uses the same output format as the Rust compiler (`rustc`). Every diagnostic includes:

```
error[calls_argument_type]: Argument `name` of `greet` expects `str`…
^^^^^                      ^                     ← severity + message
  --> bad.py:5:7                                 ← file:line:column
  |
5 | greet(42)                                    ← source context
  |       ^^                                     ← caret pointing at the issue
  |
   = help: Pass a value of type `str` for parameter `name`   ← actionable fix
   = note: Basilisk checks that literal arguments are…       ← explanation
   = see: https://www.basilisk-python.dev/errors/calls_argument_type  ← documentation link
```

- **`error[rule_code]`** — severity plus the rule that fired. PEP typing-spec
  rules use descriptive codes (`calls_argument_type`); Basilisk's opt-in
  strictness rules use `BSK-` codes (`BSK-0001`)
- **`-->`** — location in your file
- **`^^^^`** — exactly which token caused the diagnostic
- **`= help:`** — the specific change that will fix it
- **`= note:`** — why the rule exists
- **`= see:`** — link to full documentation

The same information is available in your editor — hover any symbol for its inferred type:

![Basilisk hover in VS Code — hovering a function shows its full inferred signature](/assets/images/vscode-hover.png)

## Step 7 — Intentional suppressions

When you genuinely need to use `Any` or suppress a diagnostic, you can — but you must provide a reason:

```python
# This suppression requires a reason comment
result: Any = legacy_sdk_call()  # type: ignore[returns_compatibility]
```

Suppressions without reasons are themselves flagged. This is intentional: if you need to suppress a diagnostic, you should be able to explain why.

## Step 8 — Check stats

Get a type coverage report for your project:

```bash
basilisk stats src/
```

Output includes: total functions, typed functions, type coverage percentage, files with no annotations.

## Step 9 — Profile a running script

Basilisk includes an integrated CPU and memory profiler. To try it in VS Code:

1. Open a Python file and click **Run & Profile CPU (Current File)** in the **Python Processes** panel (Basilisk icon in the activity bar) — it launches the script and profiles it from the first line. To profile an already-running process instead, click **Profile CPU** on its row in the same panel.
2. Watch inline CPU heat annotations appear on hot lines as samples accumulate
3. Run **Basilisk: Stop Profiling** (`Cmd+Shift+P Cmd+Shift+X` / `Ctrl+Shift+P Ctrl+Shift+X`, or click the status-bar counter) — the Basilisk Profiler panel opens with a flame graph and click-to-source hot-function tables

For memory leak hunting, click **Run & Track Memory (Current File)** in the same panel (memory tracking rides the debugger, so this launches a debug session for you). Snapshots auto-capture at every pause — or take them manually with **Basilisk: Take Memory Snapshot** — and **Basilisk: Compare Memory Snapshots** surfaces leaks as diagnostics in the Problems panel.

See the [Profiler guide](/docs/profiler/) for the full workflow — flame graphs, reference graphs, memory snapshot comparison, VS Code commands, Zed slash commands, Neovim user commands, and platform requirements.

## Next steps

- [Configuration reference](/docs/configuration/) — full `pyproject.toml` schema
- [Profiler](/docs/profiler/) — CPU heatmaps, flamegraphs, and memory leak detection
- [Debugging](/docs/debugging/) — F5 to debug, breakpoints, stepping, watch expressions
- [All rules](/docs/rules/) — every rule explained, PEP and opt-in alike
- [Migration guide](/docs/migration/) — migrating from Pyright or mypy
