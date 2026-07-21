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
  as an immutable ZIP that is re-hashed on every reuse, and read through the same
  archive VFS. Cached bytes standing in for the moving `main` reference expire
  after 24 hours; bytes for an explicitly pinned commit do not, because that
  commit is content-addressed and every reuse re-hashes the ZIP against its
  recorded SHA-256. An unpinned failed acquisition falls back to the bundled
  snapshot and never reuses an older commit
  ([STUBRES-TYPESHED-ACQUIRE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE)).
- **Bundled ZIP snapshot** — a complete typeshed `stdlib/` tree with **real
  `.pyi` bodies** plus its composite `LICENSE`, pinned to one SHA and refreshed
  per release. It is the offline floor, so #288/#289 hovers work with no network
  ([STUBRES-TYPESHED-BASELINE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE),
  [STUBRES-TYPESHED-WARN](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
- **No mixed source** — custom or downloaded content wholly bypasses the bundled
  snapshot.
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
| `basilisk-parser` / `ruff_python_ast` | Parse `.pyi` files for signatures and re-exports |
| `serde` / `serde_json` | (De)serialize resolution and cache metadata |

## Status

The `typeshed-path` custom-tree override, runtime `python/typeshed` archive
acquisition, and bundled full-`stdlib/` ZIP snapshot all produce the sole active
step-3 snapshot consumed by `basilisk-checker`. Its real `.pyi` bodies and
derived indexes remain one indivisible source, as defined by
[STUBRES-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED);
the HTTPS archive download (never `git clone`), tree-SHA verification, and
source reporting are tracked against that spec.
