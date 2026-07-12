//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Python injection scripts for memory profiling via DAP evaluate.
//!
//! These scripts are injected into a running Python process through the
//! debug adapter's `evaluate` request. They use `tracemalloc` (stdlib)
//! for allocation tracking and `gc` for reference graph introspection.

/// Start `tracemalloc` with deep tracebacks and register an `atexit` hook that
/// writes one final snapshot to `snapshot_file` as the program exits.
///
/// Injected once when memory tracking begins; `nframe` controls traceback depth
/// (default 25 frames). The at-exit hook is what makes the "Run & Track Memory
/// (Current File)" flow end in a *visible result* instead of dead-ending
/// ([PROFILE-MEMORY-FINAL]): that flow has no breakpoint, so the program runs to
/// completion and there is never a paused frame to take a snapshot from. The
/// hook captures one snapshot at process exit and writes it to a file the editor
/// reads when the debug session terminates. The payload is byte-identical to an
/// evaluate-path snapshot ([`take_snapshot`]) — both embed [`snapshot_payload_fn`]
/// — so the editor ingests the file through the same `basilisk.memory.ingest`
/// path. A direct in-process file write (not the `_basilisk_emit` print path) is
/// used because at exit there is no DAP `evaluate` round-trip listening, and
/// writing in-process sidesteps debugpy's print truncation entirely
/// ([PROFILE-MEMORY-COURIER]). The hook deliberately does NOT `gc.collect()`
/// before measuring: debugpy tears down the user script's `runpy` namespace when
/// the program ends, so the program's end-state objects are already unreachable
/// at at-exit time and only `DEBUG_SAVEALL` keeps them tracked for this snapshot
/// — collecting would free them and empty the profile (ux-2). The hook also runs
/// on `SIGTERM` and `SIGINT` — the VS Code Stop button and Ctrl-C, which bypass
/// `atexit` — then re-raises so the process still dies (pyscript-2); the write is
/// idempotent so the signal and `atexit` paths never double-measure. Only a hard
/// kill (`SIGKILL`, `os._exit`, a native crash) leaves no file, and the editor
/// reports "nothing captured" honestly.
#[must_use]
pub fn start_tracemalloc(nframe: u32, snapshot_file: &str, max_stats: usize) -> String {
    let payload_fn = snapshot_payload_fn();
    // Embed the path as a JSON-encoded Python string literal — the same
    // cross-platform-safe pattern the cooperative sampler uses
    // ([PROFILE-COOPERATIVE], `cooperative.rs`) — so a Windows backslash or a
    // quote in `TMPDIR` can't break the script. `to_string` of a `&str` is
    // infallible; the fallback only keeps this total.
    let path_literal = serde_json::to_string(snapshot_file).unwrap_or_else(|_| "\"\"".to_owned());
    format!(
        r"
import tracemalloc, gc, atexit, signal, os
{payload_fn}
_basilisk_exit_done = [False]
def _basilisk_write_exit_snapshot():
    if _basilisk_exit_done[0]:
        return
    _basilisk_exit_done[0] = True
    try:
        # Must NOT force a garbage collection here. debugpy runs the user script
        # in a runpy namespace torn down at program end, so the program's
        # end-state objects (e.g. a module-level cache) are already unreachable by
        # at-exit time; DEBUG_SAVEALL is exactly what keeps them tracked so this
        # final snapshot can still show the program's allocations. Freeing them
        # would empty the profile (ux-2 — the retention is by design).
        _payload = _basilisk_snapshot_payload({max_stats})
        # Atomic write: a reader racing the final flush must see either no file
        # or the WHOLE payload, never a truncated one it would then destroy
        # (conform-4). Write a sibling temp, then os.replace into place.
        _final = {path_literal}
        _tmp = _final + '.part'
        with open(_tmp, 'w') as _f:
            _f.write(_payload)
        os.replace(_tmp, _final)
    except Exception:
        pass
_basilisk_prev_handlers = {{}}
def _basilisk_signal_exit(_signum, _frame):
    # The VS Code Stop button terminates the debuggee with SIGTERM (SIGINT for
    # Ctrl-C), neither of which runs atexit (pyscript-2). Capture the final
    # snapshot, then CHAIN to the application's own handler — a server that
    # flushes state on SIGTERM must keep working under memory tracking, never
    # have its graceful shutdown clobbered by the profiler. With no prior
    # handler, restore the default disposition and re-raise so the process
    # still dies as the signal intended. SIGKILL / os._exit stay unrecoverable.
    _basilisk_write_exit_snapshot()
    _prev = _basilisk_prev_handlers.get(_signum)
    if callable(_prev):
        _prev(_signum, _frame)  # the app's handler decides how to die
        return
    if _prev is signal.SIG_IGN:
        return  # the app chose to ignore this signal; honor that
    signal.signal(_signum, signal.SIG_DFL)
    os.kill(os.getpid(), _signum)
tracemalloc.start({nframe})
gc.set_debug(gc.DEBUG_SAVEALL)
atexit.register(_basilisk_write_exit_snapshot)
for _sig in (signal.SIGTERM, signal.SIGINT):
    try:
        _basilisk_prev_handlers[_sig] = signal.getsignal(_sig)
        signal.signal(_sig, _basilisk_signal_exit)
    except (ValueError, OSError):
        pass  # only settable on the main thread / supported signals
print('__BASILISK_MEM_OK__')
"
    )
}

/// Script to stop tracemalloc and release resources.
#[must_use]
pub fn stop_tracemalloc() -> &'static str {
    r"
import tracemalloc, gc
tracemalloc.stop()
gc.set_debug(0)
print('__BASILISK_MEM_OK__')
"
}

