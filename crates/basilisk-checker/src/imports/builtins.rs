//! Implements [ANALYSIS-INCR-IMPORTS]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INCR-IMPORTS
//! The shared `builtins.pyi` class index: one parse (or precomputed decode,
//! [STUBRES-TYPESHED-BUILTINS-INDEX]) per `(snapshot, target)`, shared by
//! `Arc` across every resolved module.

use super::apply::snapshot_stub_source;
use super::ImportSearchPaths;

/// Index `builtins.pyi` from the exact root-owned active generation.
///
/// This deliberately has no compiled-table fallback: production CLI/LSP paths
/// activate a Snapshot before analysis, and a custom source is canonical. A
/// missing/malformed body therefore leaves the index empty instead of mixing a
/// second step-3 generation into editor or checker results.
/// Parsed `builtins.pyi` class index, shared by `Arc` across every module.
type BuiltinsMap = std::collections::HashMap<String, basilisk_resolver::scope::IndexedStubClass>;

/// Deterministic-per-`(snapshot, target)` cache of [`BuiltinsMap`].
///
/// The parse is pure for a given snapshot identity + target, so it is built
/// ONCE and shared by `Arc`. Previously every resolved module reparsed and
/// OWNED a full copy of this index; on a large project (e.g. `FastAPI`, ~28k
/// symbols across thousands of modules) that duplicated the entire builtins
/// index thousands of times (~1 GB LSP RSS). Sharing by `Arc` means cloning a
/// `ResolvedModule` (e.g. [`crate::incremental::cross_resolved_module`]) only
/// bumps the refcount instead of deep-cloning the map.
fn builtins_memo(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<BuiltinsMap>>> {
    static MEMO: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<BuiltinsMap>>>,
    > = std::sync::OnceLock::new();
    MEMO.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn cached_builtins(key: &str) -> Option<std::sync::Arc<BuiltinsMap>> {
    builtins_memo().lock().ok()?.get(key).cloned()
}

fn remember_builtins(key: &str, map: &std::sync::Arc<BuiltinsMap>) {
    if let Ok(mut memo) = builtins_memo().lock() {
        let _ = memo.insert(key.to_owned(), std::sync::Arc::clone(map));
    }
}

pub(super) fn populate_builtin_classes(
    resolved: &mut basilisk_resolver::ResolvedModule,
    search_paths: &ImportSearchPaths,
    importing_file: &std::path::Path,
) {
    let Some(active) = search_paths.typeshed_snapshot.as_ref() else {
        resolved.builtin_classes = std::sync::Arc::new(BuiltinsMap::new());
        return;
    };
    let Some((snapshot, target)) = active.for_importer(Some(importing_file)) else {
        resolved.builtin_classes = std::sync::Arc::new(BuiltinsMap::new());
        return;
    };
    resolved.builtin_classes = shared_builtins_index(snapshot, target);
}

/// Build (and memoize) the `builtins.pyi` class index for `(snapshot,
/// target)` — the per-process cost of indexing `builtins.pyi` is paid once.
/// The CLI calls this from a background thread right after activation so the
/// parse overlaps its pipeline lead-in ([ANALYSIS-INCR-IMPORTS]).
pub fn prewarm_builtin_classes(
    snapshot: &std::sync::Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
    target: Option<&basilisk_stubs::types::StubTarget>,
) {
    let _ = shared_builtins_index(snapshot, target);
}

fn shared_builtins_index(
    snapshot: &std::sync::Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
    target: Option<&basilisk_stubs::types::StubTarget>,
) -> std::sync::Arc<BuiltinsMap> {
    let cache_key = format!(
        "{}|{}",
        snapshot.identity.uri_component(),
        target.map_or_else(String::new, |target| format!("{:?}", target.python_version))
    );
    if let Some(cached) = cached_builtins(&cache_key) {
        return cached;
    }
    let build_index = || -> BuiltinsMap {
        let mut map = BuiltinsMap::new();
        let located = target.map_or_else(
            || snapshot.read_stub("builtins"),
            |target| snapshot.read_stub_for_target("builtins", target.python_version),
        );
        let Some((logical_uri, source_text)) = located else {
            return map;
        };
        let stub_source = snapshot_stub_source(snapshot);
        let Some(classes) =
            builtins_class_map(snapshot, target, &logical_uri, source_text, stub_source)
        else {
            return map;
        };
        let source_path = std::path::PathBuf::from(logical_uri);
        let source_identity = snapshot.identity.uri_component();
        let source_text: std::sync::Arc<str> = std::sync::Arc::from(source_text);
        let provenance =
            basilisk_stubs::TypeProvenance::from((&stub_source, &basilisk_stubs::StubTier::Tier1));
        map.extend(classes.into_iter().map(|(name, declaration)| {
            (
                name,
                basilisk_resolver::scope::IndexedStubClass {
                    declaration,
                    source_path: source_path.clone(),
                    source_identity: source_identity.clone(),
                    source_text: std::sync::Arc::clone(&source_text),
                    provenance,
                },
            )
        }));
        map
    };
    let shared = std::sync::Arc::new(build_index());
    remember_builtins(&cache_key, &shared);
    shared
}

/// The `builtins` class map for `(snapshot, target)`: the precomputed bundled
/// index when it applies ([STUBRES-TYPESHED-BUILTINS-INDEX] — bundled
/// snapshot, no version target), else a live parse of the located stub body.
/// The drift gate in `basilisk-stubs` pins the two paths equal.
fn builtins_class_map(
    snapshot: &basilisk_stubs::typeshed::snapshot::Snapshot,
    target: Option<&basilisk_stubs::types::StubTarget>,
    logical_uri: &str,
    source_text: &str,
    stub_source: basilisk_stubs::StubSource,
) -> Option<std::collections::HashMap<String, basilisk_stubs::StubClass>> {
    let bundled = matches!(
        snapshot.identity,
        basilisk_stubs::typeshed::source::SourceIdentity::Bundled { .. }
    );
    if target.is_none() && bundled {
        if let Some(classes) = basilisk_stubs::typeshed::builtins_index::bundled_builtins_classes()
        {
            return Some(classes);
        }
    }
    let parsed = match target {
        Some(target) => basilisk_stubs::pyi_parser::parse_pyi_source_for_target(
            source_text,
            std::path::Path::new(logical_uri),
            "builtins",
            stub_source,
            basilisk_stubs::StubTier::Tier1,
            target,
        ),
        None => basilisk_stubs::parse_pyi_source(
            source_text,
            std::path::Path::new(logical_uri),
            "builtins",
            stub_source,
            basilisk_stubs::StubTier::Tier1,
        ),
    };
    parsed.ok().map(|module| module.classes)
}
