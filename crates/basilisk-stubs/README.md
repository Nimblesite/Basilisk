# basilisk-stubs

Bundled type stubs and stdlib index for Basilisk.

## Role in Basilisk

This crate provides **type information for the Python standard library and popular third-party packages** without requiring an internet connection. The checker uses these stubs to resolve types for imported modules — if a module has stubs bundled here, Basilisk knows the types of every function, class, and constant in it.

## Key concepts

- **Typeshed integration** — bundles stubs from the official [typeshed](https://github.com/python/typeshed) repository.
- **Custom typeshed override** — the `typeshed-path` config points Basilisk at a custom or forked typeshed whose `stdlib/` becomes the canonical standard-library source (resolution step 3). Its stubs carry `CustomTypeshed` provenance, so hover reads `(custom typeshed)` — distinct from the bundled `(typeshed)` — and a MicroPython signature is never misreported as CPython's ([STUBRES-CUSTOM-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
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
