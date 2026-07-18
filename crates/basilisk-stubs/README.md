# basilisk-stubs

Standard-library type resolution for Basilisk: a custom or downloaded
`python/typeshed` tree, with a bundled full-`stdlib/` ZIP snapshot as the offline
floor.

## Role in Basilisk

This crate supplies step 3—"Typeshed stubs for the standard library"—of the
pinned typing resolution order
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
It returns a `StubResolution` with source and trust provenance; the normative
selection contract is
[STUBRES-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED).

## Key concepts

- **Runtime typeshed acquisition** — an explicit `typeshed-commit` or the latest
  verified `main` supplies real `.pyi` bodies, `stdlib/VERSIONS`, and the
  distribution map from one SHA. The source archive is **downloaded over HTTPS
  (never `git clone`)**, streamed through safety/shape/license/tree gates, cached
  as an immutable ZIP, and read through the same archive VFS. An unpinned failed
  acquisition never reuses old content
  ([STUBRES-TYPESHED-ACQUIRE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE)).
- **Bundled ZIP snapshot** — a complete typeshed `stdlib/` tree with **real
  `.pyi` bodies** plus its composite `LICENSE`, pinned to one SHA and refreshed
  per release. It is the offline floor, so #288/#289 hovers work with no network;
  a compiled name index accelerates lookups over it
  ([STUBRES-TYPESHED-BASELINE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE),
  [STUBRES-TYPESHED-WARN](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
- **No mixed source** — custom or downloaded content wholly bypasses the bundled
  snapshot and its compiled lookups.
- **Custom typeshed** — `typeshed-path` is the sole step-3 source when set, as
  Basilisk's implementation of the pinned "canonical source" SHOULD; a miss proceeds to step 4
  ([STUBRES-CUSTOM-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
- **Resolution priority** — manual stubs, user code, selected stdlib source,
  stub packages, inline `py.typed` packages, then optional vendored third-party
  stubs, exactly as quoted in the pinned specification
  ([STUBRES-PEP561](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PEP561)).

## Dependencies

| Crate | Purpose |
|-------|---------|
| `phf` | Compile-time perfect-hash index over the bundled snapshot's module and `types-<distribution>` names (a lookup accelerator, not a substitute for the `.pyi` bodies) |
| `basilisk-parser` / `ruff_python_ast` | Parse `.pyi` files for signatures and re-exports |
| `serde` / `serde_json` | (De)serialize resolution and cache metadata |

## Status

The `typeshed-path` custom-tree override and the compiled name tables (stdlib
name-set + `types-<distribution>` map, from `build.rs`) are shipped and consumed
by `basilisk-checker`. Runtime `python/typeshed` archive acquisition and the
bundled full-`stdlib/` ZIP snapshot (real `.pyi` bodies) are the default path
defined by
[STUBRES-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED);
the HTTPS archive download (never `git clone`), tree-SHA verification, and
source reporting are tracked against that spec.
