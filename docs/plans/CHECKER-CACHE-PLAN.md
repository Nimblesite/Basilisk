# Checker Result Cache — Implementation Plan

Implements [`CHKCACHE`](../specs/CHECKER-CACHE-SPEC.md). Opt-in, correctness-first
CLI result cache + warm/cold benchmark wiring.

## Build order (bottom-up, build green at each step)

1. **`basilisk-common::fs`** (`CHKCACHE-READSET-FS`/`-GUARD`) — `read_tracked`,
   thread-local `ReadRecorder` RAII guard, `content_hash`. Reused by parser +
   stubs.
2. **Wire reads** — `parse_file` and `parse_pyi_file` call `read_tracked`. No
   behaviour change when no recorder is active.
3. **Serde projection** (`CHKCACHE-DIAG`) — derive Serialize/Deserialize on
   `Severity`, `Span`, `TypeProvenance`; add owned `CachedDiagnostic` with
   `From<&Diagnostic>` + `into_diagnostic` via bounded interner
   (`CHKCACHE-DIAG-INTERN`).
4. **`basilisk-db::cache`** (`CHKCACHE-ENTRY`/`-FINGERPRINT`) — generic
   `CheckCache<T>` over a serde payload: `lookup`, `store`, dep re-verification,
   cache-dir management.
5. **CLI wiring** (`CHKCACHE-CLI`) — flags, lookup→(miss: run under recorder,
   store), hit/miss counters, `--cache-stats`. Extract into
   `basilisk-cli/src/cache_check.rs` to keep `main.rs` lean.
6. **Tests** (`CHKCACHE-TEST-*`) — coarse e2e in `basilisk-cli/tests`.
7. **Benchmark** — `benchmarks/run.sh`: mypy `--no-incremental` (cold, honest);
   basilisk warm (cache) + cold (`--prepare` clears cache dir); 2 new rule
   fixtures; update `benchmark_report.py`, website data + render, status CSV
   schema.

## Key decisions (made, not open)

- **Read-recorder, not import-list inference** — the only airtight read-set.
- **Store dep list in entry, re-verify on lookup** — solves the
  "don't-know-deps-before-running" bootstrap soundly (`CHKCACHE-SOUNDNESS`).
- **`docs_url` derived, code interned** — sidesteps `&'static str` round-trip.
- **Opt-in** — until the site-packages env hole (`CHKCACHE-LIMITS`) is closed.
- **Benchmark: symmetric warm + cold for every tool.** cold = full check; warm =
  a repeat run using that tool's own cache. basilisk-warm = `--cache` hit;
  mypy-warm = incremental `.mypy_cache` hit (cold mypy = `--no-incremental`).
  pyright/ty/pyrefly keep no cross-run result cache (verified: first-ever run ==
  repeat run, zero cache artifacts), so their warm ≈ cold — which is itself the
  honest result.
- **Stepping stone, not destination** (`CHKCACHE-POSITIONING`) — a result cache
  is only *useful* with watcher-driven, dependency-aware (smart) invalidation;
  v1 has neither (it re-verifies lazily on the next lookup), so it helps batch
  re-runs but not the interactive editor. The proper long-term mechanism is the
  Salsa migration (`basilisk-db` Phase 2), which gives watcher-driven sub-file
  invalidation for free and subsumes this hand-rolled read-set.