/// Python helper that ferries a marker payload back to the editor through a
/// temp **file** instead of stdout.
///
/// debugpy truncates a single `print()` to ~20 KB, which silently corrupts the
/// large JSON a real `tracemalloc` snapshot produces (100 stats × deep
/// tracebacks ≈ 200 KB). So every JSON-emitting script writes its
/// `marker + json` payload to a temp file and prints only
/// `__BASILISK_MEM_FILE__<path>` (a short, never-truncated line); the editor
/// reads the file back and posts its contents to `*.ingest` unchanged
/// ([PROFILE-MEMORY-COURIER]). Local debugging only — the editor and debuggee
/// share a filesystem, exactly as the cooperative CPU sampler assumes.
///
/// The printed marker is split (`'__BASILISK_MEM_' + 'FILE__'`) so the full
/// marker never appears verbatim in the script SOURCE: pydevd quotes the
/// source in its diagnostics (e.g. the evaluation-timeout warning), and a
/// quoted-source line containing the real marker would poison the editor's
/// output scan (pyscript-6).
fn emit_via_file_helper() -> &'static str {
    r"
def _basilisk_emit(_payload):
    import tempfile, os
    _fd, _path = tempfile.mkstemp(prefix='basilisk_mem_', suffix='.txt')
    with os.fdopen(_fd, 'w') as _f:
        _f.write(_payload)
    print('__BASILISK_MEM_' + 'FILE__' + _path)
"
}

/// Python helpers building the `__BASILISK_MEM__ + json` snapshot payload:
/// `_basilisk_capture_state()` (the C-fast freeze), `_basilisk_grouped_stats`
/// (per-traceback aggregation), `_basilisk_snapshot_payload_from(_state, n)`
/// (the heavy render), and `_basilisk_snapshot_payload(n)` combining them.
///
/// Single source of truth for the snapshot payload, embedded by both the
/// evaluate-path snapshot ([`take_snapshot`]) and the at-exit final snapshot
/// ([`start_tracemalloc`]'s `atexit` hook), so both emit a byte-identical
/// payload the same `basilisk.memory.ingest` parser dispatches. Defined as a
/// plain `&'static str` (no `format!`) so the dict-literal braces stay literal.
///
/// Capture and render are SPLIT because they live under different budgets
/// (pyscript-6, the "did not finish after 3.00 seconds" bug): capture is the
/// C-level `tracemalloc.take_snapshot()` copy that must happen at pause time
/// and is fast at any heap size, while the render walks every trace in pure
/// Python (~µs per trace — SECONDS on multi-million-trace heaps) and therefore
/// runs on a worker thread in the evaluate path, never inside the paused DAP
/// `evaluate` (pydevd warns after `PYDEVD_WARN_EVALUATION_TIMEOUT` = 3 s).
/// The grouping walks the snapshot's raw trace list and merges by traceback
/// (id-keyed for speed — the C tracer interns traceback tuples — then merged
/// by tuple equality so correctness never depends on that interning), which
/// measures ~3× faster than the stdlib's `statistics('traceback')` pass; the
/// stdlib pass remains as the fallback if the raw layout ever changes.
///
/// Two deliberate choices make the resulting `.heapprofile` worth reading
/// ([PROFILE-MEMORY-FINAL]):
/// 1. Allocations whose site (leaf frame) is the debugger or snapshot machinery
///    (pydevd/debugpy/tracemalloc/`<frozen>`/`<string>`) are dropped, so the
///    profile is the *user's* program — code that merely runs under the debugger
///    keeps its allocations (the leaf frame decides). The match is ANCHORED
///    (basename / exact path segment), never an unanchored full-path substring,
///    so a user path like `debugpy_utils/app.py` is never mistaken for the
///    debugger (pyscript-1); the top-N is taken over the survivors, not the raw
///    allocations, so debugger noise can't crowd the user out.
/// 2. Full call stacks are kept per allocation (the `take_snapshot(25)` depth),
///    root→leaf, with the debugger/runtime glue stripped — so the editor builds
///    a real call tree, not a flat list.
fn snapshot_payload_fn() -> String {
    format!("{}{}", capture_and_group_fns(), render_payload_fns())
}

/// The capture + grouping halves of [`snapshot_payload_fn`] (see its docs).
fn capture_and_group_fns() -> &'static str {
    r"
def _basilisk_capture_state():
    # The C-fast freeze of everything the payload reports, taken at the moment
    # the user asked (the paused evaluate); rendering happens later (pyscript-6).
    import tracemalloc, gc
    _snapshot = tracemalloc.take_snapshot()
    _current, _peak = tracemalloc.get_traced_memory()
    return {
        'snapshot': _snapshot,
        'current': _current,
        'peak': _peak,
        'gcCounts': list(gc.get_count()),
        'gcObjects': len(gc.get_objects()),
    }
