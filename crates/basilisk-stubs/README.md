# basilisk-stubs

Standard-library type resolution for Basilisk: the runtime `python/typeshed`
clone and on-disk cache, plus a loose bundled baseline for the offline day-one
fallback.

## Role in Basilisk

This crate owns **where standard-library and third-party type information comes
from**. Its canonical source is a real on-disk clone of
[`python/typeshed`](https://github.com/python/typeshed) that Basilisk acquires
and keeps current at runtime — resolved against the clone's actual
`stdlib/*.pyi` bodies, its `stdlib/VERSIONS` module-name set, and its
`stubs/<DIST>/` trees for the `types-<distribution>` map. A small bundled
baseline backs it up so the checker still works on the very first run with no
network. The checker asks this crate to resolve an imported module and gets back
a `StubResolution` tagged with where the types came from and how much to trust
them. See
[STUBRES-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED).

## Key concepts

- **Runtime typeshed clone (canonical)** — on LSP startup, and before the first
  CLI check, Basilisk clones `python/typeshed` into an on-disk cache and
  resolves the standard library against its real `.pyi` bodies. The clone is the
  authoritative source: types, signatures, hover, and `__init__` hints all come
  from it. Cloning is done with the pure-Rust `gix` library — no system `git`
  binary and no Python runtime — so the single-native-binary promise holds
  ([STUBRES-TYPESHED-CLONE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CLONE)).
- **Bundled baseline (offline day-one fallback)** — a small, loose, replaceable
  set of data files shipped in the package: the stdlib module-name set (in
  typeshed `VERSIONS` format) and the `types-<distribution>` map
  (`data/typeshed_stub_distributions.tsv`). It carries **names and the
  distribution map only — never stdlib `.pyi` bodies** and is **not** compiled in
  as an authoritative index. It is consulted **only** while no clone is
  available (offline, clone failed, or the first check before the clone
  finishes), and any run that falls back to it raises a CLI warning
  ([STUBRES-TYPESHED-BASELINE](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE),
  [STUBRES-TYPESHED-WARN](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
- **Clone wholesale overrides the baseline** — once a clone is available, both
  the stdlib name-set and the distribution map are read from the clone and the
  baseline is not consulted. The baseline is never authoritative; it is the
  fallback only.
- **Custom typeshed override** — the `typeshed-path` config points Basilisk at
  your own typeshed tree whose `stdlib/` becomes the canonical standard-library
  source (resolution step 3) and **disables the runtime clone entirely**. Its
  stubs carry `CustomTypeshed` provenance, so hover reads `(custom typeshed)` —
  distinct from the `(typeshed)` label used for both the clone and the baseline —
  and a MicroPython signature is never misreported as CPython's
  ([STUBRES-CUSTOM-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
- **Resolution priority** — for a module that carries type information the order
  is: user `.pyi` stubs (`stub-paths`, step 1) > workspace user code (step 2) >
  the step-3 stdlib source — `typeshed-path` custom tree, else the runtime
  typeshed clone, else the bundled baseline. Later steps (installed
  `foopkg-stubs` / `types-*` packages, then `py.typed` packages) follow for
  third-party modules
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
`basilisk-checker`. The runtime `python/typeshed` clone/cache is the canonical
path defined by
[STUBRES-TYPESHED](../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED);
its `gix`-backed acquisition, TTL refresh, and freshness reporting are tracked
against that spec.
