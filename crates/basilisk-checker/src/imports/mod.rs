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
mod fs_cache;
mod resolve;
#[cfg(test)]
mod resolve_tests;

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

#[derive(Debug, Clone)]
struct TypeshedBinding {
    root: Option<PathBuf>,
    snapshot: Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
    target: Option<basilisk_stubs::types::StubTarget>,
}

/// Root-keyed active immutable Typeshed snapshots plus their target evidence.
/// A single-root CLI uses [`Self::new`]; a multi-root LSP uses
/// [`Self::from_roots`]. Equality deliberately compares root+identity+target,
/// not `Arc` addresses, so Salsa invalidates only when semantics change.
#[derive(Debug, Clone)]
pub struct ActiveTypeshed {
    primary: TypeshedBinding,
    additional: Vec<TypeshedBinding>,
}

impl ActiveTypeshed {
    /// Pair a gate-accepted snapshot with the project/interpreter target.
    #[must_use]
    pub fn new(
        snapshot: Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
        target: Option<basilisk_stubs::types::StubTarget>,
    ) -> Self {
        Self {
            primary: TypeshedBinding {
                root: None,
                snapshot,
                target,
            },
            additional: Vec::new(),
        }
    }

    /// Build a multi-root generation map. Longest-prefix ownership decides
    /// which snapshot resolves an importing file. Returns `None` for no roots.
    #[must_use]
    pub fn from_roots(
        mut bindings: Vec<(
            PathBuf,
            Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
            Option<basilisk_stubs::types::StubTarget>,
        )>,
    ) -> Option<Self> {
        bindings.sort_by(|left, right| left.0.cmp(&right.0));
        let mut bindings = bindings
            .into_iter()
            .map(|(root, snapshot, target)| TypeshedBinding {
                root: Some(root),
                snapshot,
                target,
            });
        let primary = bindings.next()?;
        Some(Self {
            primary,
            additional: bindings.collect(),
        })
    }

    /// The shared active snapshot.
    #[must_use]
    pub fn snapshot(&self) -> &Arc<basilisk_stubs::typeshed::snapshot::Snapshot> {
        &self.primary.snapshot
    }

    /// Concrete Python target evidence, if configured or discovered.
    #[must_use]
    pub fn target(&self) -> Option<&basilisk_stubs::types::StubTarget> {
        self.primary.target.as_ref()
    }

    /// Stable semantic identity for checker-cache fingerprints. Pin policy is
    /// absent because identical commit bytes intentionally share a cache key.
    #[must_use]
    pub fn identity_fingerprint(&self) -> String {
        self.bindings()
            .map(|binding| binding.snapshot.identity.uri_component())
            .collect::<Vec<_>>()
            .join("+")
    }

    pub(crate) fn for_importer(
        &self,
        importer: Option<&std::path::Path>,
    ) -> Option<(
        &Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
        Option<&basilisk_stubs::types::StubTarget>,
    )> {
        let owned = importer.and_then(|path| {
            self.bindings()
                .filter(|binding| {
                    binding
                        .root
                        .as_deref()
                        .is_some_and(|root| path.starts_with(root))
                })
                .max_by_key(|binding| {
                    binding
                        .root
                        .as_deref()
                        .map_or(0, |root| root.components().count())
                })
        });
        let binding = owned
            .or_else(|| self.bindings().find(|binding| binding.root.is_none()))
            // An importer outside the workspace is unambiguous when exactly
            // one rooted generation is active. Never guess between roots.
            .or_else(|| self.additional.is_empty().then_some(&self.primary))?;
        Some((&binding.snapshot, binding.target.as_ref()))
    }

    pub(crate) fn source_for_uri(
        &self,
        uri: &str,
        target: Option<&basilisk_stubs::types::StubTarget>,
    ) -> Option<(
        &Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
        Option<&basilisk_stubs::types::StubTarget>,
        &str,
    )> {
        self.bindings()
            .filter(|binding| binding.target.as_ref() == target)
            .find_map(|binding| {
                binding
                    .snapshot
                    .vfs
                    .read_uri(uri)
                    .map(|source| (&binding.snapshot, binding.target.as_ref(), source))
            })
    }

    pub(crate) fn distribution_for_importer(
        &self,
        importer: Option<&std::path::Path>,
        module_name: &str,
    ) -> Option<&str> {
        self.for_importer(importer)?
            .0
            .distribution_index
            .distribution(module_name)
    }

    fn bindings(&self) -> impl Iterator<Item = &TypeshedBinding> {
        std::iter::once(&self.primary).chain(&self.additional)
    }
}

impl PartialEq for ActiveTypeshed {
    fn eq(&self, other: &Self) -> bool {
        self.additional.len() == other.additional.len()
            && self.bindings().zip(other.bindings()).all(|(left, right)| {
                left.root == right.root
                    && left.snapshot.identity == right.snapshot.identity
                    && left.target == right.target
            })
    }
}

impl Eq for ActiveTypeshed {}

/// Search paths used for import resolution.
///
/// Derives `salsa::Update` so it can be the value of a `#[salsa::input]`
/// ([`crate::SearchPathsInput`]): the derive resolves each field through its
/// `PartialEq` (the `Arc<PackageRegistry>` compares the pooled registry by
/// value), mirroring the `CachedDiagnostic`/`ConfigValue` idiom, so no salsa
/// dependency leaks into `basilisk-uv`.
#[derive(Debug, Clone, Default, PartialEq, Eq, salsa::Update)]
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
    /// Gate-accepted runtime Typeshed generation. When present, every step-3
    /// name/body/index lookup comes from this exact identity. Configuration
    /// paths never enter the checker directly; acquisition first promotes them
    /// to this immutable snapshot.
    pub typeshed_snapshot: Option<ActiveTypeshed>,
}
