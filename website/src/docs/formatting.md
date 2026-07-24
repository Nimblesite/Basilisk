---
layout: layouts/docs.njk
title: "Python Formatting & Import Hygiene — Built Into Basilisk"
description: "Basilisk embeds the Ruff formatter in the binary — format from your editor or with basilisk format, no ruff install needed — plus native import organizing."
keywords: basilisk, python formatter, ruff formatter, format on save, organize imports, basilisk format
date: 2026-07-16
dateModified: 2026-07-16
author: The Basilisk Project
eleventyNavigation:
  key: Formatting
  order: 7.5
---

# Formatting & Import Hygiene

Basilisk formats Python **without any external tool**. The formatter is the
[Ruff formatter](https://docs.astral.sh/ruff/formatter/), compiled directly
into the `basilisk` binary — no `ruff` installation, no `PATH` lookup, no
subprocess. Output is a pure passthrough of Ruff's formatter: for the same
input and configuration, Basilisk produces the same bytes your own
`ruff format` would. The style is Black-compatible, with
[Ruff's documented deviations](https://docs.astral.sh/ruff/formatter/black/).

Run `basilisk --version` to see exactly which Ruff formatter version is
embedded in your binary.

## In your editor

Formatting ships as standard LSP capabilities, so every editor gets it from
the same server:

- **VS Code / Cursor** — Format Document, Format Selection, and format on
  save, with Basilisk as the formatter for Python files.
- **Zed** — set `"formatter": "language_server"` for Python.
- **Neovim** — `vim.lsp.buf.format()` (see `:h basilisk-formatting`).

## On the command line

```bash
basilisk format             # format the current directory in place
basilisk format src tests   # format specific paths
basilisk format --check .   # verify only — exit 1 if anything would change
```

`--check` never writes, which makes it a drop-in CI gate. Files with syntax
errors are refused (never rewritten), reported, and fail the run. The CLI and
the LSP share one engine and one configuration, so their output is
byte-identical.

## Configuration

Style options are read from the same `pyproject.toml` sections Ruff uses —
see the [Ruff formatter settings](https://docs.astral.sh/ruff/settings/#format)
for what each does:

```toml
[tool.ruff]
line-length = 100

[tool.ruff.format]
quote-style = "single"
indent-style = "space"
skip-magic-trailing-comma = false
```

To turn formatting off, set the editor setting `basilisk.formatter` to
`"none"` — the server stops advertising formatting and `basilisk format`
becomes a no-op.

## Import hygiene

Import cleanup is **Basilisk-native** — implemented on the same AST the
checker uses, not delegated to Ruff — and is always available as code
actions, independent of the formatter setting:

- **Organize imports** — sort and group imports (isort semantics).
- **Expand wildcard imports** — replace `from m import *` with the names you
  actually use.
- **Split multiple imports** — one `import` statement per line.
