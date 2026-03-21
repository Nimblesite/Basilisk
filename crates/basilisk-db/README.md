# basilisk-db

Incremental computation database for Basilisk, built on the Salsa framework.

## Role in Basilisk

This crate provides the **caching and incremental recomputation layer** that makes the LSP fast. Instead of re-analyzing an entire project on every keystroke, Salsa tracks which inputs changed and only recomputes the affected outputs — delivering sub-10ms incremental checks.

```
file edit ➜ [basilisk-db] ➜ only recompute what changed ➜ updated diagnostics
```

## Key concepts

- **Salsa framework** — the same incremental computation engine behind rust-analyzer.
- **Content-addressed caching** — ASTs and resolved modules are cached by content hash.
- **Demand-driven** — only computes what downstream queries actually request.
- **File-level granularity** — tracks dependencies at the file level for optimal invalidation.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `salsa` | Incremental computation framework |

## Status

Working — powers the LSP's incremental analysis.
