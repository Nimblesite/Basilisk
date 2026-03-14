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
}
