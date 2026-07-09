//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
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

/// Why an import could not be resolved.
///
/// Used by `imports_unresolved` to produce context-aware diagnostic messages when uv
/// package registry information is available.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnresolvedReason {
    /// Package not in `uv.lock` at all.
    NotInstalled,
    /// In lock as transitive, not in `pyproject.toml` dependencies.
    NotInDeps,
    /// In `pyproject.toml` but lock file is stale or venv not synced.
    NeedsSync,
    /// Installed but no `.pyi` stubs available.
    NoStubs,
    /// stdlib module not available in the target Python version.
    WrongPythonVersion,
    /// Non-uv project or cannot determine reason.
    Unknown,
}

/// A single import statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportInfo {
    /// The dotted module name being imported (e.g. `"os.path"`, `"requests"`).
    pub module: String,
    /// Locally-bound names introduced by the import.
    /// `from X import A, B` → `["A", "B"]` (alias-aware: `import C as D` → `["D"]`).
    /// Plain `import X` / `import X.Y` is empty — the bound name is the top-level
    /// module (see `module`); aliased `import X as Y` → `["Y"]`.
    pub names: Vec<String>,
    /// The source span of the import statement.
    pub span: Span,
    /// Spans of the identifier tokens in the statement — the module path and
    /// each imported/alias name — excluding the `import`/`from`/`as` keywords.
    /// Semantic highlighting paints exactly these spans (GitHub #286).
    pub name_spans: Vec<Span>,
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
    /// Why the import could not be resolved, when known.
    ///
    /// Populated during workspace import resolution when a uv registry is
    /// available and the import is unresolved. `None` when no uv context is
    /// available or the import resolved successfully.
    pub unresolved_reason: Option<UnresolvedReason>,
}

/// The public member API of a module imported via a plain `import X`, captured
/// from its type stub so the checker can flag access to undeclared attributes.
///
/// Populated during workspace import resolution. Phase 1 only captures **user /
/// local stubs** (`stub-paths` / `.basilisk/stubs`), which the developer
/// controls and wants to be authoritative — so mere presence in
/// [`super::ResolvedModule::imported_modules`] is the gate, and third-party
/// typeshed / `py.typed` packages are deliberately not captured here yet.
/// Consumed by `imports_module_attribute`.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedModuleApi {
    /// Top-level names the stub declares (functions, classes, variables).
    pub member_names: std::collections::HashSet<String>,
    /// Whether the stub defines a module-level `__getattr__` (PEP 562). When
    /// true, any attribute access is permitted — the explicit opt-out that the
    /// create-local-stub skeleton ships by default.
    pub has_getattr: bool,
    /// Path to the `.pyi` stub, so the diagnostic can point the developer at the
    /// exact file to edit.
    pub stub_path: std::path::PathBuf,
}
