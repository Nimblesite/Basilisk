# Checker Result Cache — Specification {#CHKCACHE}

**Spec group:** `CHKCACHE`
**Status:** v1 (opt-in)
**Related:** [`CHKARCH-INCREMENTAL-SALSA`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA), [`CHKARCH-CLI`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI)

The CLI (`basilisk check`) recomputes every diagnostic from scratch on each
invocation. This spec defines an **opt-in, persistent, content-addressed result
cache** that lets an unchanged check return instantly while guaranteeing it can
never change *which* diagnostics are reported.

The cache exists so a *warm* (cache-hit) check is measurably faster than a *cold*
(full) one — and so that warm vs cold is detectable, which the benchmark relies
on.

---

## `CHKCACHE-CONTRACT` — The correctness contract (non-negotiable) {#CHKCACHE-CONTRACT}

> **A cache hit is permitted only when every input that can affect the
> diagnostics is provably byte-identical to when the entry was written. Any
> doubt is a MISS. The cache may make a check faster; it must never make it
> wrong.**

"Inputs that can affect the diagnostics" for `basilisk check <file>` are:

1. **The target file's bytes.** (`CHKCACHE-INPUT-TARGET`)
2. **The bytes of every other source/stub file the checker actually read** while
   producing the result — transitive imports and `.pyi` stubs included.
   (`CHKCACHE-INPUT-DEPS`)
3. **The effective configuration** — the resolved `BasiliskConfig` (rule
   severities, per-path/per-module overrides, excludes, stub paths, auto-stub
   mode, …). (`CHKCACHE-INPUT-CONFIG`)
4. **The resolution environment** — import search roots, site-packages dirs,
   and the `uv.lock` contents when present. A change here can change *which*
   files an import resolves to. (`CHKCACHE-INPUT-ENV`)
5. **The checker version** — `CARGO_PKG_VERSION`. A new binary may change rule
   logic or bundled stubs, so it invalidates every entry.
   (`CHKCACHE-INPUT-VERSION`)

A hit requires **all five** to match. If any differ, or anything cannot be
determined (a recorded dependency is now missing/unreadable, the entry cannot be
parsed, the config cannot be fingerprinted), the result is a MISS and the check
runs in full.

### `CHKCACHE-LIMITS` — Documented v1 boundary {#CHKCACHE-LIMITS}

The environment fingerprint (`CHKCACHE-INPUT-ENV`) hashes the search-path
configuration and `uv.lock`. Installing or removing packages **directly into a
virtualenv's site-packages without a `uv.lock` change** is not auto-detected in
v1. This is why the cache is **opt-in**: clear the cache (or omit `--cache`)
after mutating the environment outside the lockfile. Source, config, lockfile,
and version changes are always detected. This boundary is the reason v1 ships
behind a flag rather than on by default.

## `CHKCACHE-READSET` — Capturing the exact read-set {#CHKCACHE-READSET}

All checker file reads go through two functions:

- `basilisk_parser::parse_file` (target + imported `.py` sources)
- `basilisk_stubs::parse_pyi_file` (`.pyi` stubs)

Both route their `read_to_string` through `basilisk_common::fs::read_tracked`
(`CHKCACHE-READSET-FS`). When a thread-local `ReadRecorder` is active, every read
records `(canonical_path, content_hash)`. The recorder is an RAII guard
(`CHKCACHE-READSET-GUARD`): inert when absent (zero behaviour change for the LSP
and for non-cached CLI runs), active only for the duration of a cached check.

---

## `CHKCACHE-FINGERPRINT` — The fingerprint {#CHKCACHE-FINGERPRINT}

`fingerprint = hash(version ‖ config_hash ‖ env_hash ‖ sorted[(path, content_hash)…])`

where the read-set is sorted by canonical path for determinism. Hashing uses the
existing `basilisk_db::hash_source` (`DefaultHasher`); the cache stores the
**full read-set with per-file hashes** in the entry so a lookup re-verifies each
file individually rather than trusting a single rolled-up number.

---

## `CHKCACHE-ENTRY` — On-disk entry {#CHKCACHE-ENTRY}

One JSON file per target, named by the hash of the target's canonical path,
under the cache directory (`CHKCACHE-DIR`, default `.basilisk/cache/check`).
Each entry stores:

- `version`, `config_hash`, `env_hash`
- `deps`: `[(canonical_path, content_hash)]` (includes the target)
- `diagnostics`: `[CachedDiagnostic]` — an owned, serde projection of
  `Diagnostic` (`CHKCACHE-DIAG`). `docs_url` is **not** stored: it is rebuilt
  deterministically from the code. On replay, the `&'static` `code`/`docs_url`
  are produced via a bounded process-wide interner (`CHKCACHE-DIAG-INTERN`,
  ≤ one entry per distinct BSK code).

A lookup loads the entry, checks `version`/`config_hash`/`env_hash`, then
re-hashes every `deps` path against its stored hash. All match ⟹ HIT.

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
5. `CHKCACHE-TEST-DISABLED` — without `--cache`, no cache dir is created and
   output is unchanged.
6. `CHKCACHE-TEST-STATS` — `--cache-stats` reports a miss then a hit across two
   runs.
