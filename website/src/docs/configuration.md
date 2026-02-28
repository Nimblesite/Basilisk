---
layout: layouts/docs.njk
title: Configuration Reference
description: Complete reference for Basilisk's pyproject.toml configuration options.
keywords: basilisk, configuration, pyproject.toml, settings
eleventyNavigation:
  key: Configuration
  order: 4
---

# Configuration Reference

Basilisk is configured through `pyproject.toml`. All settings live under `[tool.basilisk]`.

## Minimal configuration

```toml
[tool.basilisk]
python-version = "3.12"
```

That's all you need. Basilisk finds Python files from the current directory and applies all rules.

## Full configuration example

```toml
[tool.basilisk]
python-version = "3.12"
python-platform = "All"
stub-paths = ["stubs/"]
include = ["src/", "tests/"]
exclude = ["**/migrations/**", "**/generated/**"]

[tool.basilisk.mojo-safety]
ownership = true
immutability = true
no-implicit-coercion = true

[tool.basilisk.migration]
enabled = true
started = "2025-06-01"
enforce_after = "2025-12-01"

[tool.basilisk.per-path-overrides."legacy/**"]
strict = false
deadline = "2026-12-31"
```

---

## `[tool.basilisk]`

### `python-version`

**Type:** `string`
**Default:** auto-detected from the interpreter on PATH, or `"3.12"` if not found
**Example:** `"3.12"`

The Python version to target for type checking. Affects which PEPs and typing features are available. Supports versions `"3.9"` through `"3.14"`.

### `python-platform`

**Type:** `"Linux" | "macOS" | "Windows" | "All"`
**Default:** `"All"`

Target platform. Affects platform-specific type stubs and conditional imports.

### `stub-paths`

**Type:** `string[]`
**Default:** `[]`
**Example:** `["stubs/", "typings/"]`

Additional directories to search for `.pyi` stub files. Searched in order before the bundled typeshed stubs. Useful for custom stubs for internal libraries.

### `include`

**Type:** `string[]`
**Default:** `["."]` (current directory)
**Example:** `["src/", "tests/"]`

Directories or files to analyze. Accepts paths and glob patterns. Only `.py` files are processed.

### `exclude`

**Type:** `string[]`
**Default:** `["**/node_modules/**", "**/__pycache__/**"]`
**Example:** `["**/migrations/**", "**/generated/**"]`

Glob patterns to exclude from analysis. Applied after `include`. Use `**` for recursive matching.

---

## `[tool.basilisk.mojo-safety]`

Controls Mojo-inspired safety analysis. See [Mojo-Style Safety](/docs/mojo-safety/) for full documentation.

### `ownership`

**Type:** `boolean`
**Default:** `true`

Enable ownership analysis: `Borrowed`, `InOut`, `Owned` annotation checking.
Flags mutation of `Borrowed` parameters (BSK-E0030) and use-after-move (BSK-E0031).

### `immutability`

**Type:** `boolean`
**Default:** `true`

Enforce immutability of parameters not annotated with `InOut`.
Flags mutation of unannotated parameters (BSK-E0040).

### `no-implicit-coercion`

**Type:** `boolean`
**Default:** `true`

Flag implicit type coercions: `int` → `float`, `bool` → `int`, `bytes` → `str`.
Requires explicit conversion functions (BSK-E0060 through BSK-E0063).

---

## `[tool.basilisk.migration]`

Migration mode softens selected errors to warnings for a defined period, making it easier to adopt Basilisk in an existing codebase.

### `enabled`

**Type:** `boolean`
**Default:** `false`

Enable migration mode. When `true`, errors are reported as warnings until `enforce_after`.

### `started`

**Type:** `string` (ISO date)
**Example:** `"2025-06-01"`

Informational: when migration was started. Used in progress reports.

### `enforce_after`

**Type:** `string` (ISO date)
**Example:** `"2025-12-01"`

After this date, all warnings in migration mode become errors again. Basilisk will warn you as the deadline approaches.

---

## `[tool.basilisk.per-path-overrides."<glob>"]`

Apply different settings to specific paths. The glob is matched against file paths relative to the project root.

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
strict = false
deadline = "2026-12-31"

[tool.basilisk.per-path-overrides."tests/**"]
# Tests can use Any more freely
rules.ignore = ["BSK-E0011"]
```

### `strict`

**Type:** `boolean`
**Default:** `true`

Set to `false` to disable strict mode for matching files. All errors become warnings.

### `deadline`

**Type:** `string` (ISO date)

A date after which `strict = false` is no longer honored and errors are enforced. Basilisk prints a reminder as the deadline approaches.

### `rules.ignore`

**Type:** `string[]`
**Example:** `["BSK-E0011", "BSK-W0080"]`

Specific rules to ignore in matching files. Prefer narrow ignores over `strict = false` when possible.

---

## Inline suppressions

To suppress a diagnostic on a specific line, add a comment with the rule code and a mandatory reason:

```python
result: Any = get_legacy_value()  # basilisk: ignore[BSK-E0011] -- no stub available, tracked in #123
```

To suppress all diagnostics on a line:

```python
data = unsafe_cast(value)  # basilisk: ignore -- third-party code, cannot type
```

To suppress all diagnostics in a file, add at the top:

```python
# basilisk: relaxed
```

> **Note:** Inline suppressions without a reason comment are themselves flagged as BSK-W0095. The reason is not checked for content — it just needs to be present.

---

## Configuration discovery

Basilisk searches for `pyproject.toml` starting from the directory of the file being checked, traversing up to the filesystem root. The first `pyproject.toml` containing a `[tool.basilisk]` section is used.

If no configuration file is found, Basilisk uses defaults: all rules enabled, `python-version = "3.12"`, check the current directory.
