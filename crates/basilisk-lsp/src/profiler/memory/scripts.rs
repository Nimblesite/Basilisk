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
def _basilisk_signal_exit(_signum, _frame):
    # The VS Code Stop button terminates the debuggee with SIGTERM (SIGINT for
    # Ctrl-C), neither of which runs atexit (pyscript-2). Capture the final
    # snapshot, then restore the default disposition and re-raise so the process
    # still dies as the signal intended. SIGKILL / os._exit stay unrecoverable.
    _basilisk_write_exit_snapshot()
    signal.signal(_signum, signal.SIG_DFL)
    os.kill(os.getpid(), _signum)
tracemalloc.start({nframe})
gc.set_debug(gc.DEBUG_SAVEALL)
atexit.register(_basilisk_write_exit_snapshot)
for _sig in (signal.SIGTERM, signal.SIGINT):
    try:
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
fn emit_via_file_helper() -> &'static str {
    r"
def _basilisk_emit(_payload):
    import tempfile, os
    _fd, _path = tempfile.mkstemp(prefix='basilisk_mem_', suffix='.txt')
    with os.fdopen(_fd, 'w') as _f:
        _f.write(_payload)
    print('__BASILISK_MEM_FILE__' + _path)
"
}

/// Python `def _basilisk_snapshot_payload(_max_stats)` returning the
/// `__BASILISK_MEM__ + json` snapshot payload string from the current
/// `tracemalloc` state (top allocations with full call stacks, current/peak
/// memory, gc stats).
///
/// Single source of truth for the snapshot payload, embedded by both the
/// evaluate-path snapshot ([`take_snapshot`]) and the at-exit final snapshot
/// ([`start_tracemalloc`]'s `atexit` hook), so both emit a byte-identical
/// payload the same `basilisk.memory.ingest` parser dispatches. Defined as a
/// plain `&'static str` (no `format!`) so the dict-literal braces stay literal.
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
/// 2. `statistics('traceback')` keeps each allocation's full call stack (the
///    `take_snapshot(25)` depth), root→leaf, with the debugger/runtime glue
///    stripped — so the editor builds a real call tree, not a flat list.
fn snapshot_payload_fn() -> &'static str {
    r"
def _basilisk_snapshot_payload(_max_stats):
    import tracemalloc, json, gc, sysconfig, os
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
    stats = tracemalloc.take_snapshot().statistics('traceback')
    top_stats = []
    for stat in stats:
        if len(top_stats) >= _max_stats:
            break
        # The allocation SITE (leaf) decides: an anchored debugger/runtime-glue or
        # synthetic leaf is the debugger's own allocation — drop the whole stat.
        _site = stat.traceback[-1].filename
        if _is_runtime_glue(_site) or _site.startswith('<'):
            continue
        frames = []
        has_user = False
        for _f in stat.traceback:
            _fn = _f.filename
            if _is_runtime_glue(_fn) or _fn.startswith('<'):
                continue
            frames.append({'file': _fn, 'line': _f.lineno})
            if not _is_stdlib_only(_fn):
                has_user = True
        # Drop pure stdlib/runtime noise (no user or library frame survives).
        if not frames or not has_user:
            continue
        _leaf = frames[-1]
        top_stats.append({
            'file': _leaf['file'],
            'line': _leaf['line'],
            'size': stat.size,
            'count': stat.count,
            'traceback': frames,
        })
    current, peak = tracemalloc.get_traced_memory()
    return '__BASILISK_MEM__' + json.dumps({
        'current': current,
        'peak': peak,
        'stats': top_stats,
        'gcCounts': list(gc.get_count()),
        'gcObjects': len(gc.get_objects()),
    })
"
}

/// Script to take a memory snapshot and return allocation data as JSON.
///
/// Returns top allocations by line, current/peak memory, gc stats.
/// The payload is the `__BASILISK_MEM__` marker handed back via a temp file
/// ([`emit_via_file_helper`]) so a large snapshot is never truncated.
#[must_use]
pub fn take_snapshot(max_stats: usize) -> String {
    let emit = emit_via_file_helper();
    let payload_fn = snapshot_payload_fn();
    format!(
        r"
{emit}{payload_fn}
_basilisk_emit(_basilisk_snapshot_payload({max_stats}))
"
    )
}

/// Script to compare two snapshots and find growing allocations.
///
/// Takes a fresh snapshot and compares against the previous one.
/// Returns growth data prefixed with `__BASILISK_MEM_DIFF__`.
#[must_use]
pub fn diff_snapshot(max_stats: usize) -> String {
    let emit = emit_via_file_helper();
    format!(
        r"
import tracemalloc, json
{emit}
snapshot2 = tracemalloc.take_snapshot()
# Compare against the stored previous snapshot
if hasattr(tracemalloc, '_basilisk_prev_snapshot'):
    diff = snapshot2.compare_to(tracemalloc._basilisk_prev_snapshot, 'lineno')
    leaks = []
    for stat in diff[:{max_stats}]:
        if stat.size_diff > 0:
            frame = stat.traceback[0]
            leaks.append({{
                'file': frame.filename,
                'line': frame.lineno,
                'sizeDiff': stat.size_diff,
                'countDiff': stat.count_diff,
                'size': stat.size,
                'count': stat.count,
                'traceback': [{{'file': f.filename, 'line': f.lineno}} for f in stat.traceback]
            }})
    current, peak = tracemalloc.get_traced_memory()
    result = {{
        'leaks': leaks,
        'current': current,
        'peak': peak,
    }}
    _basilisk_emit('__BASILISK_MEM_DIFF__' + json.dumps(result))
else:
    _basilisk_emit('__BASILISK_MEM_DIFF__' + json.dumps({{'error': 'no previous snapshot'}}))

# Store this snapshot as the previous one for the next diff
tracemalloc._basilisk_prev_snapshot = snapshot2
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
    let repr_filter = target_repr_contains.map_or_else(|| "None".to_owned(), |r| format!("'{r}'"));
    let label_helper = ref_label_helper();
    let emit = emit_via_file_helper();

    format!(
        r"
import gc, sys, json
{emit}{label_helper}
def _basilisk_walk_refs():
    gc.collect()
    target_type = '{target_type}'
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
    let emit = emit_via_file_helper();
    format!(
        r"
import gc, sys, json
{emit}
gc.collect()
type_name = '{type_name}'
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
for obj in gc.garbage[:20]:
    uncollectable.append({{
        'id': id(obj),
        'type': type(obj).__name__,
        'size': sys.getsizeof(obj),
        'repr': repr(obj)[:100],
        'reason': 'Instance has __del__ method and is in a reference cycle' if hasattr(obj, '__del__') else 'Uncollectable cycle',
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
        assert!(script.contains("_basilisk_snapshot_payload(500)"));
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
        // The leaf (allocation site) decides whether a stat is the debugger's own.
        assert!(
            script.contains("_site = stat.traceback[-1].filename"),
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
        // through the file emitter and must NOT print the JSON directly.
        for script in [
            take_snapshot(100),
            diff_snapshot(100),
            walk_references("Cycle", None, 5, 200),
            objects_by_type("dict", 50),
            gc_collect(),
        ] {
            assert!(
                script.contains("__BASILISK_MEM_FILE__"),
                "script must emit the file-handoff marker: {script}"
            );
            assert!(
                script.contains("_basilisk_emit("),
                "script must route its payload through the file emitter: {script}"
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
        assert!(script.contains("'huge'"));
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
    }
}
