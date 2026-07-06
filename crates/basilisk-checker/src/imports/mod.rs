//! Implements [ANALYSIS-CROSSLSP-IMPORT] / [ANALYSIS-INCR-IMPORTS]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP-IMPORT
//!
//! Import resolution engine — resolves `import X` to filesystem paths.
//!
//! Search order: workspace roots → extraPaths → venv site-packages.
//! File priority: `.pyi` stub preferred over `.py` source.
//!
//! This is the **filesystem-pure** core of import resolution: given an
//! already-built [`ImportSearchPaths`], it probes the filesystem and annotates a
//! [`basilisk_resolver::ResolvedModule`] in place. It lives in `basilisk-checker`
//! (below `basilisk-lsp`) so the memoized `checked_file` query can eventually
//! fold it in ([CHKARCH-INCREMENTAL-SALSA]). The config/venv/`uv.lock` adapter
//! that *constructs* an `ImportSearchPaths` (`search_paths_from_config`,
//! site-packages discovery) stays in `basilisk-lsp`, since it depends on the
//! LSP's `WorkspaceConfig`/`WorkspaceIndex`; it re-exports these symbols so
//! `basilisk_lsp::import_resolver::*` keeps resolving.

use std::path::PathBuf;
use std::sync::Arc;

use basilisk_resolver::scope::ImportResolution;
use basilisk_uv::PackageRegistry;

mod apply;
mod resolve;

pub use apply::{is_user_stub_import, recapture_user_stub_from_source, resolve_module_imports};
pub use resolve::{
    classify_unresolved, has_stub_package, is_inline_typed_package, resolve_module,
    resolve_module_with_importer, resolve_relative_import,
};

/// Result of resolving a single import to a filesystem path.
#[derive(Debug, Clone)]
pub struct ResolvedImport {
    /// Filesystem path of the resolved module.
    pub path: PathBuf,
    /// Whether this resolved to a `.pyi` stub or `.py` source.
    pub resolution: ImportResolution,
}

/// Search paths used for import resolution.
///
/// Derives `salsa::Update` so it can be the value of a `#[salsa::input]`
/// ([`crate::SearchPathsInput`]): the derive resolves each field through its
/// `PartialEq` (the `Arc<PackageRegistry>` compares the pooled registry by
/// value), mirroring the `CachedDiagnostic`/`ConfigValue` idiom, so no salsa
/// dependency leaks into `basilisk-uv`.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ImportSearchPaths {
    /// Workspace root directories.
    pub roots: Vec<PathBuf>,
    /// Extra module search paths from config (`extraPaths`).
    pub extra_paths: Vec<PathBuf>,
    /// User-provided stub directories from `stub-paths` config.
    pub stub_paths: Vec<PathBuf>,
    /// uv workspace member source roots (editable packages).
    ///
    /// Searched after workspace roots but before `extra_paths`.
    pub workspace_members: Vec<PathBuf>,
    /// Virtual environment site-packages directory, if detected.
    pub site_packages: Option<PathBuf>,
    /// Package registry from uv lock file, if available.
    pub registry: Option<Arc<PackageRegistry>>,
    /// Custom typeshed directory (`typeshed-path` config). When set, its
    /// `stdlib/` subtree is the **canonical source for standard-library types**
    /// — typing-spec import-resolution step 3
    /// ([STUBRES-CUSTOM-TYPESHED](../../../../docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED)).
    /// `None` keeps the name-only bundled-stdlib recognition.
    pub typeshed_path: Option<PathBuf>,
}
