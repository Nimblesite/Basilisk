//! Implements [ANALYSIS-INCR-IMPORTS]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INCR-IMPORTS
//! Applies path resolution to every import in a module, in place.

use std::path::PathBuf;

use basilisk_resolver::scope::{ImportKind, ImportedModuleApi, PackageDepKind};

use super::resolve::{classify_unresolved, resolve_module_with_importer};
use super::ImportSearchPaths;

/// Resolve every import in a single module against the search paths, in place.
///
/// Sets each `ImportInfo`'s `resolution`, `resolved_path`, `unresolved_reason`,
/// and uv package metadata. Shared by the whole-workspace scan
/// (`resolve_workspace_imports`) and the incremental single-file analysis
/// path so both agree on what resolves — preventing false `imports_unresolved` in the
/// editor for third-party imports that the CLI resolves.
/// Implements [ANALYSIS-INCR-IMPORTS].
pub fn resolve_module_imports(
    resolved: &mut basilisk_resolver::ResolvedModule,
    search_paths: &ImportSearchPaths,
) {
    // The file's own path, used to search its directory for sibling modules.
    let importing_file = PathBuf::from(&resolved.path);

    // Member APIs captured during the loop, inserted after it ends — we cannot
    // borrow `resolved.imported_modules` while iterating `resolved.imports`.
    let mut captured: Vec<(String, ImportedModuleApi)> = Vec::new();

    for import in &mut resolved.imports {
        let result = match import.kind {
            ImportKind::Plain | ImportKind::From | ImportKind::Star => {
                resolve_module_with_importer(&import.module, search_paths, Some(&importing_file))
            }
        };
        if let Some(r) = result {
            import.resolution = r.resolution;
            import.resolved_path = Some(r.path);
        } else if !basilisk_stubs::is_stdlib_module(&import.module) {
            // Classify why the import is unresolved for actionable diagnostics.
            import.unresolved_reason = Some(classify_unresolved(&import.module, search_paths));
        }

        // Capture the member API of plain `import X` statements backed by a
        // user/local stub, so `imports_module_attribute` can flag `X.undeclared_attr`. Only
        // single-segment plain imports resolved to a `.pyi` under a configured
        // `stub-paths` dir (Phase 1: the stubs the developer owns).
        if let Some((binding, api)) = capture_user_stub_api(import, search_paths) {
            captured.push((binding, api));
        }

        // Annotate with package metadata from the uv registry.
        enrich_package_metadata(import, search_paths);
    }

    for (binding, api) in captured {
        let _ = resolved.imported_modules.insert(binding, api);
    }
}

/// Build the [`ImportedModuleApi`] for a plain `import X` backed by a user stub,
/// or `None` if this import is out of scope (aliased/dotted/from-import, not a
/// user stub, or the stub fails to parse).
fn capture_user_stub_api(
    import: &basilisk_resolver::ImportInfo,
    search_paths: &ImportSearchPaths,
) -> Option<(String, ImportedModuleApi)> {
    let stub_path = user_stub_path(import, search_paths)?;
    let stub = basilisk_stubs::parse_pyi_file(
        stub_path,
        &import.module,
        basilisk_stubs::StubSource::UserStub,
        basilisk_stubs::StubTier::Tier1,
    )
    .ok()?;
    Some((import.module.clone(), build_stub_api(&stub, stub_path)))
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
    let stub = basilisk_stubs::parse_pyi_source(
        stub_source,
        stub_path,
        &import.module,
        basilisk_stubs::StubSource::UserStub,
        basilisk_stubs::StubTier::Tier1,
    )
    .ok()?;
    Some((import.module.clone(), build_stub_api(&stub, stub_path)))
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
/// presence) from a parsed user stub.
fn build_stub_api(
    stub: &basilisk_stubs::StubModule,
    stub_path: &std::path::Path,
) -> ImportedModuleApi {
    let mut member_names = std::collections::HashSet::new();
    member_names.extend(stub.functions.keys().cloned());
    member_names.extend(stub.classes.keys().cloned());
    member_names.extend(stub.variables.keys().cloned());
    member_names.extend(stub.overloads.keys().cloned());
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

    // Skip stdlib modules — they have no package metadata.
    if basilisk_stubs::is_stdlib_module(&import.module) {
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
    import.package_version = Some(info.version.clone());
    import.package_name = Some(info.name.clone());
}
