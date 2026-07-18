# basilisk-stubs

Standard-library type resolution for Basilisk: a custom or runtime
`python/typeshed` tree plus a bundled names-only fallback.

## Role in Basilisk

This crate supplies step 3—"Typeshed stubs for the standard library"—of the
pinned typing resolution order
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
It returns a `StubResolution` with source and trust provenance; the normative
selection contract is
[STUBRES-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED).

## Key concepts

- **Runtime typeshed clone** — an explicit `typeshed-commit` or freshly verified
  `main` supplies real `.pyi` bodies, `stdlib/VERSIONS`, and the distribution
  map from one SHA. An unpinned failed acquisition never reuses an old checkout
  ([STUBRES-TYPESHED-CLONE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CLONE)).
- **Bundled baseline** — loose `VERSIONS`-format names and the distribution map,
  never `.pyi` bodies. A compiled copy may accelerate that same fallback only
  ([STUBRES-TYPESHED-BASELINE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE),
  [STUBRES-TYPESHED-WARN](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
- **No mixed source** — custom or downloaded content wholly bypasses baseline
  and compiled lookups.
- **Custom typeshed** — `typeshed-path` is the sole step-3 source when set, as
  required by the pinned "canonical source" clause; a miss proceeds to step 4
  ([STUBRES-CUSTOM-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
- **Resolution priority** — manual stubs, user code, selected stdlib source,
  stub packages, inline `py.typed` packages, then optional vendored third-party
  stubs, exactly as quoted in the pinned specification
  ([STUBRES-PEP561](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561)).

## Dependencies

| Crate | Purpose |
|-------|---------|
| `phf` | Compile-time hash lookup for the bundled baseline name-set (an in-binary acceleration of the loose fallback data, never the authoritative stdlib index) |
| `basilisk-parser` / `ruff_python_ast` | Parse `.pyi` files for signatures and re-exports |
| `serde` / `serde_json` | (De)serialize resolution and cache metadata |

## Status

The `typeshed-path` custom-tree override and the bundled baseline (stdlib
name-set + `types-<distribution>` map) are shipped and consumed by
`basilisk-checker`. Runtime `python/typeshed` acquisition is the default path
defined by
[STUBRES-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED);
its `gix`-backed acquisition and source reporting are tracked
against that spec.
