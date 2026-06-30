//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Variable and attribute types for scope analysis.

use super::{rhs::RhsKind, span::Span};

/// A module-level or class-body variable assignment.
#[derive(Debug, Clone)]
pub struct VariableInfo {
    /// The variable name.
    pub name: String,
    /// The span of the variable name token.
    pub name_span: Span,
    /// `true` when an explicit type annotation is present (`x: T = ...`).
    pub has_annotation: bool,
    /// What kind of value is on the right-hand side.
    pub rhs_kind: RhsKind,
    /// The source span of the annotation expression, if present (`x: T = ...` → span of `T`).
    pub annotation_span: Option<Span>,
    /// The source span of the right-hand side expression, if present (`x = expr` → span of `expr`).
    pub rhs_span: Option<Span>,
}

/// A class attribute (declared in the class body).
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Python attributes have many boolean flags"
)]
pub struct AttributeInfo {
    /// The attribute name.
    pub name: String,
    /// The span of the attribute name token.
    pub name_span: Span,
    /// `true` when an explicit type annotation is present.
    pub has_annotation: bool,
    /// The span of the annotation expression, if present.
    pub annotation_span: Option<Span>,
    /// `true` when a right-hand-side value is present (`name: Type = value`).
    pub has_value: bool,
    /// The kind of right-hand-side expression, if a value is present.
    pub rhs_kind: RhsKind,
    /// The source span of the right-hand-side expression, if present.
    pub rhs_span: Option<Span>,
    /// `true` when the right-hand-side is a call to `nonmember(...)`.
    ///
    /// In enum class bodies, `nonmember(value)` explicitly marks an attribute
    /// as a non-member so it is not treated as an enum value.
    pub rhs_is_nonmember_call: bool,
    /// `true` when the right-hand-side is a lambda expression (`attr = lambda ...`).
    ///
    /// In enum class bodies, lambda attributes are non-members.
    pub rhs_is_lambda: bool,
    /// `true` when the right-hand-side is a call to `staticmethod(...)` or `classmethod(...)`.
    ///
    /// In enum class bodies, static/class method descriptors are non-members.
    pub rhs_is_descriptor_call: bool,
    /// `true` when the annotation contains `ReadOnly[...]` (directly or nested).
    ///
    /// Used by `typeddicts_readonly` to detect mutation of read-only `TypedDict` fields.
    pub is_readonly: bool,
    /// `true` when the field is keyword-only in a dataclass `__init__`.
    ///
    /// A field is `kw_only` when:
    /// - It appears after the `_: KW_ONLY` sentinel in the class body.
    /// - It uses `field(kw_only=True, ...)` as its value.
    /// - The class is `@dataclass(kw_only=True)` and the field does not use `field(kw_only=False)`.
    pub is_kw_only: bool,
    /// `true` when the field is excluded from the dataclass `__init__`.
    ///
    /// A field has `init=False` when:
    /// - It uses `field(init=False)` as its value.
    /// - A `dataclass_transform` field specifier function implicitly sets `init=False`
    ///   (e.g. via an overload with `init: Literal[False]` as default).
    pub is_init_false: bool,
    /// `true` when the field is an `InitVar[T]` annotation.
    ///
    /// `InitVar` fields are not real attributes — they are passed as parameters
    /// to `__post_init__` and cannot be accessed as instance attributes.
    pub is_init_var: bool,
    /// Static `if`-guard the field was defined under, if any (`None` for an
    /// unconditional member). A field whose guard is statically false at the
    /// target version is pruned by `resolve_with_target`, so consumers see only
    /// the members that exist for the target.
    pub guard: Option<crate::static_condition::StaticCondition>,
}