def _basilisk_grouped_stats(_snapshot):
    # [(size, count, frames_root_to_leaf), ...] — the same grouping as the
    # stdlib's statistics('traceback'), ~3x faster: one lean pass over the raw
    # trace list keyed by traceback id (the C tracer interns the tuples), then
    # a tiny per-GROUP merge by tuple equality so the result is exact even if
    # some build did not intern. Raw frames come most-recent-first; reverse
    # per group (cheap) so callers see root->leaf.
    try:
        _acc = {}
        _get = _acc.get
        for _t in _snapshot.traces._traces:
            _tb = _t[2]
            _k = id(_tb)
            _g = _get(_k)
            if _g is None:
                _acc[_k] = [_t[1], 1, _tb]
            else:
                _g[0] += _t[1]
                _g[1] += 1
        _merged = {}
        for _entry in _acc.values():
            _g = _merged.get(_entry[2])
            if _g is None:
                _merged[_entry[2]] = _entry
            else:
                _g[0] += _entry[0]
                _g[1] += _entry[1]
        _groups = [(_g[0], _g[1], list(reversed(_g[2]))) for _g in _merged.values()]
    except Exception:
        # Raw-layout drift on some future interpreter: fall back to the stdlib
        # pass — slower, but still correct and still off the paused evaluate.
        _groups = [
            (_s.size, _s.count, [(_f.filename, _f.lineno) for _f in _s.traceback])
            for _s in _snapshot.statistics('traceback')
        ]
    _groups.sort(key=lambda _g: _g[0], reverse=True)
    return _groups
"
}

/// The render + combiner halves of [`snapshot_payload_fn`] (see its docs).
fn render_payload_fns() -> &'static str {
    r"
def _basilisk_snapshot_payload_from(_state, _max_stats):
    import json, sysconfig, os
    _stdlib = sysconfig.get_paths().get('stdlib') or ''
    def _is_runtime_glue(_fn):
        # Anchored basename match (never a full-path substring, so a user file/dir
        # like debugpy_utils/app.py is NOT mistaken for the debugger), plus an
        # exact path-segment match for the debugger's own package files whose
        # basename isn't pydevd*/debugpy* (e.g. debugpy/server/api.py).
        _base = os.path.basename(_fn)
        _segs = _fn.replace(os.sep, '/').split('/')
        return (_base.startswith(('pydevd', 'debugpy', '_pydev'))
                or _base == 'tracemalloc.py'
                or 'debugpy' in _segs or 'pydevd' in _segs)
    def _is_stdlib_only(_fn):
        # Stdlib proper (not site-/dist-packages): the debugger's comm thread
        # bottoms out here, but a user's own stdlib use always has a user frame too.
        return bool(_stdlib) and _fn.startswith(_stdlib) and 'site-packages' not in _fn and 'dist-packages' not in _fn
    # No tracemalloc Filter pre-pass: a Filter matches the LEAF path with an
    # UNANCHORED fnmatch, which would silently drop a user allocation whose site
    # path merely contains 'debugpy'/'pydevd' (e.g. a dir like debugpy_utils/;
    # pyscript-1). Filter in the loop with the anchored helper instead, and keep
    # the top _max_stats SURVIVORS by size (iterate the size-sorted stats and
    # stop at the cap) so debugger noise can't crowd the user out of the top-N.
    top_stats = []
    for _size, _count, _all_frames in _basilisk_grouped_stats(_state['snapshot']):
        if len(top_stats) >= _max_stats:
            break
        # The allocation SITE (leaf) decides: an anchored debugger/runtime-glue or
        # synthetic leaf is the debugger's own allocation — drop the whole stat.
        _site = _all_frames[-1][0]
        if _is_runtime_glue(_site) or _site.startswith('<'):
            continue
        frames = []
        has_user = False
        for _fn, _lineno in _all_frames:
            if _is_runtime_glue(_fn) or _fn.startswith('<'):
                continue
            frames.append({'file': _fn, 'line': _lineno})
            if not _is_stdlib_only(_fn):
                has_user = True
        # Drop pure stdlib/runtime noise (no user or library frame survives).
        if not frames or not has_user:
            continue
        _leaf = frames[-1]
        top_stats.append({
            'file': _leaf['file'],
            'line': _leaf['line'],
            'size': _size,
            'count': _count,
            'traceback': frames,
        })
    return '__BASILISK_MEM__' + json.dumps({
        'current': _state['current'],
        'peak': _state['peak'],
        'stats': top_stats,
        'gcCounts': _state['gcCounts'],
        'gcObjects': _state['gcObjects'],
    })
def _basilisk_snapshot_payload(_max_stats):
    return _basilisk_snapshot_payload_from(_basilisk_capture_state(), _max_stats)
"
}

/// Python helper spawning the payload-rendering worker thread and printing the
/// reserved courier-file path.
///
/// Shared by [`take_snapshot`] and [`diff_snapshot`] (pyscript-6): the evaluate
/// hands the worker an already-frozen capture plus a zero-argument render
/// closure, reserves the courier path with `mkstemp` (an EMPTY file — the
/// editor treats empty as "worker still rendering" and polls), spawns the
/// worker, and prints the path immediately so the DAP `evaluate` returns well
/// inside pydevd's 3-second warning budget. Worker discipline:
/// - `pydev_do_not_trace` is pydevd's documented opt-out, so the worker keeps
///   running (and is never line-traced) even while every debuggee thread sits
///   suspended at the user's breakpoint; `sys.settrace(None)` inside the
///   worker sheds any trace function `threading.settrace` installed anyway.
/// - `daemon` is explicitly False: the spawning (pydevd evaluate) thread is a
///   daemon, and inheriting that would let interpreter shutdown kill a
///   half-written payload; non-daemon means `threading._shutdown` joins the
///   worker so the file always lands.
/// - The write is atomic (sibling `.part` + `os.replace`), so the editor's
///   poll sees either an empty reservation or the whole payload, never a
///   truncation. A render failure writes the error text instead — marker-less
///   on purpose, so ingest fails fast and honestly rather than timing out.
/// - The printed marker is split (`'__BASILISK_MEM_' + 'FILE__'`) so the full
///   marker never appears verbatim in the script SOURCE: pydevd quotes the
///   source in its diagnostics, and a quoted-source line containing the real
///   marker would poison the editor's output scan (the corrupted-courier
///   failure behind the original bug report).
fn spawn_render_worker_helper() -> &'static str {
    r"
