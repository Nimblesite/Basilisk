//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Variable and attribute types for scope analysis.

use super::{rhs::RhsKind, span::Span};

/// The builtin descriptor a class-body assignment's RHS call applies, decided
/// from the callee's resolved binding — never from its spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorKind {
    /// The wrapped callable consumes no implicit receiver.
    StaticMethod,
    /// The wrapped callable consumes `cls` on every access path.
    ClassMethod,
}

/// A judgeable primitive class an annotation member resolves to.
///
/// Produced by resolving the member expression through the module's bindings
/// with the builtin fallback ([RESOLV-CANONICAL-BINDING]) — never by reading
/// its spelling — and consumed by comparing against the AST node kind of a
/// literal value, which carries its class in its syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveKind {
    /// `builtins.str`.
    Str,
    /// `builtins.bytes`.
    Bytes,
    /// `builtins.int`.
    Int,
    /// `builtins.float`.
    Float,
    /// `builtins.complex`.
    Complex,
    /// `builtins.bool`.
    Bool,
    /// `types.NoneType`, written `None` in annotations.
    NoneType,
}

impl PrimitiveKind {
    /// Whether a value of this class is accepted where `field` members are
    /// declared: exact match, `bool` ≤ `int`, and the PEP 484 numeric tower
    /// (`int` promotes to `float`, `float` to `complex`), composed
    /// transitively.
    #[must_use]
    pub fn accepted_by(self, field: &[Self]) -> bool {
        field.iter().any(|&declared| {
            self == declared
                || matches!(
                    (self, declared),
                    (Self::Bool, Self::Int | Self::Float | Self::Complex)
                        | (Self::Int, Self::Float | Self::Complex)
                        | (Self::Float, Self::Complex)
                )
        })
    }
}

impl std::fmt::Display for PrimitiveKind {
    /// Diagnostic **message** rendering; never an input to a verdict.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Str => "str",
            Self::Bytes => "bytes",
            Self::Int => "int",
            Self::Float => "float",
            Self::Complex => "complex",
            Self::Bool => "bool",
            Self::NoneType => "None",
        })
    }
}

/// A module-level or class-body variable assignment.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
    /// The descriptor wrapper when the right-hand-side is a call whose callee
    /// resolves to the builtin `staticmethod` or `classmethod`, else `None`.
    ///
    /// Resolved through the module's bindings with the builtin fallback, so an
    /// aliased import is recognised and a module-local shadow is not
    /// ([RESOLV-CANONICAL-BINDING]). In enum class bodies, static/class method
    /// descriptors are non-members; in ordinary class bodies the wrapper
    /// decides which implicit receiver a bound callable consumes
    /// ([#382](https://github.com/Nimblesite/Basilisk/issues/382)).
    pub rhs_descriptor: Option<DescriptorKind>,
    /// The simple name of the callable this attribute binds, when the
    /// right-hand-side is a bare name (`m = f`) or a descriptor wrapper around
    /// one (`s = staticmethod(g)`), else `None`. Class-body assignments of
    /// module-level functions bind them as methods ([#382]).
    pub rhs_name: Option<String>,
    /// `true` when the annotation contains `ReadOnly[...]` (directly or nested).
    ///
    /// Used by `typeddicts_readonly` to detect mutation of read-only `TypedDict` fields.
    pub is_readonly: bool,
    /// The explicit `Required`/`NotRequired` marking of a `TypedDict` field:
    /// `Some(true)` for `Required[...]`, `Some(false)` for `NotRequired[...]`,
    /// `None` when neither is written — the declaring class's `total=` then
    /// decides (PEP 655).
    ///
    /// Resolved through the module's bindings at collection time, through the
    /// `Annotated`/`ReadOnly` interleavings the PEPs permit and through quoted
    /// forward references — never from annotation text.
    pub required: Option<bool>,
    /// The primitive classes this field's annotation accepts, when every
    /// member of the (possibly union) annotation resolves to one.
    ///
    /// Resolved through the module's bindings with the builtin fallback at
    /// collection time, so `int`, `builtins.int`, and an aliased import all
    /// answer alike and a module-local `class int` answers not at all. `None`
    /// is abstention — some member is not a judgeable primitive — and
    /// consumers emit nothing for such fields.
    pub accepted_primitives: Option<Vec<PrimitiveKind>>,
    /// `true` when the annotation is the `Final` qualifier, bare or subscripted.
    ///
    /// Resolved through the module's bindings at collection time, so consumers
    /// never re-derive it from annotation text. Implements [RESOLV-CANONICAL-BINDING].
    pub is_final: bool,
    /// `true` when the annotation is the `ClassVar` qualifier, bare or subscripted.
    ///
    /// Resolved through the module's bindings at collection time, so consumers
    /// never re-derive it from annotation text. Implements [RESOLV-CANONICAL-BINDING].
    pub is_class_var: bool,
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
