# Runtime typeshed clone + bundled baseline — Implementation Plan {#TYPESHEDRT}

> **Specs**: [CHECKER-STUB-RESOLUTION-SPEC §STUBRES-TYPESHED](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED) (normative model),
> [CHECKER-CACHE-SPEC](../specs/CHECKER-CACHE-SPEC.md) (cache fingerprint),
> [LSP-ANALYSIS-MODES-SPEC](../specs/LSP-ANALYSIS-MODES-SPEC.md) (startup),
> [LSP-CONFIGURATION-EDITOR-SPEC §LSPCFGED-TYPESHED](../specs/LSP-CONFIGURATION-EDITOR-SPEC.md#LSPCFGED-TYPESHED) (folder-picker)
> **Crate**: `basilisk-stubs` (owns the clone/cache + baseline loader), `basilisk-config` (keys), `basilisk-lsp` (startup, Service Info tree), `basilisk-cli` (freshness report)

This plan replaces the compile-time typeshed index **wholesale**. There is no
backward-compatibility path: after it lands, no PHF stdlib/`types-*` table is
baked into the binary and the standard-library source is a runtime clone of
`python/typeshed` with a small loose baseline as the offline fallback.

## Model recap (normative source is the spec) {#TYPESHEDRT-MODEL}

- **Canonical stdlib** = an on-disk clone of `python/typeshed`, acquired on
  startup, resolved against real `stdlib/*.pyi` + `stdlib/VERSIONS`, and the
  `stubs/<DIST>/` tree for the `types-<distribution>` map.
- **Bundled baseline** = loose, replaceable files shipped in the package: the
  stdlib name set (typeshed `VERSIONS` format) + the `types-<distribution>`
  `.tsv`. No `.pyi` bodies. Offline day-one only.
- **Override**: a successful clone wholesale supersedes the baseline; the baseline
  is read **only** when no clone is available, and every such run **warns**.
- **Determinism**: pinned `typeshed-commit` freezes the checkout; unpinned tracks
  `main` on a TTL (default `24h`); every acquire/refresh ends with `git fetch` +
  `git clean -x -f -d` + `git reset --hard`.

## Git client decision {#TYPESHEDRT-GIT}

Use **`gix`** (pure-Rust git), **not** a shelled-out system `git` and **not**
`libgit2`/`git2`. Rationale: Basilisk ships as a single native binary with **no
runtime dependencies** ([CHECKER-ARCHITECTURE-SPEC](../specs/CHECKER-ARCHITECTURE-SPEC.md));
shelling to a system `git` would silently add one. `gix` performs clone, fetch,
checkout-at-SHA, hard-reset, and clean in-process. Keep it isolated behind a
`TypeshedGit` trait so the network/git surface is one testable seam.

## Work breakdown {#TYPESHEDRT-WORK}

### 1. Teardown of the compile-time index {#TYPESHEDRT-TEARDOWN}
- Delete the `STDLIB_MODULES` list and the PHF generation from
  `crates/basilisk-stubs/build.rs`; stop `include!`-ing `stdlib_set.rs` /
  `stub_map.rs` in `src/lib.rs`.
- Ship the baseline as **loose data files** in the crate/package: a
  `VERSIONS`-format stdlib-name file + the existing
  `data/typeshed_stub_distributions.tsv`. Both are loaded at runtime.
- `is_stdlib_module` / `typeshed_stub_distribution` become lookups over the
  **resolved source** (clone if available, else baseline), not over compiled-in
  sets. Keep the same public signatures so `basilisk-checker` call sites are
  unchanged (`imports/resolve.rs`, `rules/missing_type_stubs`, `code_actions`).
- **Bench guard**: run `make bench`. Only if removing the PHF is a *material*
  regression may a compiled copy be retained as a pure optimisation behind the
  loose file. The ratchet
  ([CHKARCH-TESTING-BENCH-RATCHET](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING-BENCH-RATCHET))
  decides.

### 2. Acquire step — one function, NO manager (`basilisk-stubs`) {#TYPESHEDRT-CACHE}

The "cache" is **nothing but the cloned `python/typeshed` folder on disk**. It has
exactly three states, and acquisition is a **single function over that
trichotomy** — there is **no** stateful cache manager / `Store` subsystem, no
eviction, no LRU, no bookkeeping.

- `acquire(config) -> TypeshedSource`:
  1. If `typeshed-path` set → use it verbatim, no clone. This is the typing spec's
     custom **"canonical source"** override ([distributing §Import resolution
     ordering, step 3](https://github.com/python/typing/blob/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/docs/spec/distributing.rst)).
  2. Else resolve cache dir (`typeshed-cache-path` or OS cache default).
  3. **MISSING** (absent, or a half-cloned/corrupt tree) → clone. **OUT-OF-DATE**
     (present, unpinned, older than `typeshed-refresh-interval`) → fetch + reset.
     **CURRENT** (pinned, or within TTL) → use as-is, no network.
  4. Always finish with `clean -x -f -d` + `reset --hard <target>`.
  5. On clone/fetch failure, return the existing on-disk clone if present — a
     `Clone` with `stale: true`, resolved silently, **no** baseline warning;
     return `Baseline` **only** when no clone was ever acquired. Never error out
     ([§STUBRES-TYPESHED-CLONE](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-CLONE) is authoritative).
- Returns `TypeshedSource { Clone { path, commit, committed_at, stale } | Baseline { baseline_date } }`.
  `Clone` carries real `.pyi` bodies; `Baseline` is **names-only module
  recognition** (typeshed `VERSIONS` format, no bodies) so no consumer trusts
  absent stub data. `stale` drives the dim-amber *cloned but stale* line, distinct
  from the baseline warning ([§STUBRES-TYPESHED-WARN](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).

**Concurrency is LSP-only and conformance-invisible.** A long-running LSP that
shares the cache dir with a CLI run needs exactly two mechanics, and both live
*inside* `acquire()` — neither promotes it to a subsystem:

- **Atomic promotion** — clone/update into a temp dir, then atomic-rename into
  place, so a concurrent reader never sees a half-`reset --hard` tree.
- **Advisory process lockfile** — so the LSP and a CLI invocation never `git
  fetch` the same folder at once; the loser waits and reads the result.

Both MUST be **resolution-neutral**: they change *timing*, never *which types
resolve*. The conformance harness is a single `src/main.py` invocation over static
fixtures — one process, one shot, no concurrent readers, no mid-run reset — so
neither mechanic is ever exercised by it and neither can move the score
([CHKARCH-CONFORMANCE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)).

### 3. Config keys (`basilisk-config`, `basilisk-lsp`) {#TYPESHEDRT-CONFIG}
- Add to `crates/basilisk-config/src/parse.rs` + `crates/basilisk-lsp/src/config.rs`:
  `typeshed-commit: Option<String>`, `typeshed-cache-path: Option<PathBuf>`,
  `typeshed-refresh-interval: Duration (default 24h)`. `typeshed-path` and
  `stub-paths` unchanged.
- Merge/precedence follow the existing config-resolution rules.

### 4. Startup wiring (`basilisk-lsp`) {#TYPESHEDRT-STARTUP}
- On `initialized`, spawn the acquire task; **gate the first check** on a ready
  `TypeshedSource` so no `import os` false-flags mid-clone
  ([LSP-ANALYSIS-MODES-SPEC](../specs/LSP-ANALYSIS-MODES-SPEC.md)).
- Feed `TypeshedSource.path` into `ImportSearchPaths` as the step-3 stdlib root;
  a clone update or commit change rebuilds the derived search-path cache.

### 5. Freshness report + Service Info tree {#TYPESHEDRT-REPORT}
- **CLI**: after each run, print one **muted, low-prominence** line — dim green
  `typeshed <short-sha> · <date>` when cloned/current, dim amber
  `typeshed: bundled baseline <date> — not updated; connect to refresh` when on
  the baseline ([§STUBRES-TYPESHED-WARN](../specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN)).
- **LSP**: Service Info tree shows a spinner while acquiring, then the resolved
  cache path + freshness.

### 6. Config-UI folder-picker {#TYPESHEDRT-PICKER}
- LSP advertises the `typeshed-cache-path` and `typeshed-path` settings as
  path-typed; the VS Code extension renders a **folder-picker**
  (`showOpenDialog({ canSelectFolders: true })`) and writes the chosen path back
  — the extension registers no command the LSP does not advertise
  ([LSP-CONFIGURATION-EDITOR-SPEC §LSPCFGED-TYPESHED](../specs/LSP-CONFIGURATION-EDITOR-SPEC.md#LSPCFGED-TYPESHED)).

### 7. Checker-cache fingerprint {#TYPESHEDRT-CACHEKEY}
- The incremental cache must key on the **resolved typeshed commit** (or baseline
  identity), not the binary version, so a TTL refresh or commit change
  invalidates stale entries ([CHECKER-CACHE-SPEC](../specs/CHECKER-CACHE-SPEC.md)).

## Acceptance criteria {#TYPESHEDRT-ACCEPTANCE}

- [ ] No PHF stdlib/`types-*` table is baked into the binary (unless `make bench`
      forces a retained optimisation copy behind the loose file).
- [ ] Fresh checkout with **no network** type-checks with no false
      `imports_unresolved` on stdlib, and the CLI shows the dim amber baseline
      warning with a date.
- [ ] Online, the clone resolves real `stdlib/*.pyi` — stdlib hover / `__init__`
      hints work (closes #289 for stdlib) — and the CLI shows dim green
      `typeshed <sha> · <date>`.
- [ ] `typeshed-commit` pin → byte-identical tree, no network after first
      acquire; unpinned → refresh no more than once per `typeshed-refresh-interval`.
- [ ] Every refresh leaves a pristine tree (`git clean -x -f -d` + hard reset);
      a manually dirtied cache self-heals on next refresh.
- [ ] `typeshed-path` disables cloning; `typeshed-cache-path` only relocates it.
- [ ] Config UI offers a folder-picker for both path keys.
- [ ] **Conformance stays 100%** ([CHKARCH-CONFORMANCE](../specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE)) and the **benchmark ratchet holds**.
