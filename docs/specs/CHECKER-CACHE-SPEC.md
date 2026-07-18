# Checker Result Cache — Specification {#CHKCACHE}

**Spec group:** `CHKCACHE`
**Status:** v1 (opt-in)
**Related:** [`CHKARCH-INCREMENTAL-SALSA`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA), [`CHKARCH-CLI`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI), [`STUBRES-TYPESHED`](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED)

An **opt-in, persistent, content-addressed result cache** for the CLI
(`basilisk check`): an unchanged check returns from cache without changing
*which* diagnostics are reported. A warm (cache-hit) check is measurably faster
than a cold (full) one, and the difference is detectable (the benchmark relies on
this).

---

## `CHKCACHE-CONTRACT` — The correctness contract (non-negotiable) {#CHKCACHE-CONTRACT}

> **A hit is permitted only when every input that can affect the diagnostics is
> provably byte-identical to when the entry was written. Any doubt is a MISS.**

Inputs that can affect the diagnostics for `basilisk check <file>`:

1. **Target file bytes** (`CHKCACHE-INPUT-TARGET`).
2. **Bytes of every other source/stub file the checker actually read** —
   transitive imports and `.pyi` stubs included (`CHKCACHE-INPUT-DEPS`).
3. **Effective configuration** — the resolved `BasiliskConfig` (rule severities,
   per-path/per-module overrides, excludes, stub paths, auto-stub mode, …)
   (`CHKCACHE-INPUT-CONFIG`).
4. **Resolution environment** — import search roots, site-packages dirs, the
   selected standard-library source path, and `uv.lock` contents when
   present; a change can change *which* files an import resolves to
   (`CHKCACHE-INPUT-ENV`).
5. **Checker version** — `CARGO_PKG_VERSION`; a new binary may change rule logic,
   so it invalidates every entry (`CHKCACHE-INPUT-VERSION`).
