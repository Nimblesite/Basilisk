# basilisk-parser

> **A record, not a product claim.** Basilisk is unlisted and its type checker is
> inert ([WITHDRAWAL](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL)).
> Nothing described below ships in anything a user can install: the `basilisk`
> binary analyses nothing, and the editor extensions carry no checker. This file
> is kept as an account of what was built, and nothing in it authorises
> rebuilding what it describes.

Python source parser for Basilisk — wraps `ruff_python_parser` to produce a typed AST.

## Role in Basilisk

This is the **first stage** of the analysis pipeline. Every `.py` file enters the system through this crate. It parses Python source text into a strongly-typed AST using the same parser that powers [Ruff](https://github.com/astral-sh/ruff), then exposes a clean API for downstream crates.

```
source text ➜ [basilisk-parser] ➜ AST ➜ resolver ➜ checker ➜ diagnostics
```

## Key concepts

- **Wraps `ruff_python_parser`** — no custom grammar, no maintenance burden. Ruff's parser is MIT-licensed.
- **`ruff_python_ast`** — re-exports AST node types so downstream crates never depend on Ruff internals directly.
- **Error recovery** — partial ASTs are returned even when the source contains syntax errors, allowing the LSP to provide diagnostics on incomplete code.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ruff_python_parser` | Parsing Python source into AST |
| `ruff_python_ast` | AST node types |
| `ruff_text_size` | Byte-offset span types |

## Status

Consumed only by crates that ship in nothing.
