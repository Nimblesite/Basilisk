//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Violation types collected during resolution for use by checker rules.

use super::span::Span;

/// A violation of `Final` typing rules, collected during resolution.
///
/// These are gathered in the resolver so that the checker rule (`E0047`) can
/// emit them without duplicating AST-walking logic.
#[derive(Debug, Clone, PartialEq)]
pub struct FinalViolationInfo {
    /// The kind of violation.
    pub kind: FinalViolationKind,
    /// The source span to highlight for this violation.
    pub span: Span,
    /// Human-readable name of the variable or attribute involved.
    pub name: String,
}

/// What kind of `Final` violation was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalViolationKind {
    /// `ID2: Final` — class attribute with bare `Final` and no initializer,
    /// not set unconditionally in `__init__`.
    ClassFinalWithoutInit,
    /// `self.id3: Final = 1` — `Final` annotation on a self-attribute in a
    /// method other than `__init__`.
    InstanceFinalOutsideInit,
    /// `self.ID5 = 0` — assignment to a self-attribute that was already given
    /// a value in the class body as `ID5: Final[int] = 0`.
    InstanceReassignAlreadyInitialized,
    /// `self.ID7 = 0` — assignment to a self-attribute declared `Final` in the
    /// class body (regardless of whether it had an initializer there).
    InstanceModifyFinal,
    /// `RATE = 300` — bare re-assignment to a module-level `Final` variable.
    ModuleLevelReassignment,
    /// `ClassB.DEFAULT_ID = 0` — attribute assignment via a class reference
    /// where the attribute is declared `Final`.
    ClassAttributeReassignment,
    /// `BORDER_WIDTH = 2.5` in a subclass — overriding a `Final` attribute
    /// inherited from a parent class.
    SubclassOverrideFinal,
    /// `x += 1` / `a = (x := 4)` etc. — modifying a function-local `Final`.
    FunctionLocalFinalModification,
    /// `ID1 = 2` after `global ID1` — modifying a global `Final` from inside
    /// a function.
    GlobalFinalModification,
}

/// Information about an enum `_value_` type mismatch detected during resolution.
///
/// Populated by the resolver visitor; used by `dataclasses_hash` to emit diagnostics
/// without re-walking the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumValueTypeViolationInfo {
    /// The kind of violation.
    pub kind: EnumValueTypeViolationKind,
    /// The source span to highlight.
    pub span: Span,
    /// The name of the enum class.
    pub class_name: String,
    /// The declared `_value_` annotation text.
    pub declared_type: String,
    /// The actual type that was assigned (as a descriptive string).
    pub actual_type: String,
}

/// What kind of `_value_` type violation was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumValueTypeViolationKind {
    /// A member literal value in the class body conflicts with `_value_: T`.
    ///
    /// E.g. `_value_: int` but `GREEN = "green"`.
    MemberValueTypeMismatch,
    /// `self._value_ = param` in `__init__` where `param`'s annotation conflicts
    /// with `_value_: T`.
    ///
    /// E.g. `_value_: str` but `def __init__(self, value: int, ...)`.
    InitValueParamTypeMismatch,
}

/// A `ClassVar` annotation used inside a function body (local variable or
/// self-attribute assignment) where it is not valid.
///
/// PEP 526 forbids `ClassVar` in function bodies, including:
/// - `x: ClassVar[str] = ""` — local variable annotation
/// - `self.xx: ClassVar[str] = ""` — attribute annotation on `self` in a method
///
/// Used by `classes_classvar`.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalClassVarViolation {
    /// The variable or attribute name being annotated.
    pub name: String,
    /// The source span of the name token.
    pub name_span: Span,
    /// Whether this is a self-attribute annotation (e.g. `self.xx: ClassVar[str]`).
    pub is_self_attr: bool,
}

/// What kind of PEP 695 type parameter bound violation was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pep695BoundViolationKind {
    /// The bound is a list literal, e.g. `class Foo[T: [str, int]]`.
    ListLiteralBound,
    /// The constraint tuple is empty, e.g. `class Foo[T: ()]`.
    EmptyTuple,
    /// The constraint tuple has only one element, e.g. `class Foo[T: (str,)]`.
    SingleElementTuple,
    /// The constraint is a non-literal variable, e.g. `class Foo[T: t1]` where `t1 = (bytes, str)`.
    NonLiteralConstraint,
    /// A constraint tuple element is not a valid type (e.g. an integer literal `(3, bytes)`).
    InvalidConstraintElement,
    /// The bound/constraint references a type variable from outer scope.
    OuterScopeTypeVarInBound,
}

/// A PEP 695 type parameter bound violation.
#[derive(Debug, Clone, PartialEq)]
pub struct Pep695BoundViolation {
    /// The kind of violation.
    pub kind: Pep695BoundViolationKind,
    /// The name of the class containing the type parameter.
    pub class_name: String,
    /// The name of the type parameter with the invalid bound.
    pub type_param_name: String,
    /// The span to highlight.
    pub span: Span,
}

