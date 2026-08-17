# basilisk-resolver

> **A record, not a product claim.** Basilisk is unlisted and its type checker is
> inert ([WITHDRAWAL](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL)).
> Nothing described below ships in anything a user can install: the `basilisk`
> binary analyses nothing, and the editor extensions carry no checker. This file
> is kept as an account of what was built, and nothing in it authorises
> rebuilding what it describes.

Name resolution and scope analysis for Basilisk.

## Role in Basilisk

This is the **second stage** of the analysis pipeline. After `basilisk-parser` produces an AST, the resolver walks it to build a scope tree, resolve every name reference, and detect scope-level errors like undefined names and use-before-assignment.

```
AST ➜ [basilisk-resolver] ➜ scopes + resolved names ➜ checker ➜ diagnostics
```

## Key concepts

- **Scope tree** — builds a hierarchical scope structure (module, class, function, comprehension) tracking all bindings and references.
- **Name resolution** — links every name usage to its definition, handling Python's complex scoping rules (LEGB, nonlocal, global).
- **Type narrowing** — tracks control flow to narrow types based on `isinstance`, `is None`, truthiness checks, and pattern matching.
- **Function info** — extracts function signatures, parameter types, return types, and decorator information for downstream type checking.

## Diagnostics emitted

| Code | Description |
|------|-------------|
| `names_undefined` | Undefined name |
| `names_unbound` | Used before assignment |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | AST input |
| `ruff_python_ast` | AST node traversal |

## Status

Consumed only by crates that ship in nothing.