def _basilisk_spawn_render(_render):
    import tempfile, os, sys, threading
    _fd, _path = tempfile.mkstemp(prefix='basilisk_mem_', suffix='.txt')
    os.close(_fd)
    def _basilisk_work():
        sys.settrace(None)
        try:
            _payload = _render()
        except Exception as _exc:
            _payload = 'basilisk render worker failed: ' + repr(_exc)
        _tmp = _path + '.part'
        with open(_tmp, 'w') as _f:
            _f.write(_payload)
        os.replace(_tmp, _path)
    _t = threading.Thread(target=_basilisk_work, name='basilisk-mem-render')
    _t.pydev_do_not_trace = True
    _t.daemon = False
    _t.start()
    print('__BASILISK_MEM_' + 'FILE__' + _path)
"
}

/// Script to take a memory snapshot and return allocation data as JSON.
///
/// Returns top allocations by line, current/peak memory, gc stats. The
/// evaluate-blocking portion is only the C-level capture (pyscript-6): the
/// per-trace render runs on a worker thread ([`spawn_render_worker_helper`])
/// that writes the `__BASILISK_MEM__` payload to the courier file the editor
/// polls ([PROFILE-MEMORY-COURIER]) — so a multi-million-trace heap can no
/// longer stall the paused `evaluate` past pydevd's 3-second warning budget.
#[must_use]
pub fn take_snapshot(max_stats: usize) -> String {
    let payload_fn = snapshot_payload_fn();
    let spawn = spawn_render_worker_helper();
    format!(
        r"
{payload_fn}{spawn}
_basilisk_state = _basilisk_capture_state()
# Early-bind the captured state as a default argument: a follow-up snapshot
# reassigns the module-level name, and a late-binding closure would make a
# still-running worker render the WRONG (newer) capture.
_basilisk_spawn_render(lambda _s=_basilisk_state: _basilisk_snapshot_payload_from(_s, {max_stats}))
"
    )
}

/// Script to compare two snapshots and find growing allocations.
///
/// Takes a fresh snapshot and compares against the previous one.
/// Returns growth data prefixed with `__BASILISK_MEM_DIFF__`.
///
/// Like [`take_snapshot`], the evaluate-blocking portion is only the C-level
/// capture (pyscript-6): `Snapshot.compare_to` runs the stdlib's pure-Python
/// statistics pass over BOTH snapshots (the slowest script of all — measured
/// ~18 s at 4 M traces), so the comparison is a fast per-leaf-site grouping of
/// each snapshot's raw traces on the render worker (stdlib `compare_to` stays
/// as the fallback), and the baseline swap happens at evaluate time so
/// back-to-back diffs stay correctly ordered.
#[must_use]
pub fn diff_snapshot(max_stats: usize) -> String {
    let spawn = spawn_render_worker_helper();
    format!(
        r"
import tracemalloc, json
{spawn}
def _basilisk_lineno_stats(_snapshot):
    # {{(file, line): [size, count]}} keyed by each trace's LEAF frame — the
    # stdlib statistics('lineno') grouping, one lean pass over the raw traces.
    try:
        _acc = {{}}
        _get = _acc.get
        for _t in _snapshot.traces._traces:
            _k = _t[2][0]
            _g = _get(_k)
            if _g is None:
                _acc[_k] = [_t[1], 1]
            else:
                _g[0] += _t[1]
                _g[1] += 1
        return _acc
    except Exception:
        return {{
            (_s.traceback[0].filename, _s.traceback[0].lineno): [_s.size, _s.count]
            for _s in _snapshot.statistics('lineno')
        }}
def _basilisk_render_diff(_prev, _snap, _current, _peak, _max_stats):
    if _prev is None:
        return '__BASILISK_MEM_DIFF__' + json.dumps({{'error': 'no previous snapshot'}})
    _old = _basilisk_lineno_stats(_prev)
    _new = _basilisk_lineno_stats(_snap)
    leaks = []
    for _k in set(_old) | set(_new):
        _new_size, _new_count = _new.get(_k, (0, 0))
        _old_size, _old_count = _old.get(_k, (0, 0))
        _size_diff = _new_size - _old_size
        if _size_diff != 0:
            leaks.append({{
                'file': _k[0],
                'line': _k[1],
                'sizeDiff': _size_diff,
                'countDiff': _new_count - _old_count,
                'size': _new_size,
                'count': _new_count,
                'traceback': [{{'file': _k[0], 'line': _k[1]}}]
            }})
    # Sort by ABS(size_diff) — compare_to's order — so the head slice carries
    # the biggest shrinks alongside the biggest growths. Both are reported:
    # dropping negative diffs made totalFreed/netGrowth lies and read a cache
    # that provably releases memory as a one-way leak.
    leaks.sort(key=lambda _l: abs(_l['sizeDiff']), reverse=True)
    del leaks[_max_stats:]
    return '__BASILISK_MEM_DIFF__' + json.dumps({{
        'leaks': leaks,
        'current': _current,
        'peak': _peak,
    }})
_basilisk_snap2 = tracemalloc.take_snapshot()
_basilisk_prev = getattr(tracemalloc, '_basilisk_prev_snapshot', None)
# Store this snapshot as the previous one for the next diff — at EVALUATE time,
# so a follow-up diff always compares against this capture even if this render
# worker is still running when the next request lands.
tracemalloc._basilisk_prev_snapshot = _basilisk_snap2
_basilisk_current, _basilisk_peak = tracemalloc.get_traced_memory()
# Early-bind everything as default arguments: a follow-up diff reassigns these
# module-level names, and a late-binding closure would make a still-running
# worker compare the WRONG (newer) snapshots.
_basilisk_spawn_render(lambda _p=_basilisk_prev, _s=_basilisk_snap2, _c=_basilisk_current, _k=_basilisk_peak:
    _basilisk_render_diff(_p, _s, _c, _k, {max_stats}))
"
    )
}

