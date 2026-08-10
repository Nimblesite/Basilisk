# Checker Result Cache — Specification {#CHKCACHE}

> **SUPERSEDED — historical record.** The cross-session result cache described below is deleted. It persisted diagnostics so a fresh `basilisk check --cache` could skip unchanged files; the CLI is inert ([WITHDRAWAL-INERT](DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL-INERT)) and produces no results to cache. Kept as the record of what was built.

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
3. **Effective configuration** — the resolved `BasiliskConfig` (the
   nearest-first rule-severity chain, include/exclude patterns, stub paths,
   the `typeshed-*` keys, `python-version`/`python-platform`, narrowing
   toggles, …) (`CHKCACHE-INPUT-CONFIG`). The cache's own keys
   (`CHKCACHE-CONFIG`) ride in that fingerprint like any other field, which is
   harmless: they select *whether and where* entries are kept, never what the
   diagnostics are.
4. **Resolution environment** — import search roots, site-packages dirs, the
   selected standard-library source path, and `uv.lock` contents when
   present; a change can change *which* files an import resolves to
   (`CHKCACHE-INPUT-ENV`).
5. **Checker version** — `CARGO_PKG_VERSION`; a new binary may change rule logic,
   so it invalidates every entry (`CHKCACHE-INPUT-VERSION`).
6. **Standard-library typeshed identity** — the exact
   [`python/typeshed`](https://github.com/python/typeshed) commit SHA, custom-tree
   content identity, or bundled-ZIP identity
   ([`STUBRES-TYPESHED-PIN`](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-PIN),
   [`STUBRES-TYPESHED-BASELINE`](CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-BASELINE)).
   The selected identity moves **independently of the binary**: a
   `typeshed-commit` change swaps the stdlib `.pyi` bodies under a fixed
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

`typeshed_id` is the resolved `python/typeshed` commit SHA of the downloaded archive,
the custom-tree content identity, or the bundled-ZIP identity
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

## `CHKCACHE-CONFIG` — Project configuration {#CHKCACHE-CONFIG}

Caching is a **property of the project**, not of the command line that happened
to be typed: a repository that wants warm CI re-runs wants them from every
machine and every invocation. The standing policy therefore lives in
`pyproject.toml` `[tool.basilisk]` alongside every other project setting
([`CHKARCH-CONFIG-FILE`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-FILE)),
and the flags are a per-run override of it.

| Key | Type | Unset | Effect |
|---|---|---|---|
| `cache` | boolean | off | Run the persistent result cache (`CHKCACHE-CONFIG-ENABLE`) |
| `cache-dir` | string | `.basilisk/cache/check` under the project root | Where entries live; a relative path anchors to the project root, never the caller's cwd (`CHKCACHE-CONFIG-DIR`) |

Both keys merge nearest-first with every other non-rule field
([`CHKARCH-CONFIG-DISCOVERY`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-DISCOVERY)),
and the project-ROOT config is the one that decides — one project, one cache.
An unwritten `cache` key is off, so adding these keys to the model changed no
existing project's behaviour. A wrongly-typed value (`cache = "yes"`) is
dropped by the parser and **rejected outright** by the configuration editor
([`CONFIGEDITOR-SOURCES`](LSP-CONFIGURATION-EDITOR-SPEC.md#CONFIGEDITOR-SOURCES)),
so a setting the checker will never honour is never displayed as one.

`BasiliskConfig::cache_directory` is the single resolver for `CHKCACHE-DIR`:
the CLI writes entries through it and the configuration editor displays through
it, so the folder the editor shows is provably the folder the run uses
(`CHKCACHE-CONFIG-ONE-RESOLVER`).

### `CHKCACHE-CONFIG-SALSA` — What is NOT configured here {#CHKCACHE-CONFIG-SALSA}

Basilisk caches on two layers, and only one of them is configuration:

| Layer | Lifetime | Configured by |
|---|---|---|
| **Persistent result cache** (this spec) | across processes, on disk | `cache` / `cache-dir` |
| **In-session Salsa memoization** ([`CHKARCH-INCREMENTAL-SALSA`](CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA)) | one LSP session, in memory | **nothing — always on** |

The Salsa layer has **no key and no switch**, by design: `parse -> resolve ->
check` is one memoized query per file, an edit re-executes only the affected
file's query, and there is no alternative code path to select. Its absence from
`[tool.basilisk]` is a stated fact, not an omission — the configuration editor
reports it read-only next to the keys above precisely so a reader is never left
guessing whether the single visible "cache" switch is all the caching there is
([`LSPCFGED-CACHE`](LSP-CONFIGURATION-EDITOR-SPEC.md#LSPCFGED-CACHE)).

---

## `CHKCACHE-CLI` — CLI surface {#CHKCACHE-CLI}

The flags override `CHKCACHE-CONFIG` for one run and never write configuration:

- `--cache` — run the cache regardless of `[tool.basilisk] cache`. (`CHKCACHE-CLI-ENABLE`)
- `--no-cache` — skip the cache regardless of `[tool.basilisk] cache`; wins over
  `--cache` when a command line states both, because an explicit opt-out is the
  safer reading of a contradictory command line. (`CHKCACHE-CLI-DISABLE`)
- `--cache-dir <DIR>` — override the cache location for this run. (`CHKCACHE-CLI-DIR`)
- `--cache-stats` — print `cache: N hit / M miss` to stderr after the run, so a
  warm run is detectable. Hit/miss is also emitted via `tracing`.
  (`CHKCACHE-CLI-STATS`)

Precedence, highest first: `--no-cache`, then `--cache`, then
`[tool.basilisk] cache`, then off. `basilisk adopt` always runs cold: it
rewrites the very configuration an entry is fingerprinted against.

Disabled (the default, with no key and no flag) means behaviour is
byte-for-byte identical to before the cache existed.

---

## `CHKCACHE-TEST` — Test obligations (coarse e2e) {#CHKCACHE-TEST}

1. `CHKCACHE-TEST-HIT` — second cached run yields identical diagnostics.
2. `CHKCACHE-TEST-TARGET` — editing the target between runs yields fresh
   diagnostics, never the stale cached set.
3. `CHKCACHE-TEST-DEP` — editing an imported dependency yields fresh diagnostics.
4. `CHKCACHE-TEST-CONFIG` — a config change forces a miss.
5. `CHKCACHE-TEST-TYPESHED` — a change to the resolved typeshed commit (or a
   switch between an archive, custom tree, and bundled ZIP) forces a miss.
6. `CHKCACHE-TEST-DISABLED` — without `--cache`, no cache dir is created and
   output is unchanged.
7. `CHKCACHE-TEST-STATS` — `--cache-stats` reports a miss then a hit across two
   runs.
8. `CHKCACHE-TEST-CONFIG-ENABLE` — `[tool.basilisk] cache = true` produces a
   cold-then-warm run with **no flag at all**, and creates the default folder.
9. `CHKCACHE-TEST-CONFIG-DIR` — `cache-dir` relocates the entries, resolving a
   relative path against the project root, and the default folder is not created.
10. `CHKCACHE-TEST-CONFIG-OVERRIDE` — `--no-cache` beats a configured
    `cache = true` and beats `--cache`; `--cache` beats a configured
    `cache = false`.
