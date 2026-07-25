//! Implements [ANALYSIS-INCR-IMPORTS]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INCR-IMPORTS
//! Applies path resolution to every import in a module, in place.

use std::path::PathBuf;

use basilisk_resolver::scope::{ImportKind, ImportedModuleApi, PackageDepKind};

use super::fs_cache::FsCache;
use super::resolve::{classify_unresolved, resolve_module_with_importer_cached};
use super::ImportSearchPaths;

/// Resolve every import in a single module against the search paths, in place.
///
/// Sets each `ImportInfo`'s `resolution`, `resolved_path`, `unresolved_reason`,
/// and uv package metadata. Shared by the CLI's batch pipeline and the salsa
/// `resolved_module` query so both agree on what resolves — preventing false
/// `imports_unresolved` in the
/// editor for third-party imports that the CLI resolves.
/// Implements [ANALYSIS-INCR-IMPORTS].
pub fn resolve_module_imports(
    resolved: &mut basilisk_resolver::ResolvedModule,
    search_paths: &ImportSearchPaths,
) {
    // The file's own path, used to search its directory for sibling modules.
    let importing_file = PathBuf::from(&resolved.path);
    populate_builtin_classes(resolved, search_paths, &importing_file);

    // One directory-listing cache for the whole loop: every import of this
    // file probes the same search directories, so read each dir once instead
    // of `stat`-ing every candidate path per import.
    let fs = FsCache::new();

    // Member APIs captured during the loop, inserted after it ends — we cannot
    // borrow `resolved.imported_modules` while iterating `resolved.imports`.
    let mut captured: Vec<(String, ImportedModuleApi)> = Vec::new();

    for import in &mut resolved.imports {
        let result = match import.kind {
            ImportKind::Plain | ImportKind::From | ImportKind::Star => {
                resolve_module_with_importer_cached(
                    &import.module,
                    search_paths,
                    Some(&importing_file),
                    &fs,
                )
            }
        };
        if let Some(r) = result {
            import.resolution = r.resolution;
            import.resolved_path = Some(r.path);
        } else {
            // Classify why the import is unresolved for actionable diagnostics.
            import.unresolved_reason = Some(classify_unresolved(&import.module, search_paths));
        }

        // Capture the member API of plain `import X` statements backed by a
        // user/local stub, so `imports_module_attribute` can flag `X.undeclared_attr`. Only
        // single-segment plain imports resolved to a `.pyi` under a configured
        // `stub-paths` dir (Phase 1: the stubs the developer owns).
        if let Some((binding, api)) = capture_user_stub_api(import, search_paths) {
            captured.push((binding, api));
        } else if let Some((binding, api)) =
            capture_active_typeshed_api(import, search_paths, &importing_file)
        {
            // GitHub #330: the other half of the same rule. Without this the
            // step-3 Typeshed member set only ever reached cross-module mode,
            // so `basilisk check` accepted `json.parse_body` in silence.
            captured.push((binding, api));
        }

        import.stub_distribution =
            stub_distribution(&import.module, search_paths, Some(importing_file.as_path()));

        // Annotate with package metadata from the uv registry.
        enrich_package_metadata(import, search_paths);
    }

    for (binding, api) in captured {
        let _ = resolved.imported_modules.insert(binding, api);
    }
}

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

/// The provenance to record for stubs read out of `snapshot`.
///
/// A custom root is its own source, never Typeshed — provenance drives honest
/// hover text, so the two must not be conflated.
fn snapshot_stub_source(
    snapshot: &basilisk_stubs::typeshed::snapshot::Snapshot,
) -> basilisk_stubs::StubSource {
    if matches!(
        snapshot.identity,
        basilisk_stubs::typeshed::source::SourceIdentity::Custom { .. }
    ) {
        basilisk_stubs::StubSource::CustomTypeshed
    } else {
        basilisk_stubs::StubSource::Typeshed
    }
}

