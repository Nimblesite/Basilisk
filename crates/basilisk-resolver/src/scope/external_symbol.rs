//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! External symbols imported from other modules.
//!
//! When cross-module analysis resolves an import, the imported module's
//! exported symbols are extracted and attached to the importing module's
//! `ResolvedModule` as `ExternalSymbol` entries.

use std::path::PathBuf;
use std::sync::Arc;

use basilisk_stubs::{StubClass, TypeProvenance};

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

/// A method on an externally imported class.
///
/// Kept so hover can resolve member access on subclasses of external classes
/// (e.g. `Model.model_validate(...)` where `Model` extends pydantic's
/// `BaseModel`) — GitHub #287.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMethod {
    /// The method's name.
    pub name: String,
    /// The rendered `def` signature for hover display.
    pub signature: String,
    /// The method's docstring, when the defining module has one.
    ///
    /// `.pyi` stubs (typeshed included) carry no docstrings, so this is `None`
    /// for them; workspace and PEP 561 `py.typed` sources do carry them, and
    /// hover shows the prose it finds rather than dropping it
    /// ([LSPARCH-FEATURES-HOVER]).
    pub docstring: Option<String>,
}

/// One class declaration indexed from the active standard-library snapshot.
///
/// The structured [`StubClass`] is the single source consumed by checking,
/// hover, signature help, completion, and definition. `source_identity` is the
/// active immutable generation identity, while `source_path` is its logical
/// VFS URI rather than a mutable extracted file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedStubClass {
    /// Parsed class declaration, including every overload and receiver.
    pub declaration: StubClass,
    /// Stable `typeshed:<identity>/...` URI of the exact `.pyi` body.
    pub source_path: PathBuf,
    /// Stable active-snapshot identity used by caches and status surfaces.
    pub source_identity: String,
    /// Exact immutable `.pyi` body shared by every class from this module.
    pub source_text: Arc<str>,
    /// Built-in or user-managed custom-typeshed provenance.
    pub provenance: TypeProvenance,
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
    /// The symbol's docstring, when the defining module has one.
    ///
    /// `.pyi` stubs (typeshed included) carry no docstrings, so this is `None`
    /// for them; workspace and PEP 561 `py.typed` sources do carry them, and
    /// hover shows the prose it finds rather than dropping it
    /// ([LSPARCH-FEATURES-HOVER]).
    pub docstring: Option<String>,
    /// Where this symbol's type information came from.
    ///
    /// Set during cross-module resolution based on how the import was resolved.
    /// Used by the checker for cascade suppression and by hover for annotations.
    pub provenance: Option<TypeProvenance>,
    /// Methods of the class, when `kind` is [`ExternalSymbolKind::Class`].
    ///
    /// Empty for non-class symbols. Lets hover resolve inherited member access
    /// on subclasses of external classes (GitHub #287).
    pub methods: Vec<ExternalMethod>,
    /// Base-class names (text form, e.g. `"NonCallableMock"`),
    /// when `kind` is [`ExternalSymbolKind::Class`].
    ///
    /// Empty for non-class symbols. Carries the external class's declared bases
    /// so the constructor/MRO machinery can walk the method-resolution order of
    /// an external class (e.g. `unittest.mock.Mock` → `NonCallableMock.__new__`
    /// + `CallableMixin.__init__`) instead of stopping at local classes
    ///
    /// ([STUBRES-PYI] #289).
    pub bases: Vec<String>,
    /// Explicit metaclass name for a class imported from source or a `.pyi`.
    ///
    /// This is separate from [`Self::bases`] because the metaclass participates
    /// in class-call conversion, not the instance MRO.
    pub metaclass: Option<String>,
    /// Bound `__call__` overloads resolved from [`Self::metaclass`].
    ///
    /// Constructor conversion uses these only when a return type is not the
    /// constructed instance; ordinary instance-producing calls continue to
    /// `__new__` and `__init__` ([STUBRES-PYI] #289).
    pub metaclass_calls: Vec<ExternalMethod>,
}