/// Script to store the current snapshot as the baseline for future diffs.
#[must_use]
pub fn store_baseline() -> &'static str {
    r"
import tracemalloc
tracemalloc._basilisk_prev_snapshot = tracemalloc.take_snapshot()
print('__BASILISK_MEM_OK__')
"
}

/// Python helper function that labels edges in the reference graph.
///
/// Must be included before the main `walk_references` script body.
fn ref_label_helper() -> &'static str {
    r"
def _basilisk_find_label(referrer, target):
    target_id = id(target)
    ref_type = type(referrer).__name__
    if ref_type == 'dict':
        for key, val in referrer.items():
            if id(val) == target_id:
                return '[' + repr(key)[:50] + ']'
        return 'dict-value'
    elif ref_type == 'list':
        for idx, val in enumerate(referrer):
            if id(val) == target_id:
                return '[' + str(idx) + ']'
        return 'list-item'
    elif ref_type == 'tuple':
        for idx, val in enumerate(referrer):
            if id(val) == target_id:
                return '(' + str(idx) + ')'
        return 'tuple-item'
    elif hasattr(referrer, '__dict__'):
        for attr, val in referrer.__dict__.items():
            if id(val) == target_id:
                return '.' + attr
    return ''
"
}

/// Script to walk the reference graph for objects of a given type.
///
/// Returns a JSON graph with nodes, edges, and detected cycles,
/// prefixed with `__BASILISK_MEM_REFS__`.
#[must_use]
pub fn walk_references(
    target_type: &str,
    target_repr_contains: Option<&str>,
    max_depth: u32,
    max_nodes: u32,
) -> String {
    let target_type = python_string_literal(target_type);
    let repr_filter = target_repr_contains.map_or_else(|| "None".to_owned(), python_string_literal);
    let label_helper = ref_label_helper();
    let emit = emit_via_file_helper();

    format!(
        r"
import gc, sys, json
{emit}{label_helper}
def _basilisk_walk_refs():
    gc.collect()
    target_type = {target_type}
    repr_filter = {repr_filter}
    max_depth = {max_depth}
    max_nodes = {max_nodes}

    targets = []
    for obj in gc.get_objects():
        if type(obj).__name__ == target_type:
            if repr_filter is None or repr_filter in repr(obj)[:200]:
                targets.append(obj)
                if len(targets) >= 10:
                    break

    if not targets:
        return {{'nodes': [], 'edges': [], 'cycles': [], 'retentionPath': []}}

    nodes = {{}}
    edges = []
    visited = set()
    queue = [(id(t), t, 0) for t in targets]

    while queue and len(nodes) < max_nodes:
        obj_id, obj, depth = queue.pop(0)
        if obj_id in visited:
            continue
        visited.add(obj_id)

        obj_type = type(obj).__name__
        obj_size = sys.getsizeof(obj)
        obj_repr = repr(obj)[:100]

        nodes[obj_id] = {{
            'id': obj_id,
            'type': obj_type,
            'size': obj_size,
            'repr': obj_repr,
            'depth': depth,
            'isTarget': obj in targets,
        }}

        if depth < max_depth:
            referrers = gc.get_referrers(obj)
            for ref_obj in referrers:
                ref_id = id(ref_obj)
                ref_type = type(ref_obj).__name__
                if ref_type in ('frame', 'module', 'code', 'function'):
                    continue

                label = _basilisk_find_label(ref_obj, obj)
                edges.append({{'from': ref_id, 'to': obj_id, 'label': label}})

                if ref_id not in visited:
                    queue.append((ref_id, ref_obj, depth + 1))

    return _basilisk_detect_cycles(nodes, edges)

def _basilisk_detect_cycles(nodes, edges):
    adj = {{}}
    for edge in edges:
        adj.setdefault(edge['from'], []).append(edge['to'])
    cycles = []
    path = []
    path_set = set()
    cycle_visited = set()
    def dfs(node):
        if node in path_set:
            idx = path.index(node)
            cycles.append(path[idx:] + [node])
            return
        if node in cycle_visited:
            return
        cycle_visited.add(node)
        path.append(node)
        path_set.add(node)
        for neighbor in adj.get(node, []):
            if neighbor in nodes:
                dfs(neighbor)
        path.pop()
        path_set.discard(node)
    for nid in nodes:
        if nid not in cycle_visited:
            dfs(nid)
    return {{'nodes': list(nodes.values()), 'edges': edges, 'cycles': cycles}}

result = _basilisk_walk_refs()
_basilisk_emit('__BASILISK_MEM_REFS__' + json.dumps(result))
"
    )
}

