# basilisk-resolver

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
| `BSK-E0018` | Undefined name |
| `BSK-E0019` | Used before assignment |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | AST input |
| `ruff_python_ast` | AST node traversal |

## Status

Complete — stable API consumed by `basilisk-checker` and `basilisk-lsp`.
