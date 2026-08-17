---
layout: layouts/docs.njk
title: "Migrate to Basilisk from Pyright or mypy"
description: "Current, honest steps for moving a Python project to Basilisk with per-rule severity and gradual path-based adoption."
keywords: migrate to basilisk, from pyright, from mypy, python type checker migration
date: 2026-02-28
dateModified: 2026-07-19
author: The Basilisk Project
eleventyNavigation:
  key: Migration
  order: 9
---

# Migration Guide

Basilisk's unconfigured default enables every currently registered PEP-tagged
rule. That is a configuration property, not a claim that the implementation is
complete or conformant; the current level is temporarily unknown during the
[integrity remediation](/docs/conformance/). Extra Basilisk rules—required annotations, explicit-`Any` policy, required
`@override`, style, redundancy, dependency hygiene, and stub hygiene—are opt-in.
Migration therefore means choosing the policy you want, checking the project,
and recording narrow exceptions for the debt you cannot resolve yet.

> **Current tooling:** `basilisk migrate`, `basilisk stats`, and
> `basilisk check --only` are planned but are not implemented. The steps below
> use configuration and commands that exist today. The visual editor is
> described at the end of this page.

## 1. Add canonical project configuration

Start with the paths you own. Add `python-version` only when deliberately
overriding project/interpreter evidence; the pinned typing specification defines
how version checks behave, not a default target
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/directives.rst)):

```toml
[tool.basilisk]
include = ["src/", "tests/"]
exclude = [
  "__pycache__", ".venv", "site-packages", "build", "dist",
  "**/migrations/**", "**/generated/**",
]
```

Setting `exclude` replaces Basilisk's built-in list, so retain every default you
still need. See the complete [configuration reference](/docs/configuration/).

A legacy root-level `basilisk.json` is not read by anything. If one still
exists it is inert — no tool loads it, and the configuration editor does not
surface it at all. Translate its keys into `[tool.basilisk]` (camelCase →
kebab-case, e.g. `typeshedPath` → `typeshed-path`), move its per-rule and
per-tag severities into `[tool.basilisk.rules]` and
`[tool.basilisk.rule-tags]`, then delete the file.

## 2. Choose the target policy

The PEP rule set needs no switch. Opt into Basilisk rules individually, or
grade a whole tag with one written line:

```toml
[tool.basilisk.rule-tags]
"basilisk" = "error"     # every opt-in house rule on

[tool.basilisk.rules]
"BSK-0011" = "warning"   # then grade individual rules over their tag entry
```