fn populate_builtin_classes(
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
    let cache_key = format!(
        "{}|{}",
        snapshot.identity.uri_component(),
        target.map_or_else(String::new, |target| format!("{:?}", target.python_version))
    );
    if let Some(cached) = cached_builtins(&cache_key) {
        resolved.builtin_classes = cached;
        return;
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
        let parsed = match target {
            Some(target) => basilisk_stubs::pyi_parser::parse_pyi_source_for_target(
                source_text,
                std::path::Path::new(&logical_uri),
                "builtins",
                stub_source,
                basilisk_stubs::StubTier::Tier1,
                target,
            ),
            None => basilisk_stubs::parse_pyi_source(
                source_text,
                std::path::Path::new(&logical_uri),
                "builtins",
                stub_source,
                basilisk_stubs::StubTier::Tier1,
            ),
        };
        let Ok(module) = parsed else {
            return map;
        };
        let source_path = std::path::PathBuf::from(logical_uri);
        let source_identity = snapshot.identity.uri_component();
        let source_text: std::sync::Arc<str> = std::sync::Arc::from(source_text);
        let provenance =
            basilisk_stubs::TypeProvenance::from((&stub_source, &basilisk_stubs::StubTier::Tier1));
        map.extend(module.classes.into_iter().map(|(name, declaration)| {
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
    resolved.builtin_classes = shared;
}

/// Build the [`ImportedModuleApi`] for a plain `import X` backed by a user stub,
/// or `None` if this import is out of scope (aliased/dotted/from-import, not a
/// user stub, or the stub fails to parse).
fn capture_user_stub_api(
    import: &basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
) -> Option<(String, ImportedModuleApi)> {
    let stub_path = user_stub_path(import, search_paths)?;
    let source_text = std::fs::read_to_string(stub_path).ok()?;
    let tier = basilisk_stubs::user_stub_tier(&source_text);
    let stub = basilisk_stubs::parse_pyi_source(
        &source_text,
        stub_path,
        &import.module,
        basilisk_stubs::StubSource::UserStub,
        tier,
    )
    .ok()?;
    Some((
        import.module.clone(),
        build_stub_api(&stub, stub_path, search_paths),
    ))
}

/// Capture the member API of a plain `import X` backed by the **active step-3
/// Typeshed source** (GitHub #330).
///
/// The mirror image of [`capture_user_stub_api`] for the other authoritative
/// stub source. `populate_imported_symbols` already does this for cross-module
/// mode; doing it here too is what gives the CLI and the non-cross-module LSP
/// modes the same `imports_module_attribute` coverage of the standard library.
///
/// Exports come from [`crate::exports::load_snapshot_stub_module`] — the same
/// producer the cross-module path uses — so the captured member set follows the
/// stub's re-export graph. A shallower capture would report every valid
/// re-export as a missing attribute (GitHub #312).
fn capture_active_typeshed_api(
    import: &basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
    importing_file: &std::path::Path,
) -> Option<(String, ImportedModuleApi)> {
    // An unaliased dotted `import a.b.c` binds only the root `a`, whose members
    // are not the resolved `a.b.c` stub's members — capturing them under `a`
    // reds correct code (GitHub #324). The shared helper yields no binding for
    // exactly those imports, mirroring the user-stub path.
    let binding = crate::exports::authoritative_module_binding(import)?;
    let resolved_path = import.resolved_path.as_ref()?;
    let active = search_paths.typeshed_snapshot.as_ref()?;
    let (_, importer_target) = active.for_importer(Some(importing_file))?;
    let (snapshot, target, source_text) =
        active.source_for_uri(&resolved_path.to_string_lossy(), importer_target)?;

    let request = crate::exports::ExternalModuleRequest::Stub {
        module_name: import.module.clone(),
        source: snapshot_stub_source(snapshot),
    };
    let external = crate::exports::load_snapshot_stub_module(
        resolved_path,
        source_text,
        &request,
        snapshot,
        target,
    );
    if external.exports.is_empty() {
        return None;
    }

    Some((
        binding,
        ImportedModuleApi {
            has_getattr: external
                .exports
                .iter()
                .any(|(name, _)| name == "__getattr__"),
            member_names: external
                .exports
                .iter()
                .map(|(name, _)| name.clone())
                .collect(),
            stub_path: resolved_path.clone(),
        },
    ))
}

/// Re-capture a user-stub import's API from stub **source text** rather than
/// disk.
///
/// The incremental engine calls this so an edited-but-unsaved `.pyi` (whose
/// live content lives in a salsa `SourceFile`) updates its importers'
/// `imports_module_attribute` diagnostics. Returns `None` when `import` is not a
/// user stub or `stub_source` fails to parse.
#[must_use]
pub fn recapture_user_stub_from_source(
    import: &basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
    stub_source: &str,
) -> Option<(String, ImportedModuleApi)> {
    let stub_path = user_stub_path(import, search_paths)?;
    let tier = basilisk_stubs::user_stub_tier(stub_source);
    let stub = basilisk_stubs::parse_pyi_source(
        stub_source,
        stub_path,
        &import.module,
        basilisk_stubs::StubSource::UserStub,
        tier,
    )
    .ok()?;
    Some((
        import.module.clone(),
        build_stub_api(&stub, stub_path, search_paths),
    ))
}

/// Whether this import binds a user stub — i.e. its member API is re-derived
/// from the stub's **content**, so an importer's output genuinely depends on
/// that file's text. The incremental engine uses this to record a content edge
/// for exactly these imports and no others ([CHKARCH-INCREMENTAL-SALSA]).
#[must_use]
pub fn is_user_stub_import(
    import: &basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
) -> bool {
    user_stub_path(import, search_paths).is_some()
}

/// The `.pyi` path this import binds as a user stub, if any: a single-segment
/// plain `import X` resolved to a `.pyi` under a configured `stub-paths` dir
/// (which includes the auto-added `.basilisk/stubs`). Other `.pyi` (typeshed,
/// `*-stubs`, py.typed packages) are out of scope.
fn user_stub_path<'a>(
    import: &'a basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
) -> Option<&'a std::path::Path> {
    if import.kind != ImportKind::Plain || import.module.contains('.') {
        return None;
    }
    let stub_path = import.resolved_path.as_deref()?;
    if stub_path.extension().is_none_or(|ext| ext != "pyi") {
        return None;
    }
    search_paths
        .stub_paths
        .iter()
        .any(|dir| stub_path.starts_with(dir))
        .then_some(stub_path)
}

/// Build the [`ImportedModuleApi`] (top-level member names + `__getattr__`
/// presence) from a parsed user stub. Names the stub re-exports — redundant
/// aliases, `__all__` entries, and star-imported submodules' export sets
/// ([STUBRES-PYI-REEXPORTS], GitHub #312) — are members too.
///
/// Star targets that don't resolve beside the stub fall back to the active
/// step-3 Typeshed source: `MicroPython`'s `uio.pyi` is just `from io import *`
/// with `io` in the custom typeshed's `stdlib/` tree (GitHub #312 follow-up).
fn build_stub_api(
    stub: &basilisk_stubs::StubModule,
    stub_path: &std::path::Path,
    search_paths: &ImportSearchPaths,
) -> ImportedModuleApi {
    let mut member_names = std::collections::HashSet::new();
    member_names.extend(stub.functions.keys().cloned());
    member_names.extend(stub.classes.keys().cloned());
    member_names.extend(stub.variables.keys().cloned());
    member_names.extend(stub.overloads.keys().cloned());
    let mut snapshot_fallback = |module_name: &str| {
        let active = search_paths.typeshed_snapshot.as_ref()?;
        let (snapshot, target) = active.for_importer(Some(stub_path))?;
        crate::exports::parse_snapshot_stub(
            snapshot,
            target,
            module_name,
            snapshot_stub_source(snapshot),
        )
    };
    member_names.extend(
        basilisk_stubs::reexports::reexported_member_names_with_fallback(
            stub,
            &mut snapshot_fallback,
        ),
    );
    ImportedModuleApi {
        member_names,
        has_getattr: stub.functions.contains_key("__getattr__"),
        stub_path: stub_path.to_path_buf(),
    }
}

// Implements [LSPUV-HOVER-DATA-FLOW] steps 2-3 — match the import against the
// PackageRegistry and attach PackageInfo metadata onto the ImportInfo for hover.
/// Enrich an import with package metadata from the uv registry.
///
/// Sets `package_dep_kind`, `package_version`, and `package_name` in a
/// single registry lookup. No-op for stdlib modules, non-uv projects, or
/// imports not found in the registry.
fn enrich_package_metadata(
    import: &mut basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
) {
    let Some(registry) = search_paths.registry.as_ref() else {
        return;
    };

    // Skip modules that actually resolved from the selected step-3 source.
    // Name-only recognition is used only for the matching embedded bundle;
    // active/custom generations are identified by their resolved path.
    if is_standard_library_import(import, search_paths) {
        return;
    }

    // The registry is keyed by import name (issue #25): look the module up
    // directly — full dotted name first (`google.protobuf`), then the root.
    let root_module = import.module.split('.').next().unwrap_or(&import.module);
    let Some(info) = registry
        .lookup(&import.module)
        .or_else(|| registry.lookup(root_module))
    else {
        return;
    };

    import.package_dep_kind = Some(match info.kind {
        basilisk_uv::DepKind::Direct => PackageDepKind::Direct,
        basilisk_uv::DepKind::Dev => PackageDepKind::Dev,
        basilisk_uv::DepKind::Transitive => PackageDepKind::Transitive,
    });
    import.package_version = info.version.clone();
    import.package_name = Some(info.name.clone());
}

fn stub_distribution(
    module_name: &str,
    search_paths: &ImportSearchPaths,
    importing_file: Option<&std::path::Path>,
) -> Option<String> {
    if let Some(active) = &search_paths.typeshed_snapshot {
        return active
            .distribution_for_importer(importing_file, module_name)
            .map(ToOwned::to_owned);
    }
    None
}

fn is_standard_library_import(
    import: &basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
) -> bool {
    let Some(path) = import.resolved_path.as_deref() else {
        return false;
    };
    if search_paths.typeshed_snapshot.is_some() {
        return path.to_string_lossy().starts_with("typeshed:");
    }
    false
}
