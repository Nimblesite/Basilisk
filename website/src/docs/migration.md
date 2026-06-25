---
layout: layouts/docs.njk
title: "Migrate to Basilisk from Pyright or mypy"
description: "Step-by-step migration guide from Pyright or mypy to Basilisk. Automatic config import, per-path overrides, and incremental adoption strategy."
keywords: migrate to basilisk, from pyright, from mypy, python type checker migration
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Migration
  order: 9
---

# Migration Guide

Basilisk includes tooling to help existing codebases adopt it gradually. A full migration can take weeks on large codebases; the tools below make the process tractable.

---

<h2 id="from-pyright">From Pyright</h2>

### Step 1 — Import your configuration

If you have a `pyrightconfig.json`, Basilisk can read it and produce the equivalent `[tool.basilisk]` config:

```bash
basilisk migrate --from pyright
```

This reads `pyrightconfig.json` from the current directory and appends the equivalent settings to your `pyproject.toml`.

**Example input (`pyrightconfig.json`):**

```json
{
  "include": ["src"],
  "exclude": ["**/migrations/**"],
  "typeCheckingMode": "strict",
  "pythonVersion": "3.12"
}
```

**Generated output (`pyproject.toml`):**

```toml
[tool.basilisk]
python-version = "3.12"
include = ["src/"]
exclude = ["**/migrations/**"]
```

### Step 2 — Run Basilisk

```bash
basilisk check src/
```

If you were already using Pyright with `typeCheckingMode = "strict"`, you will see relatively few new errors. The most common additions:

| Pyright rule | Basilisk equivalent |
|---|---|
| `reportMissingTypeArgument` | BSK-E0015 |
| `reportReturnType` | BSK-E0002 |
| `reportArgumentType` | BSK-E0012 |
| `reportAttributeAccessIssue` | BSK-E0050 |
| `reportUndefinedVariable` | BSK-E0018 |
| `reportOperatorIssue` | BSK-E0014 |

Basilisk adds errors that Pyright does not flag even in strict mode:
- **BSK-E0011** — Implicit `Any` in parameter and return position
- **BSK-E0023** — Non-exhaustive `match` statement (missing cases)
- **BSK-E0025** — Missing `@override` decorator on overriding methods

### Step 3 — Handle `# type: ignore` comments

Pyright uses `# type: ignore` for inline suppressions. Basilisk requires a different format with a mandatory reason:

```python
# Pyright
x = get_value()  # type: ignore[reportArgumentType]

# Basilisk
x = get_value()  # basilisk: ignore[BSK-E0012] -- third-party API mismatch, tracked in #456
```

Basilisk will flag bare `# type: ignore` comments since it doesn't recognise them. The migration tool suggests the correct `# basilisk: ignore[CODE] -- reason` format.

### Step 4 — Adopt gradually with per-path overrides

For a large codebase, soften or disable the noisiest rules in legacy directories and tighten them as you go:

```toml
[tool.basilisk]
python-version = "3.12"
include = ["src/"]

[tool.basilisk.per-path-overrides."legacy/**"]
rules."BSK-E0011" = "warning"   # demote the noisiest rule to a warning
```

You can also drop `# basilisk: relaxed` at the top of an individual file to turn all of its errors into warnings while you work through it.

### Strict mode differences

If you used Pyright with `typeCheckingMode = "basic"` or `"standard"`, you will see significantly more errors with Basilisk — because those modes allow untyped code. This is expected and is the point. Use per-path overrides to phase in enforcement on a directory-by-directory basis.

---

<h2 id="from-mypy">From mypy</h2>

### Step 1 — Import your configuration

If you have a `mypy.ini` or `setup.cfg` with `[mypy]` section:

```bash
basilisk migrate --from mypy
```

**Example input (`mypy.ini`):**

```ini
[mypy]
python_version = 3.12
strict = True
ignore_missing_imports = True
exclude = migrations/
```

**Generated output:**

```toml
[tool.basilisk]
python-version = "3.12"
exclude = ["**/migrations/**"]
# note: ignore_missing_imports → BSK-E0010 is still active;
# use per-path overrides for specific packages without stubs
```

### Step 2 — The mypy `--strict` flag is a subset

mypy's `--strict` enables a specific set of flags. Basilisk enforces all of them and more. When migrating from `mypy --strict`, expect:

- **More errors from BSK-E0011** — mypy permits `Any` in some positions that Basilisk does not
- **More errors from BSK-E0023** — non-exhaustive `match` statements that mypy does not check
- **More errors from BSK-E0025** — missing `@override` decorators that mypy does not require

### Step 3 — mypy plugins

mypy plugins (Django, SQLAlchemy, Pydantic) do not work with Basilisk. Basilisk's WASM plugin system is planned for Phase 5. Until then, you may see errors for framework-specific patterns that mypy plugins previously suppressed.

Workaround while waiting for WASM plugins:

```toml
[tool.basilisk.per-path-overrides."models/**"]
disabled = ["BSK-E0011"]  # Django model fields use Any extensively
```

### Step 4 — Update inline suppressions

mypy uses `# type: ignore[error-code]`. Basilisk requires `# basilisk: ignore[BSK-EXXXX] -- reason`.

The migration tool generates a list of every `# type: ignore` comment in your codebase with the suggested Basilisk equivalent and a reminder to add a reason.

### Step 5 — Remove the daemon

mypy's daemon (`dmypy`) is needed because mypy is slow. Basilisk's incremental computation is built-in. Replace your mypy CI step:

```yaml
# Old (mypy)
- run: dmypy run -- src/

# New (Basilisk)
- run: basilisk check src/
```

---

## General migration advice

### Start with missing annotations

BSK-E0001 (missing parameter annotations) and BSK-E0002 (missing return annotations) tend to cascade. Fixing them first often resolves downstream errors automatically.

Run with just these rules initially:

```bash
basilisk check --only E0001,E0002 src/
```

### Use per-path overrides for legacy directories

Don't try to type everything at once. Soften the noisiest rules in legacy paths and keep new code strict:

```toml
[tool.basilisk.per-path-overrides."legacy/**"]
rules."BSK-E0011" = "warning"

[tool.basilisk.per-path-overrides."new_modules/**"]
# Full strictness immediately for new code
```

### Track progress with `basilisk stats`

```bash
basilisk stats src/
```

Output:

```
Type coverage report — src/
  Files:     142 total, 98 fully typed, 44 partially typed
  Functions: 1,847 total, 1,203 typed (65%)
  Classes:   318 total, 241 typed (76%)

  Top offenders (most errors):
    src/legacy/data_pipeline.py   — 47 errors
    src/utils/converters.py       — 23 errors
    src/models/legacy_models.py   — 19 errors
```

Use this report weekly to track migration progress.

### Prioritise the critical path

In practice, errors in the most-imported modules are the highest priority because they can cause type errors in callers too. Fix the deepest shared utilities first.
