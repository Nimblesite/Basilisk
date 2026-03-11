---
layout: layouts/docs.njk
title: Quick Start
description: Get started with Basilisk in 5 minutes. Install the VS Code extension and get full Python language support — type checking, completions, and more.
keywords: basilisk, quick start, python language server, type checking, tutorial, vs code
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
error[BSK-E0001]: Missing parameter type annotation
  --> bad.py:1:12
   |
 1 | def process(data):
   |             ^^^^ parameter `data` has no type annotation
   |
   = help: add type annotation: `data: str`
   = note: all parameters must be explicitly typed
   = see: https://www.basilisk-python.dev/docs/rules/#BSK-E0001

error[BSK-E0002]: Missing return type annotation
  --> bad.py:1:1
   |
 1 | def process(data):
   |     ^^^^^^^ function has no return type annotation
   |
   = help: add return type: `def process(data: str) -> str:`

Found 5 errors in 1 file.
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
error[BSK-E0001]: Missing parameter type annotation
^^^^^            ^                                  ← severity + message
  --> bad.py:1:12                                   ← file:line:column
   |
 1 | def process(data):                             ← source context
   |             ^^^^  parameter `data` ...         ← caret pointing at the issue
   |
   = help: add type annotation: `data: str`         ← actionable fix
   = note: all parameters must be explicitly typed  ← explanation
   = see: https://www.basilisk-python.dev/docs/rules/#BSK-E0001  ← documentation link
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

## Next steps

- [Configuration reference](/docs/configuration/) — full `pyproject.toml` schema
- [All rules](/docs/rules/) — every BSK-E and BSK-W code explained
- [Migration guide](/docs/migration/) — migrating from Pyright or mypy