/// What kind of historical positional-only parameter violation was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalPositionalViolationKind {
    /// A `__`-prefixed positional-only parameter was passed as a keyword argument.
    ///
    /// E.g. `f1(__x=3)` where `def f1(__x: int) -> None: ...`.
    KeywordPassedToPositionalOnly,
    /// A `__`-prefixed parameter appears after a non-`__` positional-or-keyword parameter.
    ///
    /// E.g. `def f2(x: int, __y: int) -> None: ...`.
    PositionalOnlyAfterKeyword,
}

/// A historical positional-only parameter violation.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoricalPositionalViolation {
    /// The kind of violation.
    pub kind: HistoricalPositionalViolationKind,
    /// The span to highlight (either the call site or the function definition).
    pub span: Span,
    /// The name involved (parameter name or function name).
    pub name: String,
}

/// What kind of invalid string annotation was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidStringAnnotationKind {
    /// The annotation string contains a non-name, non-subscript expression.
    NonTypeExpression,
    /// The annotation string starts with `f"` (f-string, not a valid type annotation).
    FStringAnnotation,
    /// The annotation string is used in a union with `|` operator (e.g. `"ClassA" | int`).
    StringInUnion,
}

/// An invalid string annotation detected during AST resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct InvalidStringAnnotation {
    /// The kind of invalidity.
    pub kind: InvalidStringAnnotationKind,
    /// The span of the annotation expression.
    pub span: Span,
}

/// A protocol conformance violation where a class is passed where a `Protocol`
/// with `Self`-returning methods is expected, but the class's corresponding
/// method does not return `Self` or the class itself.
///
/// ```python
/// class ShapeProtocol(Protocol):
///     def set_scale(self, scale: float) -> Self: ...
///
/// class BadReturn:
///     def set_scale(self, scale: float) -> int:  # returns int, not Self
///         return 42
///
/// def accepts(s: ShapeProtocol) -> None: ...
/// accepts(BadReturn())  # E — BadReturn does not conform
/// ```
///
/// Used by `namedtuples_type_compat`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolSelfViolation {
    /// The name of the class that was passed as argument.
    pub class_name: String,
    /// The name of the protocol that is expected.
    pub protocol_name: String,
    /// The method name with the `Self` return type in the protocol.
    pub method_name: String,
    /// What the class's method actually returns (source text of return annotation).
    pub actual_return_type: String,
    /// The span of the call argument.
    pub span: Span,
}

/// A direct instantiation of a Protocol class or a concrete class that fails
/// to implement all required members of its Protocol base(s).
///
/// The typing spec forbids instantiating Protocol classes directly, and
/// concrete subclasses that do not implement all abstract/stub methods or
/// required `ClassVar` attributes are effectively abstract and cannot be
/// instantiated.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolInstantiationViolation {
    /// The name of the class being instantiated.
    pub class_name: String,
    /// The span of the call expression.
    pub span: Span,
    /// `true` when the class is a concrete subclass missing protocol members
    /// (treated as abstract), `false` when it is a Protocol class itself.
    pub is_abstract: bool,
}

/// A violation related to `isinstance`/`issubclass` calls on Protocol classes
/// that are not decorated with `@runtime_checkable` or are data protocols used
/// with `issubclass`.
///
/// Used by `protocols_runtime_checkable`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolRtcViolation {
    /// The span of the offending call expression.
    pub span: Span,
    /// The kind of violation.
    pub kind: ProtocolRtcViolationKind,
}

/// The kind of `@runtime_checkable` protocol violation.
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolRtcViolationKind {
    /// Protocol is not decorated with `@runtime_checkable`.
    NotRuntimeCheckable {
        /// The protocol class name.
        protocol_name: String,
        /// `isinstance` or `issubclass`.
        call_name: String,
    },
    /// A data protocol used with `issubclass()`.
    IssubclassDataProtocol {
        /// The protocol class name.
        protocol_name: String,
    },
}

/// A generator-related type violation detected during resolution.
///
/// Covers:
/// - Generator function with non-generator return type (e.g. `-> int` with `yield`)
/// - Yield type mismatch (`yield 3` in `Generator[str, ...]`)
/// - Yield-from type mismatch (`yield from iter_a` in `Iterator[B]` where A != B)
///
/// Used by `directives_deprecated`.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratorViolation {
    /// The span of the offending expression.
    pub span: Span,
    /// The kind of violation.
    pub kind: GeneratorViolationKind,
}

