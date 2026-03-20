# basilisk-stubs

Bundled type stubs and stdlib index for Basilisk.

## Role in Basilisk

This crate provides **type information for the Python standard library and popular third-party packages** without requiring an internet connection. The checker uses these stubs to resolve types for imported modules — if a module has stubs bundled here, Basilisk knows the types of every function, class, and constant in it.

## Key concepts

- **Typeshed integration** — bundles stubs from the official [typeshed](https://github.com/python/typeshed) repository.
- **Offline-first** — no network requests needed. All stubs are compiled into the binary.
- **PHF lookup** — uses perfect hash functions for fast module-to-stub resolution.
- **Stub priority** — inline type annotations > bundled stubs > inferred types.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `phf` | Perfect hash map for fast stub lookup |
| `ruff_python_ast` | AST types for stub parsing |

## Status

Working — stdlib stubs bundled and consumed by `basilisk-checker`.
