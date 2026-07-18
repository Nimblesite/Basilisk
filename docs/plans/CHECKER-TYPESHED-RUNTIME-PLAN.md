# Runtime typeshed acquisition — Implementation Plan {#TYPESHEDRT}

> **Normative spec**: [STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)
> **Typing authority**: [`python/typing@6ef9f77`, distributing step 3](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)

This plan supplies the real standard-library `.pyi` data missing in
[#324](https://github.com/Nimblesite/Basilisk/issues/324), enabling the hover
information requested by [#289](https://github.com/Nimblesite/Basilisk/issues/289)
and [#288](https://github.com/Nimblesite/Basilisk/issues/288). It does not change
the typing specification's six-step resolution order.

## Model {#TYPESHEDRT-MODEL}

The pinned typing specification names "Typeshed stubs for the standard library",
notes they are "usually" vendored, and makes a configured custom tree canonical
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Implement one selected step-3 source:

1. custom `typeshed-path`;
2. `python/typeshed@main`, fetched and verified for this CLI run or LSP session;
3. the bundled names-only baseline when acquisition is unavailable.

Downloaded data replaces bundled or compiled data wholesale. There is no TTL,
historical pin, last-known-good fallback, stale state, or
Python-version-to-commit map.

## Git client {#TYPESHEDRT-GIT}

The pinned typing specification leaves the transport unspecified
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
Use `gix` behind a small `TypeshedGit` trait so Basilisk remains one native
binary and network/git behavior has one test seam.

## Work breakdown {#TYPESHEDRT-WORK}

All work preserves the pinned order that says stub packages precede inline
`py.typed` packages and optional vendored third-party stubs come last
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

### 1. Replace the compile-time index {#TYPESHEDRT-TEARDOWN}

Step 3 requires typeshed stubs, not merely a module-name classification
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- Resolve downloaded stdlib modules to real `stdlib/*.pyi` paths and load
  `stdlib/VERSIONS` plus `stubs/<DIST>/METADATA.toml` from the same commit.
- Keep the packaged baseline names-only and loose. A compiled copy is permitted
  only as an exact acceleration of that baseline and is bypassed whenever a
  custom or downloaded source is active.
- Remove the compile-time table as an authoritative source. Preserve public
  lookup signatures while moving them behind the selected source.

### 2. Acquire one source {#TYPESHEDRT-CACHE}

This implements the pinned step-3 typeshed source without changing resolution
semantics
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

`acquire(config) -> Custom | Downloaded | Baseline`:

1. Return `Custom` immediately when `typeshed-path` is configured.
2. Lock the cache directory and fetch `python/typeshed@main` into a temporary
   checkout. Existing git objects may seed the fetch but are never activated
   without successful upstream verification.
3. Validate the tree and resolved SHA, then atomically promote it and return
   `Downloaded { path, commit, committed_at }`.
4. On any acquisition or validation failure, return `Baseline`; never return a
   previous checkout. `stale` is not a source state.

The activated source is immutable for that CLI run or LSP session. The next
acquisition verifies upstream again. Atomic promotion and the process lock affect
timing only, never which source wins.

### 3. Configuration {#TYPESHEDRT-CONFIG}

The only setting the pinned typing specification calls for is a custom canonical
typeshed path
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- Keep `typeshed-path` as the canonical custom source.
- Add `typeshed-cache-path` only to relocate automatic storage.
- Do not add `typeshed-commit` or `typeshed-refresh-interval`.

### 4. Startup and reporting {#TYPESHEDRT-STARTUP}

Acquisition supplies step 3 of the pinned order before resolution begins
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- Gate the first CLI/LSP analysis on `acquire`; never publish transient
  unresolved-stdlib diagnostics.
- Build `ImportSearchPaths` from the selected source and include its identity in
  the checker-cache fingerprint.
- Report either `typeshed <sha> · <date>` or
  `typeshed download unavailable; using bundled names only`. There is no stale
  status.
- Expose folder pickers for `typeshed-path` and `typeshed-cache-path`.

## Acceptance criteria {#TYPESHEDRT-ACCEPTANCE}

The tests below enforce the pinned typing-spec source and resolution order
([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

- [ ] Online acquisition verifies current `python/typeshed@main` and resolves
      stdlib imports to `.pyi` files from one exact SHA.
- [ ] A failed update never reuses an earlier checkout; only the bundled
      names-only baseline is eligible.
- [ ] Downloaded or custom data wholly disables bundled and compiled lookups.
- [ ] `typeshed-path` is the sole step-3 source when configured; a miss proceeds
      to step 4.
- [ ] No Python version selects or guesses a typeshed commit. A known target only
      evaluates `VERSIONS` and version/platform guards.
- [ ] Hovering `unittest.mock.Mock` exposes class and constructor information
      from typeshed, covering #289.
- [ ] Hovering a built-in method such as `str.join` exposes its typeshed
      signature, covering #288 without a hand-maintained signature table.
- [ ] The six-step resolution diagram is exercised in order, including separate
      stub-package and inline-`py.typed` cases.
- [ ] The upstream conformance harness remains at 100%, and documentation checks
      contain no stale/TTL/pin/default-Python contradictions.