/// Script to list objects of a given type with sizes and refcounts.
///
/// Returns JSON prefixed with `__BASILISK_MEM_OBJECTS__`.
#[must_use]
pub fn objects_by_type(type_name: &str, limit: u32) -> String {
    let type_name = python_string_literal(type_name);
    let emit = emit_via_file_helper();
    format!(
        r"
import gc, sys, json
{emit}
gc.collect()
type_name = {type_name}
limit = {limit}
objects = []
type_summary = {{}}

for obj in gc.get_objects():
    tn = type(obj).__name__
    sz = sys.getsizeof(obj)
    type_summary.setdefault(tn, {{'count': 0, 'size': 0}})
    type_summary[tn]['count'] += 1
    type_summary[tn]['size'] += sz

    if tn == type_name and len(objects) < limit:
        objects.append({{
            'id': id(obj),
            'type': tn,
            'size': sz,
            'refcount': sys.getrefcount(obj) - 1,
            'repr': repr(obj)[:100],
        }})

objects.sort(key=lambda o: o['size'], reverse=True)
result = {{
    'objects': objects,
    'totalCount': type_summary.get(type_name, {{}}).get('count', 0),
    'totalSize': type_summary.get(type_name, {{}}).get('size', 0),
    'typeSummary': type_summary,
}}
_basilisk_emit('__BASILISK_MEM_OBJECTS__' + json.dumps(result))
"
    )
}

/// Encode client-provided text as a string literal accepted by Python.
///
/// JSON and Python share double-quoted string escaping, so this keeps quotes,
/// backslashes, and newlines inside the literal instead of executable source.
fn python_string_literal(value: &str) -> String {
    serde_json::Value::String(value.to_owned()).to_string()
}

