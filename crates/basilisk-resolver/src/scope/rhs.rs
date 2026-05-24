//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Right-hand-side expression kind classification.

/// What kind of right-hand-side expression a variable assignment has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RhsKind {
    /// Integer literal — type is trivially `int`.
    IntLiteral,
    /// Float literal — type is trivially `float`.
    FloatLiteral,
    /// String literal — type is trivially `str`.
    StrLiteral,
    /// Boolean literal — type is trivially `bool`.
    BoolLiteral,
    /// Bytes literal — type is trivially `bytes`.
    BytesLiteral,
    /// A list literal with known element kinds.
    List(Vec<RhsKind>),
    /// A dict literal with known key/value kinds.
    Dict(Vec<(RhsKind, RhsKind)>),
    /// A set literal with known element kinds.
    Set(Vec<RhsKind>),
    /// A tuple literal with known element kinds.
    Tuple(Vec<RhsKind>),
    /// Empty list literal `[]` — element type unknown without annotation.
    EmptyList,
    /// Empty dict literal `{}` — key/value types unknown without annotation.
    EmptyDict,
    /// `None` literal — type is `None` / `NoneType`.
    NoneValue,
    /// A function or constructor call — return type may be unknown.
    CallExpr,
    /// A `type(X)` call — returns a class object (e.g. `type(None)` → `NoneType`).
    TypeCall,
    /// A lambda expression (`lambda x: x + 1`).
    Lambda,
    /// Any other expression.
    Other,
}
