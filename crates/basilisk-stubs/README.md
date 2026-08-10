# basilisk-stubs

> **A record, not a product claim.** Basilisk is unlisted and its type checker is
> inert ([WITHDRAWAL](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL)).
> Nothing described below ships in anything a user can install: the `basilisk`
> binary analyses nothing, and the editor extensions carry no checker. This file
> is kept as an account of what was built, and nothing in it authorises
> rebuilding what it describes.

Standard-library type resolution for Basilisk: a custom `python/typeshed`
tree, or a pinned commit verified offline against the on-disk store, with a
bundled full-`stdlib/` ZIP snapshot as the default pin.

## Role in Basilisk

This crate supplies step 3—"Typeshed stubs for the standard library"—of the
pinned typing resolution order
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
It returns a `StubResolution` whose active source IS the trust story; the
normative selection contract is
[STUBRES-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED).

## Key concepts

- **Offline pin verification** — an explicit `typeshed-commit` (or the bundled
  commit when unset) supplies real `.pyi` bodies, `stdlib/VERSIONS`, and the
  distribution map from one SHA. Resolution **never downloads**: it re-hashes
  the store entry's materialized tree against the pin's commit object, offline,
  on every activation. A pin that is not on this machine is a terminal
  `NO SOURCE` failure naming `basilisk typeshed download` — never a fallback
  ([STUBRES-TYPESHED-OFFLINE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-OFFLINE),
  [STUBRES-TYPESHED-PIN](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-PIN)).
  Downloading lives solely in `basilisk-typeshed-fetch`, behind explicit user
  actions, and writes immutable entries into the content-addressed store this
  crate reads
  ([STUBRES-TYPESHED-STORE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-STORE),
  [STUBRES-TYPESHED-DOWNLOAD](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-DOWNLOAD)).
- **Bundled ZIP snapshot** — a complete typeshed `stdlib/` tree with **real
  `.pyi` bodies** plus its composite `LICENSE`, pinned to one SHA and refreshed
  per release. It is the offline floor, so #288/#289 hovers work with no network
  ([STUBRES-TYPESHED-BASELINE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE),
  [STUBRES-TYPESHED-WARN](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
- **No mixed source** — custom or store-backed pinned content wholly bypasses
  the bundled snapshot.
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

Consumed only by crates that ship in nothing.
