//! Import-related types.

use super::span::Span;

/// How an import statement is structured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportKind {
    /// `import X` or `import X as Y`
    Plain,
    /// `from X import Y` or `from X import Y as Z`
    From,
    /// `from X import *`
    Star,
}

/// How an import was resolved (source file type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportResolution {
    /// Import resolved from a .py source file.
    SourcePy,
    /// Import resolved from a .pyi stub file.
    StubPyi,
    /// Import resolution failed or not yet resolved.
    Unresolved,
}

/// Classification of an import's dependency relationship, as determined by
/// the package manager (e.g. from `uv.lock`).
///
/// Set during workspace import resolution when a uv package registry is
/// available. `None` for non-uv projects or stdlib/local imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageDepKind {
    /// Declared in `[project.dependencies]`.
    Direct,
    /// Declared in dev-dependency groups.
    Dev,
    /// Pulled in by another dependency, not declared directly.
    Transitive,
}

/// A single import statement.
#[derive(Debug, Clone)]
pub struct ImportInfo {
    /// The dotted module name being imported (e.g. `"os.path"`, `"requests"`).
    pub module: String,
    /// Names imported from the module (`from X import A, B` → `["A", "B"]`).
    /// Empty for plain `import X` statements.
    pub names: Vec<String>,
    /// The source span of the import statement.
    pub span: Span,
    /// The kind of import.
    pub kind: ImportKind,
    /// How the import was resolved (source file type).
    pub resolution: ImportResolution,
    /// Filesystem path the import resolved to, if known.
    pub resolved_path: Option<std::path::PathBuf>,
    /// Dependency classification from the package manager (e.g. uv.lock).
    ///
    /// `None` for non-uv projects, stdlib modules, or local imports.
    /// Set during workspace import resolution.
    pub package_dep_kind: Option<PackageDepKind>,
    /// Package version from the lock file (e.g. `"2.31.0"`).
    ///
    /// Populated during workspace import resolution when a uv registry is
    /// available. `None` for stdlib, local, or non-uv imports.
    pub package_version: Option<String>,
    /// Package name from the lock file (e.g. `"requests"`).
    ///
    /// Populated during workspace import resolution when a uv registry is
    /// available. `None` for stdlib, local, or non-uv imports.
    pub package_name: Option<String>,
}
