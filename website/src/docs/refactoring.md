---
layout: layouts/docs.njk
title: "Python Refactoring — Code Actions in Basilisk"
description: "A full suite of Python refactoring code actions in Basilisk — extract variable or function, inline, move symbol, rename, convert, and change signature safely."
keywords: basilisk, refactoring, extract variable, extract function, inline, move symbol, rename, code actions
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Refactoring
  parent: Introduction
  order: 7
---

# Refactoring

Basilisk provides **a full suite of refactoring code actions** via the LSP protocol. They appear in the lightbulb menu in VS Code, Zed, and Neovim automatically (Cursor/Windsurf via Open VSX coming very soon) — no additional extensions or configuration required.

Every refactoring produces a `WorkspaceEdit` that the editor applies atomically. Multi-file refactorings (move symbol, module rename) use `DocumentChanges` with `CreateFile` operations.

## Extract

### Extract variable

Select an expression. Basilisk inserts `extracted_value = <expression>` before the current line and replaces the selection with `extracted_value`.

If the expression appears more than once in the file, a second action **"Extract variable -- replace all"** is offered that replaces every identical occurrence.

```python
# Before
result = some_func(42) + other_func(7)

# After "Extract variable" on `some_func(42)`
extracted_value = some_func(42)
result = extracted_value + other_func(7)
```

### Extract constant

Same as extract variable, but places the assignment at module level with a `SCREAMING_SNAKE_CASE` name — after the last import line.

```python
# Before
def f():
    return 42

# After "Extract constant" on `42`
EXTRACTED_VALUE = 42

def f():
    return EXTRACTED_VALUE
```

### Extract function

Select one or more complete lines. Basilisk extracts them into a new function with:

- **Data flow analysis** — variables read inside but defined outside become parameters; variables written inside and read after become return values
- **Async detection** — if the enclosing function is `async def`, the extracted function is also `async` and called with `await`
- **Method detection** — if inside a method, the extracted function gets `self`/`cls` as its first parameter and is called via `self.extracted_function()`
- **Safety guards** — selections containing `yield`, `break`, or `continue` are rejected (these cannot be extracted without changing semantics)
- **PEP 8 formatting** — top-level extractions get two blank lines before/after; nested extractions get one

```python
# Before
def process(data: list[int]) -> int:
    total = sum(data)
    average = total / len(data)
    return average

# After "Extract function" on lines 2-3
def extracted_function(data: list[int]) -> int:
    total = sum(data)
    average = total / len(data)
    return average

def process(data: list[int]) -> int:
    average = extracted_function(data)
    return average
```

## Inline

### Inline variable

Place the cursor on a simple `name = expr` assignment. Basilisk replaces every subsequent use of `name` with `expr` and deletes the assignment.

Only offered when the assignment is a simple `name = expr` (no tuple unpacking, no augmented assignment) and the name appears at least once after the assignment within the same indentation scope.

```python
# Before
temp = calculate()
result = temp + 1

# After "Inline variable"
result = calculate() + 1
```

### Inline function

Place the cursor on a function call. If the called function has a single `return expr` body, Basilisk replaces the call with the expression, substituting parameter names with the provided arguments.

```python
# Before
def double(x: int) -> int:
    return x * 2

result = double(5)

# After "Inline function" on `double(5)`
result = 5 * 2
```

## Move

### Move to new file

Place the cursor on a top-level `def` or `class` line. Basilisk:

1. Creates a new `.py` file named after the symbol (`CamelCase` becomes `snake_case.py`)
2. Moves the full definition plus its imports into the new file
3. Replaces the definition in the original file with `from .snake_case import SymbolName`

### Move to existing file

Same as above, but moves the symbol to a file you choose. This uses the `basilisk.moveSymbol` command, which editor extensions can wire to a file picker.

## Rename

Basilisk's rename is **scope-aware** — renaming `x` inside one function does not touch `x` in another function or at module level.

Additional rename features:

- **Keyword arguments** — renaming a parameter also renames `param=value` at call sites
- **`self.attr` / `cls.attr`** — renaming a class attribute updates all `self.attr` references across all methods
- **`__all__` entries** — if the symbol appears in `__all__`, its entry is updated
- **Docstring references** — `:param name:` (Sphinx), `name (type):` (Google), and `name :` (NumPy) patterns are updated
- **Validation** — rejects renames to Python builtins, keywords, or names that would shadow existing variables

## Change signature

### Remove parameter

Place the cursor on a parameter name in a `def` line. Basilisk removes the parameter and the corresponding argument from every call site in the file. `self` and `cls` cannot be removed.

### Add parameter

Place the cursor on a `def` line. Basilisk appends `new_param=None` to the parameter list.

### Sort parameters

Place the cursor on a `def` line with 3+ non-self parameters. Basilisk sorts them alphabetically, keeping `self`/`cls` first.

## Implement abstract methods

Place the cursor on a class that inherits from an abstract base class. Basilisk generates method stubs for all unimplemented abstract methods, with correct `self` parameter and `pass` body.

## Construct conversions

These are bidirectional conversions offered when the cursor is on the relevant syntax:

| Conversion | Example |
|-----------|---------|
| `Union[X, Y]` ↔ `X \| Y` | `Union[int, str]` becomes `int \| str` |
| `Optional[X]` ↔ `X \| None` | `Optional[int]` becomes `int \| None` |
| f-string ↔ `.format()` | `f"hello {name}"` becomes `"hello {}".format(name)` |
| `dict()` ↔ `{}` | `dict(a=1, b=2)` becomes `{"a": 1, "b": 2}` |
| `list()` ↔ `[]` | `list()` becomes `[]` |
| Ternary ↔ if/else | `x = a if cond else b` becomes a 4-line if/else block |
| NamedTuple class ↔ functional | Class syntax becomes `namedtuple()` call and vice versa |

## Module rename (workspace/willRenameFiles)

When a `.py` file is renamed in the editor, Basilisk rewrites all `import` and `from ... import` statements across the workspace that referenced the old module path. This handles:

- `import old.module` → `import new.module`
- `from old.module import name` → `from new.module import name`
- `__init__.py` → package name mapping
- Nested submodule paths

> **Note:** This feature requires `workspace/willRenameFiles` support in the LSP client. Currently blocked on tower-lsp 0.21+ for capability registration; the handler logic is fully implemented.

## Competitive comparison

| Feature | Basilisk | Pylance | Pyright | Jedi-LSP | pylsp + Rope |
|---------|----------|---------|---------|----------|-------------|
| Rename (scope-aware) | Yes | Yes | Yes | Yes | Yes |
| Rename module/file | Yes | Yes | No | No | Yes |
| Extract function | Yes | Yes | No | Yes | Yes |
| Extract variable | Yes | Yes | No | Yes | Yes |
| Move symbol | Yes | Yes | No | No | Yes |
| Implement abstract methods | Yes | Yes | No | No | No |
| Construct conversions | Yes | Partial | No | No | No |
| Inline variable | Yes | No | No | No | Yes |
| Change signature | Yes | No | No | No | Partial |
| Inline function | Yes | No | No | No | Yes |
