//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! External symbols imported from other modules.
//!
//! When cross-module analysis resolves an import, the imported module's
//! exported symbols are extracted and attached to the importing module's
//! `ResolvedModule` as `ExternalSymbol` entries.

use std::path::PathBuf;

use basilisk_stubs::TypeProvenance;

use super::span::Span;

/// Kind of an externally imported symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalSymbolKind {
    /// A function or method definition.
    Function,
    /// A class definition.
    Class,
    /// A module-level variable or constant.
    Variable,
    /// A re-exported symbol (imported in the source module, then re-exported).
    ReExport,
}

/// A symbol imported from another module during cross-module analysis.
///
/// Contains enough information for the checker and LSP to provide
/// cross-file type checking, hover, go-to-definition, etc.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalSymbol {
    /// The symbol's name as it appears in the source module.
    pub name: String,
    /// What kind of symbol this is.
    pub kind: ExternalSymbolKind,
    /// Type annotation text, if available (e.g. `"int"`, `"List[str]"`).
    pub type_annotation: Option<String>,
    /// The file path where this symbol is defined.
    pub source_path: PathBuf,
    /// Byte span of the symbol's name in the source file.
    pub source_span: Span,
    /// The full function/class signature for hover display.
    pub signature: Option<String>,
    /// Where this symbol's type information came from.
    ///
    /// Set during cross-module resolution based on how the import was resolved.
    /// Used by the checker for cascade suppression and by hover for annotations.
    pub provenance: Option<TypeProvenance>,
}
