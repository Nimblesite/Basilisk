//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! AST-derived PEP 695 scoping facts consumed by `BSK-E0149`.
//!
//! These structures are populated purely from `ruff_python_ast` nodes (never
//! from raw `source.lines()` scanning), so docstring/comment/string content can
//! never be mistaken for real `class` / `def` / `type` declarations.

use super::span::Span;

/// The flavour of a PEP 695 type parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pep695ParamKind {
    /// A `TypeVar` parameter (`T`, or `T: Bound`).
    TypeVar,
    /// A `ParamSpec` parameter (`**P`).
    ParamSpec,
    /// A `TypeVarTuple` parameter (`*Ts`).
    TypeVarTuple,
}

/// A single PEP 695 type parameter with its optional bound.
#[derive(Debug, Clone)]
pub struct Pep695Param {
    /// The parameter name (e.g. `T`).
    pub name: String,
    /// The source span of the parameter name.
    pub span: Span,
    /// The parameter flavour.
    pub kind: Pep695ParamKind,
    /// Simple names referenced inside the bound expression (for cross-reference checks).
    pub bound_refs: Vec<String>,
    /// The source text of the bound expression, if any (for diagnostic messages).
    pub bound_text: Option<String>,
}

/// Which kind of construct declared a PEP 695 type parameter list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenericDefKind {
    /// A `class Name[...]` definition.
    Class,
    /// A `def`/`async def Name[...]` definition.
    Function,
}

/// A decorator applied to a generic definition.
#[derive(Debug, Clone)]
pub struct DecoratorRef {
    /// Simple names referenced anywhere in the decorator expression.
    pub refs: Vec<String>,
    /// The source span of the decorator expression.
    pub span: Span,
}

/// A `class` or `def` that declares a PEP 695 `[...]` type parameter list,
/// plus the scope facts `BSK-E0149` needs.
#[derive(Debug, Clone)]
pub struct Pep695Def {
    /// Whether this is a class or a function.
    pub kind: GenericDefKind,
    /// The construct name.
    pub name: String,
    /// The span of the name token.
    pub name_span: Span,
    /// The span of the `class`/`def` header (name token, used for diagnostics).
    pub def_span: Span,
    /// The declared type parameters, in order.
    pub params: Vec<Pep695Param>,
    /// Decorators applied to this construct.
    pub decorators: Vec<DecoratorRef>,
    /// Type-parameter names of the directly enclosing class, if this is a method.
    pub enclosing_class_params: Vec<String>,
}

/// A PEP 695 `type Name[...] = rhs` alias statement, resolved from the AST.
#[derive(Debug, Clone)]
pub struct Pep695AliasDef {
    /// The alias name.
    pub name: String,
    /// The span of the alias name token.
    pub name_span: Span,
    /// The declared type parameters, in order.
    pub params: Vec<Pep695Param>,
    /// Simple names referenced in the RHS value expression.
    pub rhs_refs: Vec<String>,
    /// Names referenced at the *top level* of the RHS — a bare `Name` or a direct
    /// member of a top-level `X | Y` union — but NOT names nested inside a
    /// subscript/container. A bare reference to another alias is non-terminating
    /// (`type A = B`), whereas one through a container (`type A = list[B]`) is
    /// legitimate recursion; this powers mutual-cycle detection (BSK-E0149).
    pub rhs_bare_refs: Vec<String>,
    /// When the RHS contains a self-referential subscript `Name[args]`, the
    /// simple argument names of the first such subscript.
    pub self_ref_args: Option<Vec<String>>,
    /// `true` when this alias is nested (directly or transitively) in a function body.
    pub in_function: bool,
}

/// An attribute access `Name.attr` somewhere in the module (outside `type` RHS).
#[derive(Debug, Clone)]
pub struct AttrAccess {
    /// The base name being accessed.
    pub base: String,
    /// The attribute name.
    pub attr: String,
    /// The span of the access (whole `Name.attr` expression).
    pub span: Span,
}

/// All PEP 695 scoping facts for a module, derived purely from the AST.
#[derive(Debug, Clone, Default)]
pub struct Pep695Scoping {
    /// Classes and functions that declare PEP 695 type parameters.
    pub defs: Vec<Pep695Def>,
    /// PEP 695 `type` alias statements.
    pub aliases: Vec<Pep695AliasDef>,
    /// Module-scope `Name` load references (outside any def/class/alias scope),
    /// with their spans. Used to detect type parameters used at module scope.
    pub module_name_refs: Vec<(String, Span)>,
    /// Names bound at module scope, paired with the byte offset of the binding.
    pub module_bindings: Vec<(String, u32)>,
    /// `Name.attr` accesses anywhere in the module (excluding `type` alias RHS).
    pub attr_accesses: Vec<AttrAccess>,
}
