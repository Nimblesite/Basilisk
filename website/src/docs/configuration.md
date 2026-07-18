---
layout: layouts/docs.njk
title: "Configuration Reference — pyproject.toml Settings"
description: "Complete reference for all Basilisk configuration options in pyproject.toml. Severity overrides, per-path rules, inline suppressions, and Ruff integration."
keywords: basilisk, configuration, pyproject.toml, settings
date: 2026-02-28
dateModified: 2026-07-14
author: The Basilisk Project
eleventyNavigation:
  key: Configuration
  order: 4
---

# Configuration Reference

`[tool.basilisk]` in `pyproject.toml` is the only configuration source. For
each file it checks, Basilisk walks up from the file's directory and reads
every ancestor `pyproject.toml` that carries a `[tool.basilisk]` table. The
tables merge cumulatively, with the nearest file winning wherever the same key
is set — a `pyproject.toml` in a child folder refines the root configuration,
it never replaces it.

> **Migrating from `basilisk.json`?** The legacy root-level `basilisk.json`
> file is no longer read. Translate its keys into `[tool.basilisk]`
> (camelCase → kebab-case, e.g. `typeshedPath` → `typeshed-path`) and delete
> the file. The configuration editor reports a stray `basilisk.json` as an
> ignored shadowed source.

## Minimal configuration

```toml
[tool.basilisk]
```

That's all you need. Basilisk finds Python files from the current directory and
applies the **core PEP conformance rules**. It has no fixed Python-version
default; version-dependent behavior follows the project's evidence and the
pinned typing directives
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)).

## Full configuration example

```toml
[tool.basilisk]
python-platform = "All"
stub-paths = ["stubs/"]
# Unpinned acquisition verifies python/typeshed@main:
# typeshed-commit = "<full commit SHA>"     # optional explicit immutable source
typeshed-cache-path = ".cache/typeshed"     # optional automatic storage path
# typeshed-path = "typeshed-micropython"    # optional canonical custom tree
include = ["src/", "tests/"]
exclude = ["**/migrations/**", "**/generated/**"]

[tool.basilisk.rules]
"BSK-0001" = "warning"             # selects this opt-in rule at warning
"imports_unresolved" = "info"
"dataclasses_order" = "disabled"

[tool.basilisk.per-path-overrides."legacy/**"]
disabled = ["returns_compatibility"]
rules."imports_unresolved" = "warning"
```

---

## `[tool.basilisk]`

### `python-version`

**Type:** `string`
**Default:** inferred from project/interpreter evidence; unset when none exists

An optional explicit target for type checking. Basilisk applies it to typeshed's
`stdlib/VERSIONS` and the simple `sys.version_info` / `sys.platform` checks the
pinned typing specification expects checkers to understand
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)).
It selects within one typeshed snapshot; it never selects a commit.

### `python-platform`

**Type:** `"Linux" | "macOS" | "Windows" | "All"`
**Default:** `"All"`

Target platform. Affects platform-specific type stubs and conditional imports.

### `stub-paths`

**Type:** `string[]`
**Default:** `[]`
**Example:** `["stubs/", "typings/"]`