6. **Standard-library typeshed identity** — the exact
   [`python/typeshed`](https://github.com/python/typeshed) commit SHA, custom-tree
   content identity, or bundled-baseline identity
   ([`STUBRES-TYPESHED-CLONE`](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CLONE),
   [`STUBRES-TYPESHED-BASELINE`](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE)).
   The selected identity moves **independently of the binary**: a fresh unpinned
   acquisition or a `typeshed-commit` change swaps the stdlib `.pyi` bodies under a fixed
   `CARGO_PKG_VERSION`, so keying only on the checker version would serve a stale
   entry across a typeshed update. The fingerprint MUST therefore key on the
   exact selected source identity as well
   (`CHKCACHE-INPUT-TYPESHED`). This preserves step 3 of the pinned typing order
   ([`python/typing@6ef9f77`](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).

A hit requires **all six** to match. If any differ, or anything cannot be
determined (a recorded dependency missing/unreadable, the entry unparseable, the
config unfingerprintable), it is a MISS and the check runs in full.

### `CHKCACHE-LIMITS` — Documented v1 boundary {#CHKCACHE-LIMITS}

The environment fingerprint (`CHKCACHE-INPUT-ENV`) hashes the search-path
configuration and `uv.lock`. Installing or removing packages **directly into a
virtualenv's site-packages without a `uv.lock` change** is not auto-detected in
v1. This is why the cache is **opt-in**: clear the cache (or omit `--cache`)
after mutating the environment outside the lockfile. Source, config, lockfile,
resolved typeshed identity, and version changes are always detected. This boundary is the
reason v1 ships behind a flag rather than on by default.

### `CHKCACHE-POSITIONING` — When this cache helps, and the Salsa endgame {#CHKCACHE-POSITIONING}

This v1 cache is **correct everywhere but only *useful* in narrow conditions**,
and the documentation must say so plainly.

- **`CHKCACHE-POSITIONING-WATCHER` — a cache is only useful with invalidation on
  edit.** A result cache earns its keep when something tells it *which* files
  changed so unchanged work can be skipped. In a long-lived process that means a
  **file watcher** that invalidates entries the moment a file is edited. v1 has
  no watcher: it is a one-shot CLI cache that lazily re-verifies the recorded
  read-set on the *next* lookup (re-reading and re-hashing every dependency).
  That keeps it correct, but its value is confined to **repeated batch runs over
  a mostly-unchanged tree** (CI re-runs, pre-commit, `basilisk check` loops). In
  an interactive editor it is the wrong shape — the LSP already holds documents
  in memory and is notified of edits, so a re-verify-on-read disk cache buys
  little.

- **`CHKCACHE-POSITIONING-SMART` — invalidation must be smart, not blanket.**
  Re-checking every file because one changed defeats the purpose. Useful
  invalidation is **dependency-aware**: an edit invalidates only the edited file
  and its transitive importers (the reverse of the read-set this cache already
  records), leaving everything else cached. v1 approximates this lazily and
  per-file; it does not maintain the reverse dependency graph needed to
  invalidate *eagerly* and *precisely* on a watcher event.

- **`CHKCACHE-POSITIONING-SALSA` — Salsa is the better vehicle for *in-session*
  invalidation.** Smart, demand-driven invalidation is exactly what a
  query-memoization engine gives for free, and the in-session engine now exists
  ([`CHKARCH-INCREMENTAL-SALSA`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA),
  `crates/basilisk-db` + `basilisk-checker`'s `checked_file` query): `parse →
  resolve → check` is a tracked query, and an edit re-executes only the affected
  file's query, leaving every other file's memo intact. Granularity today is
  **module-level** (the pipeline is fused into one tracked query per file); finer
  per-function granularity is possible but not yet implemented. The read-set this
  cache records by hand is the dependency graph Salsa tracks automatically.

  The two layers are **complementary, not redundant**: this content-addressed
  cache is the *cross-session* durable layer (a fresh process recomputes only
  files whose read-set changed on disk), while Salsa is the *in-session* layer (a
  live editing session recomputes only the file you touched). The remaining step
  is to make the salsa database itself durable across sessions — at which point
  this whole-file content-hash cache could be retired in its favour — but until
  that lands, **this cache remains the cross-session mechanism** and is not
  superseded.

---

## `CHKCACHE-READSET` — Capturing the exact read-set {#CHKCACHE-READSET}

### Tracked filesystem reads {#CHKCACHE-READSET-FS}

All checker file reads go through `basilisk_parser::parse_file` (target +
imported `.py` sources) and `basilisk_stubs::parse_pyi_file` (`.pyi` stubs), both
routing `read_to_string` through `basilisk_common::fs::read_tracked`. When a
thread-local `ReadRecorder` is active, every read records
`(canonical_path, content_hash)`.

### Recorder guard {#CHKCACHE-READSET-GUARD}

The recorder is an RAII guard: inert when absent (zero behaviour change for the
LSP and non-cached CLI runs), active only during a cached check.

---

## `CHKCACHE-FINGERPRINT` — The fingerprint {#CHKCACHE-FINGERPRINT}

`fingerprint = hash(version ‖ typeshed_id ‖ config_hash ‖ env_hash ‖ sorted[(path, content_hash)…])`

`typeshed_id` is the resolved `python/typeshed` commit SHA of the runtime clone,
or the bundled-baseline identity when no clone is available
(`CHKCACHE-INPUT-TYPESHED`) — see
[`STUBRES-TYPESHED`](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED).

The read-set is sorted by canonical path for determinism. Hashing uses
`basilisk_db::hash_source` (`DefaultHasher`); the entry stores the **full
read-set with per-file hashes** so a lookup re-verifies each file individually
rather than trusting a single rolled-up number.

---

## `CHKCACHE-ENTRY` — On-disk entry {#CHKCACHE-ENTRY}

One JSON file per target, named by the hash of the target's canonical path,
under the cache directory (`CHKCACHE-DIR`, default `.basilisk/cache/check`).
Each entry stores:

- `version`, `typeshed_id`, `config_hash`, `env_hash`
- `deps`: `[(canonical_path, content_hash)]` (includes the target)
- `diagnostics`: `[CachedDiagnostic]`, described below.

### Diagnostic projection {#CHKCACHE-DIAG}

`CachedDiagnostic` is an owned serde projection of `Diagnostic`. `docs_url` is not stored; it
is rebuilt deterministically from the code.

#### Bounded code interning {#CHKCACHE-DIAG-INTERN}

On replay, the `&'static` code/docs URL values come from a bounded process-wide interner with
at most one entry per distinct Basilisk code.

A lookup loads the entry, checks
`version`/`typeshed_id`/`config_hash`/`env_hash`, then re-hashes every `deps`
path against its stored hash. All match ⟹ HIT.

---

## `CHKCACHE-CLI` — CLI surface {#CHKCACHE-CLI}

- `--cache` — enable the cache (off by default). (`CHKCACHE-CLI-ENABLE`)
- `--cache-dir <DIR>` — override the cache location. (`CHKCACHE-CLI-DIR`)
- `--cache-stats` — print `cache: N hit / M miss` to stderr after the run, so a
  warm run is detectable. Hit/miss is also emitted via `tracing`.
  (`CHKCACHE-CLI-STATS`)

Disabled (default) ⟹ behaviour is byte-for-byte identical to today.

---

## `CHKCACHE-TEST` — Test obligations (coarse e2e) {#CHKCACHE-TEST}

1. `CHKCACHE-TEST-HIT` — second cached run yields identical diagnostics.
2. `CHKCACHE-TEST-TARGET` — editing the target between runs yields fresh
   diagnostics, never the stale cached set.
3. `CHKCACHE-TEST-DEP` — editing an imported dependency yields fresh diagnostics.
4. `CHKCACHE-TEST-CONFIG` — a config change forces a miss.
5. `CHKCACHE-TEST-TYPESHED` — a change to the resolved typeshed commit (or a
   switch between the runtime clone and the bundled baseline) forces a miss.
6. `CHKCACHE-TEST-DISABLED` — without `--cache`, no cache dir is created and
   output is unchanged.
7. `CHKCACHE-TEST-STATS` — `--cache-stats` reports a miss then a hit across two
   runs.