Rules are organised by their live
[provenance, PEP-category, and descriptive tags](/docs/rules/). Basilisk has no
basic/standard/strict mode; the policy is exactly the rule and tag entries
persisted in the file. See
[Rules: two flat maps](/docs/configuration/#rules-two-flat-maps).

## 3. Run the checker and fix safe debt first

```bash
basilisk check src/ tests/
basilisk fix src/ tests/
```

`basilisk fix` applies deterministic Safe fixes by default. Re-run the checker
afterward so the remaining list represents debt that still needs a decision.

## 4. Demote only what cannot be fixed now

Prefer a visible warning/info over hiding a rule completely:

```toml
[tool.basilisk.rules]
"returns_compatibility" = "warning"
"imports_unresolved" = "info"
```

Accepted severities are `error`, `warning`, `info`, and `disabled`. An explicit
non-disabled severity also enables an opt-in rule; removing the entry returns it
to inherited tag/default selection.

For legacy areas, keep the exception local by placing a `pyproject.toml` with
a `[tool.basilisk]` table in that folder — the nearest deciding table wins per
rule, so the rest of the tree keeps the strict grading:

```toml
# legacy/pyproject.toml
[tool.basilisk.rules]
"returns_compatibility" = "warning"
"imports_unresolved" = "info"
```

There are no glob path patterns or per-path override tables — scoped policy is
always a config file in the scoped folder. A project-wide `disabled` entry
hides future violations too (and is invalid for PEP rules, which can only be
graded); `basilisk adopt` is the safer tool when the debt is confined to
existing code.

## 5. Preserve and audit inline ignores

Basilisk recognises standard ignores:

```python
value = legacy_api()  # type: ignore[returns_compatibility]
```

Bare `# type: ignore` suppresses everything on the line. A mypy/Pyright code
that is not a Basilisk code also falls back to blanket PEP 484 behavior, so
replace foreign codes with the matching Basilisk code where possible:

```python
# Broad compatibility fallback
value = legacy_api()  # type: ignore[arg-type]

# Auditable Basilisk-specific suppression
value = legacy_api()  # type: ignore[calls_argument_type]
```

You can demote without hiding:

```python
value = legacy_api()  # type: warning[calls_argument_type]
```

The `suppressions` tag contains opt-in diagnostics for active specific
(`BSK-0060`), active blanket (`BSK-0061`), unused (`BSK-0062`), and malformed
(`BSK-0063`) directives. They emit nothing by default; configure each rule at
error, warning, info, or disabled to navigate ignores workspace-wide.

## From Pyright

Copy the settings you actually use instead of translating a mode name. Typical
manual mappings are:

| Pyright | Basilisk |
|---|---|
| `pythonVersion` | `[tool.basilisk].python-version` |
| `include` / `exclude` | `[tool.basilisk].include` / `exclude` |
| `stubPath` | `[tool.basilisk].stub-paths` |
| `typeshedPath` | `[tool.basilisk].typeshed-path` |
| `report…` severity | `[tool.basilisk.rules]."RULE_CODE"` |
| execution-environment exception | a `pyproject.toml` with `[tool.basilisk]` in that folder |

Use the [Pyright configuration reference](https://microsoft.github.io/pyright/#/configuration)
to inspect the source value and Basilisk's [rule reference](/docs/rules/) to
choose the corresponding diagnostic. Do not mechanically map
`typeCheckingMode`: Basilisk intentionally stores explicit rule policy instead
of a mode.

## From mypy

Typical manual mappings are:

| mypy | Basilisk |
|---|---|
| `python_version` | `[tool.basilisk].python-version` |
| `exclude` | `[tool.basilisk].exclude` (gitignore-style patterns) |
| `mypy_path` | `[tool.basilisk].stub-paths` when it contains stubs |
| `custom_typeshed_dir` | `[tool.basilisk].typeshed-path` |
| per-module relaxations | a folder-scoped `pyproject.toml` where the exception is source-path based |
| `# type: ignore[code]` | keep the syntax, replace the foreign code with a Basilisk code |

Consult the [mypy configuration reference](https://mypy.readthedocs.io/en/stable/config_file.html)
for the exact meaning of each source option. Mypy plugins do not load into
Basilisk; use targeted path/rule exceptions for framework-specific debt rather
than disabling unrelated checks globally.

## Visual strict-first migration

The VS Code configuration editor provides this workflow as one review surface:

1. browse the canonical rule catalog by tags;
2. turn a whole tag on with one `rule-tags` entry (e.g. `"basilisk" = "error"`)
   and preview the effect before applying;
3. run Safe fixes through the separate root-scoped LSP action;
4. group remaining debt by tag and rule;
5. grade selected rules down explicitly, or record the debt with
   `basilisk adopt`;
6. track exceptions and opt-in suppression diagnostics until they graduate.

Adoption debt is stored as ordinary warning-severity rule entries in the same
active project config. It does not create a second config file or a persistent
mode.

It is [specified](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/LSP-CONFIGURATION-EDITOR-SPEC.md)
and tracked in the [implementation plan](https://github.com/Nimblesite/Basilisk/blob/main/docs/plans/LSP-CONFIGURATION-EDITOR-PLAN.md).
The VSIX is a rendering shell; the reusable LSP owns catalog, preview, config
writing, analysis, safe-fix execution, and adoption.