/// Script to force garbage collection and report results.
///
/// Returns JSON prefixed with `__BASILISK_MEM_GC__`, handed back via a temp
/// file ([`emit_via_file_helper`]) so a large `gc.garbage` report is not
/// truncated.
#[must_use]
pub fn gc_collect() -> String {
    let emit = emit_via_file_helper();
    format!(
        r"
import gc, sys, json, tracemalloc
{emit}
before = tracemalloc.get_traced_memory()[0] if tracemalloc.is_tracing() else 0
collected = gc.collect()
after = tracemalloc.get_traced_memory()[0] if tracemalloc.is_tracing() else 0

uncollectable = []
garbage = sorted(
    gc.garbage,
    key=lambda obj: '__del__' not in type(obj).__dict__,
)
for obj in garbage[:20]:
    has_finalizer = '__del__' in type(obj).__dict__
    uncollectable.append({{
        'id': id(obj),
        'type': type(obj).__name__,
        'size': sys.getsizeof(obj),
        'repr': object.__repr__(obj)[:100],
        'reason': 'Instance has __del__ method and is in a reference cycle' if has_finalizer else 'Uncollectable cycle',
    }})

result = {{
    'collected': collected,
    'uncollectable': len(gc.garbage),
    'memoryFreed': max(0, before - after),
    'uncollectableObjects': uncollectable,
}}
_basilisk_emit('__BASILISK_MEM_GC__' + json.dumps(result))
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // [PROFILE-MEMORY-COMMANDS] Client-provided selectors must remain data in
    // the generated Python program, even when they contain Python syntax.
    #[test]
    fn client_strings_are_json_encoded_in_generated_scripts() {
        let hostile_type = "Widget';\n__import__('os').system('echo injected')\n#";
        let hostile_repr = "needle\\\"';\nraise RuntimeError('injected')\n#";
        let encoded_type = serde_json::Value::String(hostile_type.to_owned()).to_string();
        let encoded_repr = serde_json::Value::String(hostile_repr.to_owned()).to_string();

        let references = walk_references(hostile_type, Some(hostile_repr), 5, 200);
        assert!(
            references.contains(&format!("target_type = {encoded_type}")),
            "target type must be emitted as a JSON/Python string literal: {references}"
        );
        assert!(
            references.contains(&format!("repr_filter = {encoded_repr}")),
            "repr filter must be emitted as a JSON/Python string literal: {references}"
        );

        let objects = objects_by_type(hostile_type, 20);
        assert!(
            objects.contains(&format!("type_name = {encoded_type}")),
            "object type must be emitted as a JSON/Python string literal: {objects}"
        );
    }

    #[test]
    fn start_script_contains_tracemalloc() {
        let script = start_tracemalloc(25, "/tmp/basilisk-final.memfinal", 100);
        assert!(script.contains("tracemalloc.start(25)"));
        assert!(script.contains("gc.set_debug"));
    }

    #[test]
    fn exit_hook_does_not_collect_before_measuring() {
        // ux-2: debugpy tears down the user script's runpy namespace at program
        // end, so the program's end-state allocations are unreachable by at-exit
        // time and only DEBUG_SAVEALL keeps them tracked for the final snapshot.
        // A gc.collect()/gc.garbage.clear() in the hook would free exactly those
        // and empty the profile, so the hook must NOT collect before measuring.
        let script = start_tracemalloc(25, "/tmp/basilisk-final.memfinal", 100);
        assert!(
            !script.contains("gc.collect()") && !script.contains("gc.garbage.clear()"),
            "the at-exit hook must not collect/clear garbage before measuring (ux-2): {script}"
        );
    }

    #[test]
    fn start_script_captures_on_stop_signals() {
        // pyscript-2: the VS Code Stop button (SIGTERM) and Ctrl-C (SIGINT)
        // bypass atexit, so the run-to-completion flow would otherwise lose its
        // result whenever the user stops a long-running target. The start script
        // must install a handler for both that writes the final snapshot, and
        // re-raise the default disposition so the process still terminates.
        let script = start_tracemalloc(25, "/tmp/basilisk-final.memfinal", 100);
        assert!(
            script.contains("signal.SIGTERM"),
            "must handle the Stop button's SIGTERM: {script}"
        );
        assert!(
            script.contains("signal.SIGINT"),
            "must handle Ctrl-C's SIGINT: {script}"
        );
        assert!(
            script.contains("signal.signal(_signum, signal.SIG_DFL)")
                && script.contains("os.kill(os.getpid(), _signum)"),
            "must re-raise the signal so the process still dies as intended: {script}"
        );
        // Idempotent: the signal path and the atexit path must not double-measure.
        assert!(
            script.contains("_basilisk_exit_done"),
            "the exit write must be guarded: {script}"
        );
    }

    #[test]
    fn start_script_registers_an_atexit_final_snapshot() {
        // [PROFILE-MEMORY-FINAL] The "Run & Track Memory" flow has no breakpoint,
        // so the program runs to completion with no paused frame to snapshot from.
        // The start script must register an `atexit` hook that writes a final
        // snapshot to the given file as the program exits — that is what makes
        // the run end in a visible result instead of dead-ending.
        let script = start_tracemalloc(25, "/tmp/basilisk-final.memfinal", 100);
        assert!(script.contains("atexit"), "must import atexit: {script}");
        assert!(
            script.contains("atexit.register(_basilisk_write_exit_snapshot)"),
            "must register the exit-snapshot hook: {script}"
        );
        // The path is embedded as a JSON-encoded Python string literal (the
        // cross-platform-safe pattern from cooperative.rs), not a hand-rolled
        // raw string — so a backslash or quote in the path can't break it.
        assert!(
            script.contains("_final = \"/tmp/basilisk-final.memfinal\"")
                && script.contains("os.replace(_tmp, _final)"),
            "the hook must atomically write to the JSON-encoded snapshot file path: {script}"
        );
        // The at-exit payload reuses the shared snapshot builder, so its output is
        // byte-identical to an evaluate-path snapshot the same parser ingests.
        assert!(
            script.contains("_basilisk_snapshot_payload(100)"),
            "the hook must build the shared snapshot payload: {script}"
        );
        assert!(
            script.contains("__BASILISK_MEM__"),
            "payload must carry the snapshot marker: {script}"
        );
    }

    #[test]
    fn start_script_json_encodes_windows_style_paths() {
        // A Windows temp path with backslashes (or a stray quote in TMPDIR) must
        // be embedded as a valid, escaped Python string literal — never a raw
        // `r'...'` that a trailing backslash could break ([PROFILE-MEMORY-FINAL]).
        let script = start_tracemalloc(
            25,
            r"C:\Users\me\AppData\Local\Temp\basilisk-x.memfinal",
            100,
        );
        assert!(
            script
                .contains(r#"_final = "C:\\Users\\me\\AppData\\Local\\Temp\\basilisk-x.memfinal""#),
            "a Windows path must be JSON-escaped into the snapshot-file literal: {script}"
        );
    }

    #[test]
    fn snapshot_script_contains_marker() {
        let script = take_snapshot(500);
        assert!(script.contains("__BASILISK_MEM__"));
        // The snapshot payload rides the shared builder; the bound is passed
        // through and applied to the SURVIVORS (top-N after filtering), not a raw
        // pre-slice — so debugger noise can't crowd the user out (pyscript-1).
        assert!(script.contains("len(top_stats) >= _max_stats"));
        // Early-bound default arg (`_s=_basilisk_state`): a late-binding closure
        // would let a follow-up snapshot's reassignment leak into a still-running
        // worker's render.
        assert!(
            script.contains("lambda _s=_basilisk_state: _basilisk_snapshot_payload_from(_s, 500)")
        );
    }

    #[test]
    fn evaluate_scripts_render_off_the_paused_evaluate() {
        // pyscript-6 — the "did not finish after 3.00 seconds" bug: the paused
        // DAP evaluate may only run the C-fast capture; the per-trace pure-Python
        // render (seconds on multi-million-trace heaps, past pydevd's 3 s
        // PYDEVD_WARN_EVALUATION_TIMEOUT) must run on a worker thread that
        // writes the courier file. Lock in the whole worker discipline for both
        // evaluate-path scripts (snapshot and diff).
        for script in [take_snapshot(100), diff_snapshot(100)] {
            assert!(
                script.contains("_basilisk_spawn_render("),
                "the render must be handed to the worker spawner: {script}"
            );
            // pydevd's documented opt-out: the worker keeps running (untraced)
            // even while every debuggee thread is suspended at a breakpoint.
            assert!(
                script.contains("_t.pydev_do_not_trace = True"),
                "the worker must opt out of pydevd tracing/suspension: {script}"
            );
            // The spawning pydevd evaluate thread is a daemon; inheriting that
            // would let interpreter shutdown kill a half-written payload.
            assert!(
                script.contains("_t.daemon = False"),
                "the worker must be non-daemon so shutdown joins it: {script}"
            );
            // Shed any trace fn threading.settrace installed despite the opt-out.
            assert!(
                script.contains("sys.settrace(None)"),
                "the worker must shed debugger line-tracing: {script}"
            );
            // Atomic hand-off: the editor's poll must never see a truncation.
            assert!(
                script.contains("os.replace(_tmp, _path)"),
                "the payload write must be atomic (sibling .part + os.replace): {script}"
            );
        }
        // The data is FROZEN at evaluate time — the snapshot captures state
        // before spawning, and the diff swaps the baseline at evaluate time so
        // back-to-back diffs stay ordered even with a render still in flight.
        assert!(
            take_snapshot(100).contains("_basilisk_state = _basilisk_capture_state()"),
            "the snapshot must capture at evaluate time, not in the worker"
        );
        assert!(
            diff_snapshot(100).contains("tracemalloc._basilisk_prev_snapshot = _basilisk_snap2"),
            "the diff must swap the baseline at evaluate time, not in the worker"
        );
    }

    #[test]
    fn snapshot_keeps_full_call_stacks_and_filters_the_debugger() {
        // [PROFILE-MEMORY-FINAL] The .heapprofile is only worth reading if it's
        // the user's program (not pydevd/debugpy/tracemalloc) and carries real
        // call stacks (not a flat by-line list). Lock both in.
        let script = take_snapshot(100);
        assert!(
            script.contains("statistics('traceback')"),
            "must keep full tracebacks for a real call tree, not statistics('lineno'): {script}"
        );
        // pyscript-1/pyscript-5: there must be NO tracemalloc Filter pre-pass —
        // its fnmatch is unanchored over the leaf path, so a user allocation in a
        // dir like `debugpy_utils/` would be silently dropped before the anchored
        // per-frame logic ever runs. All debugger filtering is the anchored helper.
        assert!(
            !script.contains("filter_traces") && !script.contains("tracemalloc.Filter"),
            "must NOT use an unanchored filter_traces pre-pass: {script}"
        );
        assert!(
            !script.contains("'*pydevd*'") && !script.contains("'*debugpy*'"),
            "must NOT bake in unanchored debugger globs: {script}"
        );
        // The allocation site is the LEAF frame (frames are root->leaf), not [0].
        assert!(
            script.contains("_leaf = frames[-1]"),
            "the reported site must be the leaf (allocation) frame: {script}"
        );
        // Debugger detection must match the basename ANCHORED (and exact path
        // segments), never an unanchored full-path substring — else a user file
        // in a dir like `debugpy_utils/` would be silently deleted.
        assert!(
            script.contains("startswith(('pydevd', 'debugpy', '_pydev'))"),
            "debugger detection must use anchored basename matching: {script}"
        );
        assert!(
            !script.contains("'pydevd' in _fn"),
            "debugger detection must NOT use an unanchored full-path substring match: {script}"
        );
        // The leaf (allocation site) decides whether a stat is the debugger's own
        // (grouped frames are root->leaf, so the site is the LAST frame's file).
        assert!(
            script.contains("_site = _all_frames[-1][0]"),
            "the allocation site (leaf) must gate the debugger drop: {script}"
        );
        // Pure debugger/runtime stacks (no user or library frame) are dropped.
        assert!(
            script.contains("not has_user"),
            "must drop allocations with no surviving user/library frame: {script}"
        );
    }

    #[test]
    fn payload_scripts_hand_off_via_a_file_not_a_raw_print() {
        // debugpy truncates a single `print()` (~20KB), so large tracemalloc
        // payloads must be written to a temp file and only the PATH printed
        // ([PROFILE-MEMORY-COURIER]). Every JSON-emitting script must route
        // through a file hand-off (the sync emitter or the render worker) and
        // must NOT print the JSON directly.
        for script in [
            take_snapshot(100),
            diff_snapshot(100),
            walk_references("Cycle", None, 5, 200),
            objects_by_type("dict", 50),
            gc_collect(),
        ] {
            // The marker is printed SPLIT ('__BASILISK_MEM_' + 'FILE__') so the
            // assembled marker never appears verbatim in the script source:
            // pydevd quotes source in its diagnostics, and a quoted-source line
            // carrying the real marker would poison the editor's output scan
            // (pyscript-6 — the corrupted-courier half of the stall bug).
            assert!(
                script.contains("print('__BASILISK_MEM_' + 'FILE__' + _path)"),
                "script must print the (split) file-handoff marker: {script}"
            );
            assert!(
                !script.contains("__BASILISK_MEM_FILE__"),
                "the assembled file marker must never appear in script source: {script}"
            );
            assert!(
                script.contains("_basilisk_emit(") || script.contains("_basilisk_spawn_render("),
                "script must route its payload through a file hand-off: {script}"
            );
            // The only direct print is the short file-path line; no JSON marker
            // may be printed (that is what truncates).
            for json_marker in [
                "__BASILISK_MEM__",
                "__BASILISK_MEM_DIFF__",
                "__BASILISK_MEM_REFS__",
                "__BASILISK_MEM_OBJECTS__",
                "__BASILISK_MEM_GC__",
            ] {
                assert!(
                    !script.contains(&format!("print('{json_marker}'")),
                    "JSON marker {json_marker} must not be printed directly (it would truncate): {script}"
                );
            }
        }
    }

    #[test]
    fn diff_script_contains_marker() {
        let script = diff_snapshot(100);
        assert!(script.contains("__BASILISK_MEM_DIFF__"));
        assert!(script.contains("_basilisk_prev_snapshot"));
    }

    #[test]
    fn walk_refs_script_with_filter() {
        let script = walk_references("DataFrame", Some("huge"), 5, 200);
        assert!(script.contains("DataFrame"));
        assert!(script.contains("repr_filter = \"huge\""));
        assert!(script.contains("__BASILISK_MEM_REFS__"));
    }

    #[test]
    fn walk_refs_script_without_filter() {
        let script = walk_references("list", None, 3, 100);
        assert!(script.contains("repr_filter = None"));
    }

    #[test]
    fn objects_by_type_script() {
        let script = objects_by_type("DataFrame", 50);
        assert!(script.contains("__BASILISK_MEM_OBJECTS__"));
        assert!(script.contains("DataFrame"));
    }

    #[test]
    fn gc_collect_script() {
        let script = gc_collect();
        assert!(script.contains("__BASILISK_MEM_GC__"));
        assert!(script.contains("gc.collect()"));
        assert!(
            script.contains("'__del__' not in type(obj).__dict__"),
            "finalizer cycles must be selected ahead of incidental garbage"
        );
        assert!(
            script.contains("object.__repr__(obj)"),
            "reporting garbage must not execute an object's custom __repr__"
        );
        assert!(
            !script.contains("'repr': repr(obj)"),
            "a hostile __repr__ must remain inert during collection"
        );
    }
}
