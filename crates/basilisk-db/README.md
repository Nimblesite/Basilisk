# basilisk-db

> **A record, not a product claim.** Basilisk is unlisted and its type checker is
> inert ([WITHDRAWAL](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL)).
> Nothing described below ships in anything a user can install: the `basilisk`
> binary analyses nothing, and the editor extensions carry no checker. This file
> is kept as an account of what was built, and nothing in it authorises
> rebuilding what it describes.

Incremental computation database for Basilisk, built on the Salsa framework.

## Role in Basilisk

This crate provides the **caching and incremental recomputation layer** for the language server. Instead of re-analyzing an entire project on every keystroke, Salsa tracks which inputs changed and only recomputes the affected outputs.

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

Consumed only by the language server, which ships in nothing. The cross-session
result cache this crate used to hold was deleted with the checking it cached
([CHECKER-CACHE-SPEC](../../docs/specs/CHECKER-CACHE-SPEC.md)).