/// The kind of generator violation.
#[derive(Debug, Clone, PartialEq)]
pub enum GeneratorViolationKind {
    /// A generator function (has yield) with a non-generator return type.
    InvalidReturnType {
        /// The function name.
        func_name: String,
        /// The declared return type text.
        return_type: String,
    },
    /// Yield expression produces a type incompatible with the declared yield type.
    YieldTypeMismatch {
        /// The expected yield type from the annotation.
        expected: String,
        /// The actual inferred yield type.
        actual: String,
    },
    /// `yield from` expression produces a type incompatible with the declared yield type.
    YieldFromTypeMismatch {
        /// The expected yield type from the annotation.
        expected: String,
        /// The actual inferred yield type.
        actual: String,
    },
    /// `yield from` subgenerator has an incompatible send type.
    YieldFromSendTypeMismatch {
        /// The expected send type from the outer generator annotation.
        expected: String,
        /// The actual send type from the subgenerator.
        actual: String,
    },
}

/// An unbound type variable usage detected during AST resolution.
///
/// Captures cases where a `TypeVar` is used outside its binding scope, such as:
/// - Inner class reusing an outer class's `TypeVar` in `Generic[T]`
/// - Inner class body annotations using outer class `TypeVars`
/// - Function-nested class using function-scoped `TypeVars` in `Generic[T]`
#[derive(Debug, Clone, PartialEq)]
pub struct UnboundTypeVarUsage {
    /// The source span of the unbound usage.
    pub span: Span,
    /// The name of the type variable that is unbound.
    pub typevar_name: String,
    /// Human-readable context description (e.g. "inner class `Bad`").
    pub context: String,
}

/// A violation where augmented assignment widens a `Literal`-typed variable.
#[derive(Debug, Clone, PartialEq)]
pub struct LiteralAugmentedAssignViolation {
    /// The source span of the augmented assignment.
    pub span: Span,
    /// The name of the variable being augmented.
    pub var_name: String,
}

/// A violation where a tuple is indexed out of bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct TupleIndexViolation {
    /// The source span of the index expression.
    pub span: Span,
    /// The name of the tuple variable.
    pub tuple_var_name: String,
    /// The literal index value used.
    pub index_value: i64,
    /// The length of the fixed-size tuple.
    pub tuple_length: usize,
}

/// A violation where an attribute is accessed on a bounded type variable
/// that does not exist on the bound type.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundedTypeVarAttrViolation {
    /// The source span of the attribute access.
    pub span: Span,
    /// The name of the type variable.
    pub typevar_name: String,
    /// The name of the parameter typed with the type variable.
    pub param_name: String,
    /// The bound type of the type variable.
    pub bound_type: String,
    /// The attribute name that was accessed.
    pub attr_name: String,
}

/// A violation where a Protocol class is used where `type[Proto]` is expected.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolClassObjectViolation {
    /// The source span of the violation.
    pub span: Span,
    /// The name of the Protocol class.
    pub protocol_name: String,
    /// Description of the context (e.g. "argument" or "assignment").
    pub context: String,
}

/// A violation of `ReadOnly` `TypedDict` field mutation rules.
///
/// Covers module-level subscript assignment (`td["key"] = val`) and `.update()` calls
/// on `TypedDict` variables that have `ReadOnly` fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadOnlyViolationInfo {
    /// The name of the variable being mutated.
    pub var_name: String,
    /// The `ReadOnly` field key being mutated, if applicable (subscript assignment).
    pub field_name: Option<String>,
    /// The kind of violation.
    pub kind: ReadOnlyViolationKind,
    /// Span of the offending expression.
    pub span: Span,
}

/// Kind of `ReadOnly` `TypedDict` mutation violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOnlyViolationKind {
    /// Direct subscript assignment: `td["key"] = val`.
    SubscriptAssign,
    /// `.update(...)` call on a `TypedDict` with `ReadOnly` fields.
    UpdateCall,
}

/// A violation detected in a `TypeAliasType(...)` call.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasTypeViolation {
    /// The source span to highlight.
    pub span: Span,
    /// The kind of violation.
    pub kind: TypeAliasTypeViolationKind,
    /// The alias name (LHS variable).
    pub alias_name: String,
}

/// Kind of `TypeAliasType` violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeAliasTypeViolationKind {
    /// The value argument is not a valid type expression.
    InvalidTypeExpression,
    /// The alias value references itself (circular dependency).
    CircularReference,
    /// A `TypeVar` used in the value is not declared in `type_params`.
    UndeclaredTypeVar {
        /// The name of the undeclared type variable.
        typevar_name: String,
    },
    /// The `type_params` keyword is not a literal tuple.
    NonLiteralTypeParams,
    /// Accessing an attribute that doesn't exist on `TypeAliasType` instances.
    InvalidAttributeAccess {
        /// The attribute name that was accessed.
        attr_name: String,
    },
    /// Incorrect number of type arguments when subscripting a type alias.
    IncorrectTypeArgCount {
        /// Expected (minimum) number of type arguments.
        expected: usize,
        /// Actual number provided.
        actual: usize,
    },
}
