---
layout: layouts/docs.njk
title: "Configuration Reference — pyproject.toml Settings"
description: "Complete reference for all Basilisk configuration options in pyproject.toml. Severity overrides, per-path rules, inline suppressions, and Ruff integration."
keywords: basilisk, configuration, pyproject.toml, settings
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
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

[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["BSK-E0011"]
rules."BSK-E0010" = "warning"
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

## `[tool.basilisk.per-path-overrides."<glob>"]`

Apply different settings to specific paths. The glob is matched against file paths relative to the project root.

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
# Turn rules off entirely for matching files
disabled = ["BSK-E0011"]

[tool.basilisk.per-path-overrides."tests/**"]
# Or soften a rule's severity instead of disabling it
rules."BSK-E0011" = "warning"
```

### `disabled`

**Type:** `string[]`
**Example:** `["BSK-E0011", "BSK-E0001"]`

Rule codes to disable entirely for files matching this glob.

### `rules`

**Type:** table of rule code → severity
**Severities:** `"error"`, `"warning"`, `"info"`, `"disabled"`
**Example:** `rules."BSK-E0011" = "warning"`

Override the severity of specific rules for matching files. Prefer softening or disabling individual rules over relaxing broad swaths of checking.

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

> **Note:** Inline suppressions without a reason comment are themselves flagged as a warning. The reason is not checked for content — it just needs to be present.

---

## Configuration discovery

Basilisk searches for `pyproject.toml` starting from the directory of the file being checked, traversing up to the filesystem root. The first `pyproject.toml` containing a `[tool.basilisk]` section is used.

If no configuration file is found, Basilisk uses defaults: all rules enabled, `python-version = "3.12"`, check the current directory.
