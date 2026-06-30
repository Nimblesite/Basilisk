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

That's all you need. Basilisk finds Python files from the current directory and applies its default rule set — the **core PEP conformance rules**. Extra Basilisk rules that go beyond the spec are opt-in; enable them when you want stricter-than-spec checking.

## Full configuration example

```toml
[tool.basilisk]
python-version = "3.12"
python-platform = "All"
stub-paths = ["stubs/"]
include = ["src/", "tests/"]
exclude = ["**/migrations/**", "**/generated/**"]

[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["returns_compatibility"]
rules."imports_unresolved" = "warning"
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

Directories or files to analyze. Plain paths relative to the project root — unlike `exclude`, `include` does **not** accept glob patterns. Only `.py` files are processed.

### `exclude`

**Type:** `string[]`
**Default:**

```toml
exclude = [
    "__pycache__", "node_modules", "venv", ".venv", "env", ".env",
    ".tox", ".mypy_cache", ".ruff_cache", ".pytest_cache",
    "site-packages", "__pypackages__", "build", "dist", ".eggs",
    "bundled", "_vendored",
]
```

**Example:** `["py-gen", "**/generated/**", "*.pb.py"]`

Gitignore-style glob patterns for paths to skip. Hidden directories (names starting with `.`) are always skipped regardless of this setting.

> **`exclude` _replaces_ the defaults — it does not extend them.** As soon as you set `exclude`, the built-in list above no longer applies. Re-list any defaults you still want alongside your own patterns, or they'll be analyzed again.

Pattern syntax, matched against each path relative to the project root:

| Pattern | Matches |
| --- | --- |
| `build` | a **bare name** — that directory or file segment at **any** depth |
| `**/generated/**` | `**` — zero or more directory segments (a `generated` dir anywhere) |
| `*.pb.py` | `*` — any run of characters within a single segment (a file glob) |
| `gen?.py` | `?` — exactly one character within a segment |
| `src/generated` | an **anchored** pattern (contains `/`) — the path or any ancestor dir, plus its subtree |

The same patterns are honoured everywhere Basilisk discovers files: the LSP workspace scan, the `basilisk check` / `fix` / `adopt` CLI, and the editor's per-file checks when you open or edit a file — so a file excluded on the CLI is also silent in the editor. See `CHKARCH-CONFIG-EXCLUDE` in the architecture spec for the canonical semantics.

---

## `[tool.basilisk.per-path-overrides."<glob>"]`

Apply different settings to specific paths. The glob is matched against file paths relative to the project root.

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
# Turn rules off entirely for matching files
disabled = ["returns_compatibility"]

[tool.basilisk.per-path-overrides."tests/**"]
# Or soften a rule's severity instead of disabling it
rules."returns_compatibility" = "warning"
```

### `disabled`

**Type:** `string[]`
**Example:** `["returns_compatibility", "BSK-E0001"]`

Rule codes to disable entirely for files matching this glob.

### `rules`

**Type:** table of rule code → severity
**Severities:** `"error"`, `"warning"`, `"info"`, `"disabled"`
**Example:** `rules."returns_compatibility" = "warning"`

Override the severity of specific rules for matching files. Prefer softening or disabling individual rules over relaxing broad swaths of checking.

---

## Inline suppressions

To suppress a diagnostic on a specific line, add a comment with the rule code and a mandatory reason:

```python
result: Any = get_legacy_value()  # basilisk: ignore[returns_compatibility] -- no stub available, tracked in #123
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

If no configuration file is found, Basilisk uses defaults: the **core PEP conformance rule set** enabled (extra Basilisk rules stay opt-in), `python-version = "3.12"`, check the current directory.
