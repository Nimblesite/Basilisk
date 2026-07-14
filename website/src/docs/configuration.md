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
python-version = "3.12"
```

That's all you need. Basilisk finds Python files from the current directory and applies its default rule set — the **core PEP conformance rules**. Extra Basilisk rules that go beyond the spec are opt-in; enable them when you want stricter-than-spec checking.

## Full configuration example

```toml
[tool.basilisk]
python-version = "3.12"
python-platform = "All"
stub-paths = ["stubs/"]
typeshed-path = "typeshed-micropython"   # optional: replace the bundled stdlib typeshed
include = ["src/", "tests/"]
exclude = ["**/migrations/**", "**/generated/**"]

[tool.basilisk.rules]
"BSK-E0001" = "warning"             # selects this opt-in rule at warning
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

Additional directories to search for `.pyi` stub files. These sit at the **head** of the import search path — step 1 of the [typing spec's import-resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering) — so they can patch or shadow any later module, standard-library or third-party. Useful for custom stubs for internal libraries.

### `typeshed-path`

**Type:** `string`
**Default:** _(unset — the bundled typeshed is used)_
**Example:** `"typeshed-micropython"`
**Spec:** [`STUBRES-CUSTOM-TYPESHED`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)

Path to a directory containing a custom or modified version of typeshed's standard-library stubs. When set, this directory becomes the **canonical source for standard-library types** — step 3 of the [typing spec's import-resolution ordering](https://typing.python.org/en/latest/spec/distributing.html#import-resolution-ordering), which states that type checkers "SHOULD use this as the canonical source for standard-library types in this step." Basilisk resolves stdlib modules against it in preference to the bundled typeshed; a stdlib module absent from the directory falls through to the remaining resolution steps.

The directory must follow typeshed's layout — standard-library stubs live under a top-level `stdlib/` subdirectory, so Basilisk resolves each module as `<typeshed-path>/stdlib/<module>.pyi`. A clone of the [python/typeshed](https://github.com/python/typeshed) repository, or any directory you already use as Pyright's [`typeshedPath`](https://microsoft.github.io/pyright/#/configuration) or mypy's [`custom_typeshed_dir`](https://mypy.readthedocs.io/en/stable/config_file.html), works unchanged. Relative paths resolve against the project root.

Use this to type-check against an alternative standard library — for example MicroPython's [`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs), whose `os`, `time`, and `machine` signatures differ from CPython. Symbols resolved from the custom typeshed hover with a `(custom typeshed)` tag — distinct from the bundled typeshed's `(typeshed)` — so you can confirm the override is active and know a MicroPython signature is never misreported as CPython's.

`stub-paths` *prepends* extra stub directories; `typeshed-path` *replaces* the vendored standard library wholesale. They are independent and can be combined. See [How to use a custom typeshed](#how-to-use-a-custom-typeshed) below for a step-by-step walkthrough.

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

`typeshed-path` swaps the standard-library stubs Basilisk bundles for your own copy. Reach for it when you target an alternative Python whose standard library differs from CPython (MicroPython, a patched CPython, a vendor SDK), or when you need a newer or forked typeshed than the one baked into your Basilisk release.

### 1. Lay the directory out like typeshed

Point `typeshed-path` at the **root** of a typeshed-layout directory. Standard-library stubs must sit under a top-level `stdlib/` subdirectory, exactly as in the [python/typeshed](https://github.com/python/typeshed) repository — Basilisk resolves each module as `<typeshed-path>/stdlib/<module>.pyi`:

```
vendor/typeshed/
└── stdlib/
    ├── os.pyi
    ├── time.pyi
    └── ...
```

Any directory you already use as Pyright's [`typeshedPath`](https://microsoft.github.io/pyright/#/configuration) or mypy's [`custom_typeshed_dir`](https://mypy.readthedocs.io/en/stable/config_file.html) consumes this same layout, so it works with Basilisk unchanged.

### 2a. Point at a forked or newer typeshed

Clone the typeshed repo (or your fork of it), then point `typeshed-path` at the clone and patch the `.pyi` files you need:

```sh
git clone https://github.com/python/typeshed vendor/typeshed
```

```toml
[tool.basilisk]
typeshed-path = "vendor/typeshed"
```

Basilisk now type-checks the standard library against `vendor/typeshed/stdlib/` instead of its bundled copy.

### 2b. Point at MicroPython's standard library

MicroPython's stdlib diverges from CPython — `os`, `time`, and `machine` carry different signatures. Install [`micropython-stdlib-stubs`](https://github.com/Josverl/micropython-stubs) (a typeshed-layout copy of the stdlib with MicroPython-specific edits) and point at it:

```toml
[tool.basilisk]
python-version = "3.12"
typeshed-path = ".venv/lib/python3.12/site-packages/micropython_stdlib_stubs"
```

Because `micropython-stdlib-stubs` is a **partial** stdlib, a module it does not ship (e.g. `tkinter`, which does not exist on a board) is **not** rescued by the bundled CPython stub — the custom typeshed is the canonical source for step 3, so the import is reported as unresolved. That is the honest answer for an embedded target.

### 3. Configure it in the active project file

`typeshed-path` lives in `[tool.basilisk]` like every other setting — there is
no second spelling and no second file. Set it in the `pyproject.toml` that
governs the files you are checking (the nearest ancestor with a
`[tool.basilisk]` table). Editors do not carry a second copy; that project
config file is authoritative. If you are migrating a legacy `basilisk.json`,
its camelCase `typeshedPath` key becomes `typeshed-path` here.

### 4. Confirm it took effect — hover provenance

Symbols resolved from a custom typeshed hover with a `(custom typeshed)` tag, distinct from the bundled typeshed's `(typeshed)` tag. Hover over an imported stdlib symbol: seeing `(custom typeshed)` confirms the override is active and that the signature came from your directory — a MicroPython `os.uname` is never misreported as CPython's.

### `typeshed-path` vs `stub-paths`

They solve different problems and can be combined:

| | `stub-paths` (step 1) | `typeshed-path` (step 3) |
| --- | --- | --- |
| Role | *Prepends* extra `.pyi` directories at the head of the search path | *Replaces* the bundled standard-library typeshed wholesale |
| Scope | Can shadow any single module, stdlib or third-party | Canonical source for the entire standard library |
| Typical use | Patch one broken stub; stubs for an internal library | Target an alternative or forked stdlib (MicroPython, a newer typeshed) |
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
"BSK-E0001" = "error"   # required parameter annotations
"BSK-E0025" = "error"   # required @override
"BSK-W0011" = "warning" # undeclared dependency imports
"BSK-E0152" = "error"   # missing type stubs
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
"BSK-W0050" = "error"
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
**Example:** `["returns_compatibility", "BSK-E0001"]`

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
default. Configure `BSK-I0060` (active specific), `BSK-W0061` (active blanket),
`BSK-W0062` (unused), and `BSK-E0063` (malformed) independently at error,
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

If no ancestor `pyproject.toml` carries a `[tool.basilisk]` table, Basilisk uses defaults: the **core PEP conformance rule set** enabled (extra Basilisk rules stay opt-in), `python-version = "3.12"`, check the current directory.

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
