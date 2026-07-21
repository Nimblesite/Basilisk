//! Implements [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
//!
//! Cross-module resolution fixtures for LSP feature tests.
//!
//! Feature tests that need `imported_symbols` populated from a real Typeshed
//! snapshot must enter through the salsa `cross_resolved_module` query — the
//! same query the LSP itself uses ([TYPESHEDRT-ACCEPTANCE-HOVER]). Building
//! that database is identical in every such test, so it lives here once.

use std::path::PathBuf;
use std::sync::Arc;

use basilisk_checker::imports::{ActiveTypeshed, ImportSearchPaths};
use basilisk_checker::{
    cross_resolved_module, BasiliskDatabase, FileRegistry, ResolvedFile, SearchPathsInput,
    SourceFile, WorkspaceFiles,
};
use basilisk_resolver::ResolvedModule;

/// Import search paths that resolve **only** through `typeshed`.
///
/// `roots` are the workspace roots; pass an empty vector when the fixture has
/// no workspace files of its own.
#[must_use]
pub fn typeshed_search_paths(typeshed: ActiveTypeshed, roots: Vec<PathBuf>) -> ImportSearchPaths {
    ImportSearchPaths {
        roots,
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: Some(typeshed),
    }
}

/// Resolve `source` as `main.py` through the salsa cross-module query, so its
/// `imported_symbols` carry everything the LSP would see at request time.
///
/// Returns `None` when the source does not parse or resolve, leaving the
/// assertion (and its message) to the caller.
#[must_use]
pub fn cross_resolve(source: &str, search_paths: ImportSearchPaths) -> Option<Arc<ResolvedModule>> {
    let database = BasiliskDatabase::default();
    let search_input = SearchPathsInput::new(&database, search_paths);
    let workspace = WorkspaceFiles::new(&database, FileRegistry::default());
    let file = SourceFile::new(&database, "main.py".to_owned(), source.to_owned());
    match cross_resolved_module(&database, file, search_input, workspace) {
        ResolvedFile::Resolved(resolved) => Some(Arc::clone(resolved)),
        ResolvedFile::ParseError(_) | ResolvedFile::ResolveError => None,
    }
}
