# Profiler Branch Audit — `proiler-fix`

**Scope:** 4 commits (#145 CPU profile delivery, #146 memory run→result, #146 real heapprofile call tree, #147 Processes empty-state) — ~2,000 LOC across Rust, the VS Code extension, docs, and tests.
**Method:** multi-agent review (map → scoped + cross-cutting reviewers → per-finding adversarial verification). 78 findings raised, **69 verified** (5 high, 20 medium, 44 low/nit), 9 refuted.

The branch genuinely fixes the dead-ends it targets; happy paths hold. The issues below are real bugs, one correctness regression that contradicts the branch's own stated guarantee, and a long tail of robustness / limitation / test-quality gaps.

## Fix order (status)

Legend: ⬜ todo · 🟦 in progress · ✅ done

| # | ID | Sev | Status | Title |
|---|----|-----|--------|-------|
| 1 | flame-1 | High | ✅ | Flamegraph webview injects untrusted strings into `innerHTML` (XSS + breaks `<module>` rendering) |
| 2 | dap-1 | High | ✅ | Global `profileOnLaunch` corrupts "Run & Track Memory" (double-start + breakpoint strip) |
| 3 | memui-2 | High | ✅* | "Never silently produces nothing" violated; captured data deleted on failure paths |
| 4 | ux-2 | High | ↩️ | `gc.set_debug(DEBUG_SAVEALL)` never cleared → **fix REVERTED** (retention is load-bearing; see below) |
| 5 | ux-1 / pyscript-2 | High/Med | ✅ | Memory result lost on crash/`os._exit`/SIGKILL/Stop(SIGTERM); misleading "finished" message |
| 6 | pyscript-1 / pyscript-5 | Med | ✅ | `filter_traces` unanchored globs silently drop user allocations under `debugpy`/`pydevd` paths |
| 7 | memui-3 | Med | ✅ | Undefined `memoryDebugSessionId` → permanent stale "tracking" state |
| 8 | conform-4 / ux-11 | Med | ✅ | Non-atomic write + destructive early unlink loses a partially-flushed snapshot |
| 9 | procexp-2 | Med | ✅ | "No Python processes running" still lies when a filter empties a non-empty list |
| 10 | dry-1 | Med | ✅ | open-native-viewer-or-fallback duplicated & divergent (memory copy lacks try/catch + action) |
| 11 | ux-6 | Med | ✅ | Profiling silently strips user breakpoints; gutter still shows red dots, no notice |
| 12 | ux-5 | Med | ⏭️ | Memory has no live/streaming view (single end-of-run snapshot only) — **deferred (feature)** |
| — | test-seams | Med | ⬜ | dap-2/tests-1, tests-2, tests-4, tests-5, heaptree-2, memui-9, tests-6 (assertiveness) |
| — | dup/size | Med/Low | ⬜ | flame-2, heaptree-1, pyscript-4, dry-2/3/4/5, dap-6, conform-1/2/3 (>500 LOC) |
| — | conform-5 | Process | ✅ | Merging re-flips `[PROFILE-UI-GATE]` (re-ships UI that #150 reverted) — **signed off: profiling ships enabled; the gate is deleted** |

### Progress — session 1

**Done (10 fixes, all with tests where a seam exists):** flame-1 (+flame-2/6/7), dap-1, memui-2, ux-1/pyscript-2, pyscript-1/pyscript-5, memui-3, conform-4/ux-11, procexp-2, dry-1, ux-6.
**Attempted then reverted:** ux-2 (the `gc.collect()`-at-exit fix emptied the headline allocation under debugpy teardown — caught by the full extension suite; see ux-2 above).
**Verification:** full `make test` re-run after the revert (Rust + coverage gate, VS Code extension, Neovim, Zed) · `cargo clippy --lib --tests` clean · `cargo fmt --check` clean · injected Python AST-validated · anchoring + at-exit retention behaviorally verified · `tsc -p ./` clean · `eslint src/` clean.

**Remaining (not yet done):**
- **ux-5** — live/streaming memory view (new feature, deferred).
- **Test-seams** — memui-2 failure-path tests + memui-9 (need a store/client seam); a full-pipeline memory e2e with a `debugpy_utils` allocation (pyscript-1); tests-1/dap-2 (proxy stateful flow), tests-2 (`exceptionOptions`), tests-4/tests-5, heaptree-2 (source-line label coverage), tests-6 (restore the removed #82 assertion).
- **Dedup/size** — heaptree-1 (`file_basename`), pyscript-4, dry-2/3/4/5, dap-6; file-size splits conform-1 (`scripts.rs`), conform-2 (`memory-profiler.ts`), conform-3 (`dap-proxy.ts`).
- **conform-5** — reviewer sign-off that merging deliberately re-ships the profiling UI (#150 revert) is intended.
- **Low/nit tail** — see the Low/Nit section (heaptree-4 recursion guard, ux-8 fork, dap-3 attach, flame-3/8, etc.).

> ✅ Full `make test` passes end-to-end after the ux-2 revert: all Rust crates meet their coverage thresholds, VS Code extension **405 passing** (91% ≥ 87%), Neovim (45% ≥ 39%) and Zed suites green — `✓ All tests passed`. The full suite caught the ux-2 regression that local Rust+tsc+eslint did not, which is why it's the gate.

---

## 🔴 High

### flame-1 — Flamegraph webview interpolates untrusted frame names/paths into `innerHTML`
`vscode-extension/src/profiler-flamegraph-html.ts:384-410` (`flamegraphScriptRender`)
- **What/why:** `tr.innerHTML = '<td>' + fn.name + '</td>'` plus `basename(fn.file)`/`basename(line.file)` with **no escaping**. `fn.name`/`fn.file` come from the profiled (possibly third-party) program (`cooperative.rs:72`, `code.co_name`/`co_filename`). (1) **Correctness:** the most common frame is `<module>` (also `<lambda>`/`<listcomp>`/`<genexpr>`), parsed by the browser as an element → top-of-stack cell renders blank/garbled on essentially every profile. (2) **Security:** `enableScripts:true`, no CSP; the webview script holds `acquireVsCodeApi()` and can post `navigateToSource`/`openSpeedscope` (open arbitrary files / external URLs).
- **Fix:** reuse the existing shared `PROFILER_JS_UTILS.escapeHtml` (`profiler-styles.ts:132`), wrap every interpolated `fn.name`/`fn.file`/`line.file`, prefer `textContent`. Don't add a 4th `escapeHtml` copy. (Also `basename` is reinvented inline here — use the shared one.)
- **✅ Resolved:** flamegraph now imports `PROFILER_JS_UTILS` (shared `escapeHtml`/`basename` — removes the inline `basename` dup, closing **flame-2**), escapes `fn.name`/`basename(fn.file)`/`basename(line.file)` before `innerHTML`, escapes `<` in the embedded JSON to prevent a `</script>` breakout, and gates the inline script behind a per-render CSP nonce with `default-src 'none'` (closing **flame-6**). New e2e test locks all of it (closing **flame-7**). tsc + eslint clean.

### dap-1 — Global `profileOnLaunch` contaminates a "Run & Track Memory" run
`vscode-extension/src/debug-adapter.ts:418-433` (`applyDebugConfigDefaults`)
- **What/why:** Stamps `profileOnLaunch:true` onto **every** `basilisk-debug` launch when the global setting is on, with no `memoryTrackOnLaunch` carve-out. A memory launch then (a) gets breakpoints stripped by the proxy and (b) **starts CPU profiling concurrently** — cooperative sampler vs tracemalloc fighting over the single entry pause (`profiler.ts:105` CPU-start guard ignores memory state).
- **Fix:** add `resolved.memoryTrackOnLaunch !== true` to the predicate **and** have `shouldProfileOnLaunch`/CPU auto-start bail when `memoryTrackOnLaunch === true`. Note `debug-integration.test.ts:1772` currently cements the buggy behavior — update it.
- **✅ Resolved:** `applyDebugConfigDefaults` no longer stamps memory launches; `shouldProfileOnLaunch` returns false for `memoryTrackOnLaunch` sessions (covers the global-setting path it reads directly). Retitled the over-broad #145 test and added two carve-out tests (memory launch not stamped; memory session never CPU-auto-profiled with global on). tsc + full eslint clean.

### memui-2 — `finalizeMemorySessionOnEnd` silently produces nothing on failure paths
`vscode-extension/src/memory-profiler.ts:509-539`
- **What/why:** Spec + docstring promise "Stopping never silently produces nothing." Three post-capture paths show nothing: client disconnected (`if (client?.isRunning() !== true) return`), ingest null/non-snapshot (no `else`), ingest throws (only `Logger.warn`). And `readFinalSnapshot` already **deleted** the file, so the captured data is unrecoverable.
- **Fix:** honest toast on every post-capture failure; defer `unlink` until ingest succeeds.
- **✅\* Resolved (code):** all three post-capture paths now show an honest warning toast ("Captured a final memory snapshot at exit, but …": LSP not running / could not be analyzed / ingest failed `<msg>`). Deferring `unlink` for recoverability is folded into **conform-4** (item 8). Failure-path **tests** are folded into the test-seams pass (memui-9/tests-4) since they need a store/client seam that doesn't exist yet.

### ux-2 — `gc.set_debug(gc.DEBUG_SAVEALL)` never cleared in the breakpoint-free run
`crates/basilisk-lsp/src/profiler/memory/scripts.rs:46-47` (vs `stop_tracemalloc` 56-62)
- **What/why:** `DEBUG_SAVEALL` keeps every unreachable cyclic object alive; only `stop_tracemalloc()` resets it, and the run-to-completion flow never calls stop. The program runs its whole life with cyclic garbage retained → inflates the RSS/current/peak this feature reports, and can OOM a program that otherwise fits. (`stop_tracemalloc` has zero production callers.)
- **Fix:** `gc.set_debug(0)`/`gc.collect()` inside the atexit hook before measuring, or only enable `DEBUG_SAVEALL` transiently around a snapshot.
- **↩️ Reverted — the proposed fix was wrong.** I first added `gc.set_debug(0)` + `gc.garbage.clear()` + `gc.collect()` to the at-exit hook, but the full extension suite caught the regression: it **emptied the headline allocation**. debugpy runs the user script in a `runpy` namespace that is torn down when the program ends, so by at-exit time the program's own end-state objects (e.g. the fixture's module-level `CACHE`) are already unreachable — and `DEBUG_SAVEALL` is exactly what keeps them tracked so the snapshot can still show them. Collecting freed precisely those: the fixture's `memory_growth.py:9` allocation dropped from **1.5 MB×3 → gone** (6 stats → 1), failing the #146 e2e. Reverted; the hook now deliberately does **not** collect, with a code comment + test (`exit_hook_does_not_collect_before_measuring`) locking that in. **Net:** ux-2's premise is mistaken for the at-exit snapshot — the retention is by design. The genuine residual (during-run RSS/`peak` inflation under `DEBUG_SAVEALL`) is inherent to the cycle-diagnostics design and left as a documented limitation, not worth breaking the headline result for.

### ux-1 — Memory final snapshot lost on any unclean exit (+ pyscript-2: Stop/SIGTERM)
`crates/basilisk-lsp/src/profiler/memory/scripts.rs:30-52`
- **What/why:** Delivery rests entirely on Python `atexit`, which doesn't fire on crash, `os._exit`, OOM-kill, SIGKILL, **or the VS Code Stop button (SIGTERM)** — exactly the runs (leaks/OOMs, servers, long loops) a memory profiler exists for. The null path says "The program **finished** before a snapshot…" — wrong for a crash, and indistinguishable from a no-allocation run. The CPU sampler streams ticks every 0.5s; memory writes once.
- **Fix:** SIGTERM/SIGINT handler that writes-then-re-raises; ideally a rolling periodic snapshot (see ux-5). Differentiate exit-code in the message. Document the truly-unrecoverable SIGKILL/`os._exit` residue.
- **✅ Resolved:** the at-exit hook now also handles `SIGTERM` (Stop button) and `SIGINT` (Ctrl-C), captures the snapshot, then restores `SIG_DFL` and re-raises so the process still dies; the write is idempotent (`_basilisk_exit_done`) so signal + atexit never double-measure. The null-case message no longer claims the program "finished" — it names the abrupt-exit case honestly. New signal-handler test; generated Python AST-validated. *Residual:* `SIGKILL`/`os._exit`/native crash are genuinely unrecoverable (a rolling snapshot — ux-5 — would mitigate further).

---

## 🟠 Medium

- **pyscript-1** (`scripts.rs:126-133`) — `filter_traces` runs unanchored globs `'*debugpy*'`/`'*pydevd*'`/`'*_pydev*'` on the **leaf** frame *before* the anchored per-frame logic, contradicting the branch's headline guarantee. A user file under `…/debugpy_utils/app.py` is silently dropped (empirically confirmed). **Fix:** drop the globs, rely on the anchored per-frame logic + `not has_user` drop; add a `debugpy_utils` leaf test.
- **pyscript-5** (`scripts.rs:534-574`) — the guard test only string-greps the script and *asserts the globs are present*, so it passes while pyscript-1 is live. **Fix:** real test that runs the snapshot logic against a `debugpy_utils` leaf and asserts survival.
- **✅ Resolved (pyscript-1 + pyscript-5):** removed the `filter_traces` unanchored-glob pre-pass entirely; debugger filtering is now the single anchored helper applied in the loop, with the **top-N taken over survivors** (iterate size-sorted, break at the cap) so debugger noise still can't crowd the user out. The guard test now asserts the globs/`filter_traces` are *absent* and the anchored leaf-decides rule is present. Anchoring behaviorally verified (`debugpy_utils/app.py` kept; real `debugpy`/`pydevd`/`tracemalloc.py` dropped); generated Python AST-valid; clippy clean. *Follow-up:* a full-pipeline e2e with a real `debugpy_utils` allocation belongs in the memory e2e (test-seams).
- **memui-3** (`memory-profiler.ts:170-182, 448-452`) — `memoryDebugSessionId` from optional `activeDebugSession?.id`; if undefined the terminate guard never matches → never finalises, temp-file leak, "Memory: tracking" forever. **Fix:** refuse 'active' without a concrete id, or capture from `onDidStartDebugSession`, or fallback-settle on active basilisk session end. **✅ Resolved:** terminate handler now falls back to finalising on any `basilisk-debug` session ending while tracking is active when no concrete id was captured (single memory session at a time), so tracking always settles.
- **conform-4 / ux-11** (`memory-profiler.ts:548-558`, `scripts.rs:42-43`) — Python `open('w')`+`write()` is non-atomic; `readFinalSnapshot` unlinks *before* the marker check and only retries when `readFile` throws → a present-but-partial file is read, deleted, lost. **Fix:** `os.replace` atomic write, or only unlink once marker present + JSON parses. **✅ Resolved:** at-exit hook now writes a sibling `.part` temp then `os.replace`s atomically; `readFinalSnapshot` unlinks ONLY after reading a complete marker-bearing payload (missing/marker-less reads keep polling, never destroy). Path-encoding tests updated; AST valid; scripts tests + tsc + eslint green. (This also gives **memui-2** its recoverability.)
- **procexp-2** (`process-explorer.ts:299-329`, `package.json`) — "No Python processes running" gated only on `processesState == loaded`, but VS Code shows it whenever `getChildren` returns `[]`, including when a search filter or `showLaunchers=false` empties a non-empty list. **Fix:** distinguish zero-processes from zero-after-filtering (e.g. `loaded-empty` state / filter-aware welcome / `treeView.message`). **✅ Resolved:** `getChildren` now returns a non-process `MessageTreeItem` placeholder ("No process matches …" / "N launcher processes hidden …") when a filter empties a non-empty list, so the empty-welcome never shows while processes are running. New test; tsc + eslint green.
- **dry-1** (`memory-profiler.ts:487-495` vs `profiler-flamegraph-html.ts:114-131`) — open-native-viewer-or-fallback duplicated; CPU copy has try/catch + fallback, memory copy doesn't, and memory lacks the CPU side's always-present "Open" notification action. **Fix:** extract `openNativeProfileViewerBeside(path, fallback)`.
- **ux-6** (`dap-proxy.ts:56-79`) — with the global setting on, ordinary F5 runs to completion, breakpoints never hit, **red dots still shown**, no message. **Fix:** one-time toast/status; mark breakpoints inactive; reconsider auto-stripping vanilla F5. **✅ Resolved:** a profiling launch now shows a one-time-per-session info toast ("Profiling run — your breakpoints are disabled so the program runs to completion …"), gated on breakpoints actually being set, covering both Run&Profile and the global-setting F5.
- **dry-1** ✅ Resolved — extracted `openNativeProfileViewerBeside(filePath, fallback)` in the result-presentation module; both CPU (`.cpuprofile`) and memory (`.heapprofile`) route through it, so the memory side now gets the open-beside-else-fallback try/catch it lacked. tsc + eslint green.
- **ux-5** (`memory-profiler.ts` finalize/present) — single end-of-run snapshot only; no growth-over-time (spec lists it as intended). **Fix:** mirror the cooperative model — periodically append `get_traced_memory()`+top-N to a tailed file (also rescues the crash case). **⏭️ Deferred:** this is a new streaming feature (not a defect); tracked for a follow-up. The signal-handler fix (item 5) already covers the common Stop-button crash case.
- **dap-2 / tests-1** (`dap-proxy.test.ts`) — proxy's stateful `launch→profilingLaunch→setBreakpoints` flow untested (`maybeRecordProfilingLaunch`/`profilingLaunch`/`DapTcpProxy` never referenced in tests). A field-name typo would pass every test while reintroducing #145. **Fix:** drive a proxy/harness with `launch{profileOnLaunch:true}` then `setBreakpoints`, assert forwarded breakpoints stripped (+ inverse).
- **tests-2** (`dap-proxy.test.ts:37-55`) — `setExceptionBreakpoints` test never asserts `exceptionOptions:[]` (and input omits the field). **Fix:** add `exceptionOptions:[…]` to input, assert it becomes `[]`.
- **tests-4** (`memory-e2e.test.ts`) — "nothing captured" honest-message path (null `readFinalSnapshot`, marker check) entirely uncovered. **Fix:** fixture that `os._exit`s / allocates nothing; assert the honest message.
- **tests-5** (`profiler-entrypoints.test.ts:181-214`) — state tests assert the in-memory getter, not the `setContext('basilisk.processesState', …)` publish the welcome reads. **Fix:** spy on `executeCommand`/assert via the context-key seam.
- **heaptree-2** (`heapprofile.rs:166-195, 244-401`) — the source-line label feature (incl. 2 MB cap, blank/EOF fallback) has zero success-path coverage; all tests use non-existent paths. **Fix:** write a real temp source file, assert trimmed line is the label; >2 MB → basename; blank/out-of-range → basename.
- **memui-9** (`memory-e2e.test.ts`) — only happy + unrelated-session covered; partial-read, ingest-fail silence, undefined-id stale state, orphan-file cleanup all untested. **Fix:** assertive tests for each.
- **flame-2** (`profiler-flamegraph-html.ts` vs siblings) — 4 `escapeHtml` copies exist; flamegraph has none. Shared `PROFILER_JS_UTILS` already exists. **Fix:** consolidate (part of flame-1).
- **dry-2** (`memory-profiler.ts:554`) — `__BASILISK_MEM__` hardcoded in TS, duplicating Rust `SNAPSHOT_MARKER` + the TS marker module → 3 sites. **Fix:** export one TS constant; derive the Python literal from `SNAPSHOT_MARKER`.
- **conform-2** (`memory-profiler.ts`) — pushed 500 → 655 LOC (over the limit). **Fix:** extract final-snapshot lifecycle into `memory-final-snapshot.ts`.

---

## 🟡 Low / Nit (grouped)

**Spec/size/process:** conform-1 (`scripts.rs` 463→648 LOC) · conform-3 (`dap-proxy.ts` 560→610) · conform-5 (re-flips `[PROFILE-UI-GATE]`; signed off — profiling ships enabled, gate deleted, spec updated) · heaptree-7 / tests-10-style (unit tests added despite "coarse e2e only").

**CPU export:** cpu-1 (validate_exportable couples .cpuprofile to speedscope-only invariants) · cpu-2 (all-empty-stack profile passes the zero-sample guard) · cpu-3 (zero-sample test leaks temp file, asserts only `is_err()`).

**DAP proxy:** dap-3 (attach-mode never breakpoint-suppressed) · dap-4 (`setDataBreakpoints`/`setInstructionBreakpoints` not neutralised) · dap-5 (docstring omits `setFunctionBreakpoints`) · dap-6 (profiling predicate in 3 places).

**Heap tree:** heaptree-3/ux-4 (labels degrade to basename for remote/container or >2 MB/unreadable) · heaptree-4 (**unbounded tree recursion** from unclamped depth — stack-overflow risk) · heaptree-5 (distinct lines in unreadable file indistinguishable) · heaptree-6 (unreachable empty-head emits schema-invalid head) · heaptree-1 (`file_basename` duplicated with divergent non-UTF-8 behavior).

**Python scripts:** pyscript-4 (debugger-frame filtering triplicated, 3 inconsistent strategies) · pyscript-6 (drop rule keeps any site-packages alloc even with no user frame) · ux-8 (atexit fires in forked/multiprocessing children, racing/overwriting parent snapshot) · dry-3 (path→Python-literal encoding duplicated) · dry-4 (temp-file minting reinvented vs `mint_sample_file`).

**Memory UI:** memui-4 (orphan temp file leaks on stop+restart same session) · memui-5 (>500 LOC) · memui-6 (read-then-unlink-then-check duplicated with `resolveMarkerFilePayload`) · memui-7 (LSP memory sessions never torn down — accumulate) · ux-7 (`.memfinal` orphaned across reload/host crash) · tests-6 (**reduced assertiveness** — #82 "tracking survives until stopped" removed, not relocated).

**Flamegraph / viewer:** flame-3 (uncaught promise on `openSpeedscope`) · flame-4/ux-3/ux-10 (speedscope is reveal+manual drag-drop, not a deep link; short-program message has no working remedy) · flame-5 (full path in user-facing toast) · flame-6 (no CSP + `enableScripts:true`) · flame-7 (synthetic-frame/escaping cases untested) · flame-8 (`navigateToSource` opens any posted path, no guard) · dry-5 (welcome action block triplicated) · dry-6 (exportError guard repeated) · dry-7 (hand-rolled launch configs duplicate `buildProfileLaunchConfig`).

**Process explorer:** procexp-4 ("no client/disconnected" collapsed into 'loading') · procexp-5 (loading seed bypasses the setter).

**Tests:** tests-3 (#146 at-exit path not verified for call-tree quality) · tests-8 (one-sample 48/0 edge only via `[]` inputs) · tests-9 (speedscope-link test asserts HTML markers, not that the message is handled).

---

## ✅ Refuted (for the record)
pyscript-3 (Windows encoding — paths are JSON-encoded correctly) · memui-1 (partial-flush retry — but see conform-4) · memui-8 (orphan-cleanup leak) · procexp-1 & procexp-3 (reactivity/setContext ordering) · conform-6 (atexit/cooperative parity) · tests-7 (CPU e2e poll timeout) · tests-10 (Rust string-grep tests — conceded acceptable) · ux-9 (#148 reactivity — panel does live-update).
