//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Python injection scripts for memory profiling via DAP evaluate.
//!
//! These scripts are injected into a running Python process through the
//! debug adapter's `evaluate` request. They use `tracemalloc` (stdlib)
//! for allocation tracking and `gc` for reference graph introspection.

/// Script to start tracemalloc with deep tracebacks.
///
/// Injected once when memory profiling begins. The `nframe` parameter
/// controls traceback depth (default 25 frames).
#[must_use]
pub fn start_tracemalloc(nframe: u32) -> String {
    format!(
        r"
import tracemalloc, gc
tracemalloc.start({nframe})
gc.set_debug(gc.DEBUG_SAVEALL)
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

/// Script to take a memory snapshot and return allocation data as JSON.
///
/// Returns top allocations by line, current/peak memory, gc stats.
/// The payload is the `__BASILISK_MEM__` marker handed back via a temp file
/// ([`emit_via_file_helper`]) so a large snapshot is never truncated.
#[must_use]
pub fn take_snapshot(max_stats: usize) -> String {
    let emit = emit_via_file_helper();
    format!(
        r"
import tracemalloc, json, gc
{emit}
snapshot = tracemalloc.take_snapshot()
stats = snapshot.statistics('lineno')
top_stats = []
for stat in stats[:{max_stats}]:
    frame = stat.traceback[0]
    top_stats.append({{
        'file': frame.filename,
        'line': frame.lineno,
        'size': stat.size,
        'count': stat.count,
        'traceback': [{{'file': f.filename, 'line': f.lineno}} for f in stat.traceback]
    }})

current, peak = tracemalloc.get_traced_memory()
mem_info = {{
    'current': current,
    'peak': peak,
    'stats': top_stats,
    'gcCounts': list(gc.get_count()),
    'gcObjects': len(gc.get_objects()),
}}
_basilisk_emit('__BASILISK_MEM__' + json.dumps(mem_info))
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
        let script = start_tracemalloc(25);
        assert!(script.contains("tracemalloc.start(25)"));
        assert!(script.contains("gc.set_debug"));
    }

    #[test]
    fn snapshot_script_contains_marker() {
        let script = take_snapshot(500);
        assert!(script.contains("__BASILISK_MEM__"));
        assert!(script.contains("stats[:500]"));
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