Additional directories to search for `.pyi` stub files. These sit at the **head** of the import search path — step 1 of the [typing spec's import-resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering) — so they can patch or shadow any later module, standard-library or third-party. Useful for custom stubs for internal libraries.

### Standard-library typeshed

**Spec:** [`STUBRES-TYPESHED`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)

Pinned typing step 3 calls for "Typeshed stubs for the standard library" and
makes a supplied custom tree canonical
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Without a custom path, Basilisk uses an explicit `typeshed-commit` or verifies
`python/typeshed@main` before the first CLI/LSP analysis. The selected SHA
supplies `.pyi` bodies, `stdlib/VERSIONS`, and the distribution map as one
generation.

If acquisition is unavailable, a bundled baseline supplies names and the
distribution map only—never `.pyi` bodies—and Basilisk reports
`typeshed download unavailable; using bundled names only`. An unpinned failure
never reuses an earlier checkout. Downloaded or custom data wholly bypasses
bundled and compiled lookups.

#### `typeshed-commit`

**Type:** `string`
**Default:** _(unset — acquisition verifies `python/typeshed@main`)_
**Example:** a full 40-character commit SHA

Select an exact immutable upstream commit. This is deliberate user-controlled
determinism, not an automatic Python-version mapping or a stale fallback.

#### `typeshed-cache-path`

**Type:** `string`
**Default:** _(the OS cache directory)_
**Example:** `".cache/typeshed"`

Relocate **where the automatic clone is stored**. It only moves the auto-clone — it does not turn cloning off (that is `typeshed-path`). The visual configuration editor exposes this as a **folder picker**.

### `typeshed-path`

**Type:** `string`
**Default:** _(unset — use the explicit-pin/main/baseline selection)_
**Example:** `"typeshed-micropython"`
**Spec:** [`STUBRES-CUSTOM-TYPESHED`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)

Path to a custom or modified typeshed tree. Pinned step 3 says checkers should
use it as the "canonical source for standard-library types in this step"
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
It is the sole step-3 source and disables automatic, bundled, and compiled
lookups. A missing module proceeds to step 4 rather than another typeshed.

The tree must contain `stdlib/<module>.pyi`; relative paths resolve from the
project root. Use it for a fork or an alternative standard library such as
MicroPython. Hover reports `(custom typeshed)` so the winning source is visible.

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

## How to use a custom typeshed

This walkthrough applies pinned step 3's **“canonical source for standard-library
types in this step”** clause
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

### 1. Lay the directory out like typeshed

Point `typeshed-path` at a root containing `stdlib/`; modules resolve as
`<typeshed-path>/stdlib/<module>.pyi`.

### 2a. Point at a forked or hand-patched typeshed

Clone the typeshed repo (or your fork of it), then point `typeshed-path` at the clone and patch the `.pyi` files you need:

```sh
git clone https://github.com/python/typeshed vendor/typeshed
```

```toml
[tool.basilisk]
typeshed-path = "vendor/typeshed"
```

Basilisk now uses `vendor/typeshed/stdlib/` as the sole step-3 source; this
checkout is yours to maintain. Leave `typeshed-path` unset to verify upstream
`main`, or select an exact upstream SHA with `typeshed-commit`.

### 2b. Point at MicroPython's standard library

MicroPython's stdlib diverges from CPython — `os`, `time`, and `machine` carry different signatures. Install [`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs) (a typeshed-layout copy of the stdlib with MicroPython-specific edits) and point at it:

```toml
[tool.basilisk]
typeshed-path = "vendor/micropython_stdlib_stubs"
```

Because `micropython-stdlib-stubs` is a **partial** stdlib, a module it does not ship (e.g. `tkinter`, which does not exist on a board) is **not** rescued by the auto-cloned CPython typeshed — setting `typeshed-path` disables the auto-clone, and your custom typeshed is the canonical source for step 3, so the import is reported as unresolved. That is the honest answer for an embedded target.

### `typeshed-path` vs `stub-paths`

They solve different problems and can be combined:

| | `stub-paths` (step 1) | `typeshed-path` (step 3) |
| --- | --- | --- |
| Role | *Prepends* extra `.pyi` directories at the head of the search path | *Replaces* the auto-cloned standard-library typeshed wholesale, and disables cloning |
| Scope | Can shadow any single module, stdlib or third-party | Canonical source for the entire standard library |
| Typical use | Patch one broken stub; stubs for an internal library | Target an alternative or forked stdlib (MicroPython, a patched fork), or reuse a typeshed tree already on disk |
| Precedence | Wins — a `stub-paths` module still shadows the custom typeshed | Sits below `stub-paths`, above installed packages |

---

## Rule selection and global severity

The unconfigured default enables the complete core PEP rule set. Basilisk-
specific house rules are tagged `basilisk` and stay off until a project opts in.
There is no ambient basic/standard/strict mode. The editor's **Strict
preset** is a one-shot recipe that writes every live rule's native severity
explicitly into the active config file; after applying it, each rule remains
independently configurable.

Every opt-in rule is selected by assigning that rule a non-disabled severity:

```toml
[tool.basilisk.rules]
"BSK-0001" = "error"   # required parameter annotations
"BSK-0025" = "error"   # required @override
"BSK-0011" = "warning" # undeclared dependency imports
"BSK-0152" = "error"   # missing type stubs
```

There are no family switches in the project or editor settings. Use the
generated [rule reference](/docs/rules/) or configuration editor to browse the
canonical tags; tag actions expand to explicit rule entries in this file.

### `[tool.basilisk.rules]`

Set a rule's global severity. For an opt-in rule, any non-disabled value also
selects it:

```toml
[tool.basilisk.rules]
"imports_unresolved" = "warning"
"BSK-0050" = "error"
"dataclasses_order" = "disabled"
```

Accepted values are `"error"`, `"warning"`, `"info"`, and `"disabled"`.
For an opt-in rule, `"error"`, `"warning"`, or `"info"` selects that individual
rule; `"disabled"` keeps it off. No second switch is required.

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
**Example:** `["returns_compatibility", "BSK-0001"]`

Rule codes to disable entirely for files matching this glob.

### `rules`

**Type:** table of rule code → severity
**Severities:** `"error"`, `"warning"`, `"info"`, `"disabled"`
**Example:** `rules."returns_compatibility" = "warning"`

Override the severity of specific rules for matching files. Prefer softening or disabling individual rules over relaxing broad swaths of checking.

---

## Inline suppressions

Use the standard `# type: ignore` spelling. A Basilisk rule code makes the
suppression specific:

```python
result: Any = get_legacy_value()  # type: ignore[returns_compatibility]
```

Bare or foreign-checker ignore codes follow PEP 484 compatibility behavior and
suppress all diagnostics on the line:

```python
data = unsafe_cast(value)  # type: ignore
```

The same syntax can change severity instead of hiding a diagnostic:

```python
value = legacy_call()  # type: warning[returns_compatibility]
value = legacy_call()  # type: info[returns_compatibility]
value = legacy_call()  # type: disabled[returns_compatibility]
```

File-level directives are standalone comments:

```python
# basilisk: relaxed
# basilisk: file-warning[returns_compatibility]
# basilisk: file-disabled[imports_unresolved]
```

Suppression auditing is an **opt-in** tagged rule family. It emits nothing by
default. Configure `BSK-0060` (active specific), `BSK-0061` (active blanket),
`BSK-0062` (unused), and `BSK-0063` (malformed) independently at error,
warning, info, or disabled. See the
[configuration-editor specification](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-SUPPRESSIONS).

---

## Configuration discovery

Basilisk discovers configuration per checked file by walking **up** from the
file's directory. Every ancestor `pyproject.toml` that carries a
`[tool.basilisk]` table contributes, and the tables merge cumulatively: where
the same key is set in more than one file, the **nearest** file wins. Keys a
child table does not set continue to come from the ancestors, so a nested
`pyproject.toml` refines the root configuration — it never blows the root
config away.

The legacy root-level `basilisk.json` is **never** read. If one is still
present, the configuration editor reports it as an ignored shadowed source;
translate its keys into `[tool.basilisk]` and delete the file.

If no ancestor `pyproject.toml` carries a `[tool.basilisk]` table, Basilisk
enables the core PEP conformance rules and checks the current directory. It does
not manufacture a Python-version default.

---

## Visual configuration editor

The tag-first VS Code editor reads the live rule catalog from the LSP, previews
all/tag/rule bulk changes, exposes every rule's effective and explicit severity,
and makes opt-in suppression diagnostics searchable across the workspace. Its
LSP-advertised Strict preset turns the complete catalog on at each rule's native
severity and persists the expanded rule entries—not a mode flag. Safe fixes are
a separate root-scoped LSP action, so applying a preset never hides source edits
inside a config transaction.

Generated adoption debt is stored as ordinary exact-file `per-path-overrides`
entries in this same active config file—no `.basilisk/adoptions.toml`, hidden
state, or adoption mode. The VSIX does not parse or write configuration itself.

![Basilisk's tag-first VS Code configuration editor, showing live rule facets and per-rule severity controls](/assets/images/vscode-configuration-editor.png)

Track the authoritative
[specification](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
and [implementation plan](https://github.com/Nimblesite/Basilisk/blob/main/docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md).
