//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Module-level types: call sites, type definitions, comparisons, and misc info.

use super::{rhs::RhsKind, span::Span};

/// A reference to an unhashable expression used as a dict key.
#[derive(Debug, Clone, PartialEq)]
pub struct UnhashableKeyRef {
    /// The span of the unhashable key expression.
    pub span: Span,
    /// A human-readable description of the key type (`"list"`, `"set"`, `"dict"`).
    pub key_type: &'static str,
}

/// A `ClassName(args).__hash__()` call on a non-hashable dataclass.
///
/// A `@dataclass` with `eq=True` (the default) sets `__hash__` to `None`
/// unless `frozen=True`, `unsafe_hash=True`, or the class defines `__hash__`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnhashableHashCallViolation {
    /// The class name that is not hashable.
    pub class_name: String,
    /// The span of the entire `.__hash__()` call expression.
    pub span: Span,
}

/// A call site detected in module-level code.
#[derive(Debug, Clone, PartialEq)]
pub struct CallSite {
    /// The name of the called function (simple name only; complex callees ignored).
    pub callee: String,
    /// Receiver of a supported bound method call, if this is `receiver.method(...)`.
    pub receiver: Option<CallReceiver>,
    /// Kinds and spans of positional arguments at the call site.
    pub args: Vec<(RhsKind, Span)>,
    /// Keyword arguments at the call site: `(name, rhs_kind)` pairs.
    ///
    /// For `func(a=1, b="x")`, this is `[("a", IntLiteral), ("b", StrLiteral)]`.
    /// Only populated for keyword arguments with an explicit name (`arg=val`).
    /// Star-unpacked kwargs (`**kw`) are not included.
    pub keywords: Vec<(String, RhsKind)>,
    /// `true` when the call contains any `**kwargs` unpacking (e.g. `func(**d)`).
    /// Unpacked kwargs are not listed in `keywords`, so arity checks must treat
    /// their presence as "argument count unknown" to avoid false positives.
    pub has_unpacked_kwargs: bool,
    /// The span of the entire call expression.
    pub span: Span,
}

/// Receiver shapes whose built-in type is statically knowable at a call site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallReceiver {
    /// A string literal, which also satisfies `LiteralString` receivers.
    StringLiteral,
    /// A bytes literal.
    BytesLiteral,
    /// A named variable or parameter whose annotation/inferred type is resolved later.
    Name(String),
    /// A direct constructor call on a named callee (`C().method(...)`): the
    /// receiver is a fresh *instance* of `C`, so instance-method binding
    /// consumes the implicit `self` parameter
    /// ([#382](https://github.com/Nimblesite/Basilisk/issues/382)).
    Constructor(String),
}

/// A `NamedTuple` definition collected from module-level code.
///
/// Covers calls of the form `N = NamedTuple("N", [(field1, type1), ...])`.
/// Field names are resolved by substituting `Final` string-literal constants.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedTupleDefInfo {
    /// The name the result is bound to (LHS of the assignment).
    pub lhs_name: String,
    /// Field names in declaration order.
    ///
    /// Each name is either a literal string from the tuple list, or a `Final`
    /// string constant resolved at resolver time.
    pub field_names: Vec<String>,
    /// Field type texts in declaration order (parallel to `field_names`).
    ///
    /// Contains the source text of each type expression (e.g. `"int"`, `"str"`).
    /// Empty when `has_types` is `false` (i.e., for `collections.namedtuple`).
    pub field_types: Vec<String>,
    /// Number of trailing fields that have default values.
    ///
    /// Set from the `defaults` keyword argument, e.g. `namedtuple("N", "a b c", defaults=(1, 2))`
    /// yields `defaults_count = 2` (fields `b` and `c` have defaults).
    pub defaults_count: usize,
    /// `true` when field type information is available (i.e., `typing.NamedTuple`).
    /// `false` for `collections.namedtuple` where no type information is given.
    pub has_types: bool,
    /// Span of the entire `NamedTuple(...)` or `namedtuple(...)` call expression.
    pub span: Span,
}

/// Information about a module-level `TypeVar(...)` call.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "TypeVar has many boolean constraint flags"
)]
pub struct TypeVarCallInfo {
    /// The name the `TypeVar` is bound to (LHS of assignment).
    pub name: String,
    /// Number of positional constraint arguments (excludes the name string).
    pub constraint_count: usize,
    /// Whether a `default=` keyword argument is present (PEP 696).
    pub has_default: bool,
    /// Whether a `bound=` keyword argument is present.
    pub has_bound: bool,
    /// Whether any constraint argument is parameterized by a `TypeVar`
    /// (e.g. `TypeVar("T", str, list[T])` — constraint `list[T]` contains a `TypeVar`).
    pub has_parameterized_constraint: bool,
    /// Whether the `bound=` expression is itself parameterized by a `TypeVar`
    /// (e.g. `TypeVar("T", bound=list[T])` — bound `list[T]` contains a `TypeVar`).
    pub has_parameterized_bound: bool,
    /// Whether `covariant=True` keyword argument is present.
    pub is_covariant: bool,
    /// Whether `contravariant=True` keyword argument is present.
    pub is_contravariant: bool,
    /// Whether `infer_variance=True` keyword argument is present.
    pub has_infer_variance: bool,
    /// The span of the entire `TypeVar` call expression.
    pub span: Span,
    /// Simple type name from the `bound=` keyword argument (e.g. `"str"` from `bound=str`).
    /// `None` if not present or not a simple name.
    pub bound_type_name: Option<String>,
    /// Simple type name from the `default=` keyword argument (e.g. `"int"` from `default=int`).
    /// `None` if not present or not a simple name.
    pub default_type_name: Option<String>,
    /// Type names from positional constraint arguments (excluding the `TypeVar` name string arg).
    /// Empty when there are no constraints.
    pub constraint_type_names: Vec<String>,
    /// `true` when this is a `TypeVarTuple(...)` call rather than `TypeVar(...)`.
    pub is_typevartuple: bool,
    /// `true` when this is a `ParamSpec(...)` call rather than `TypeVar(...)`.
    pub is_paramspec: bool,
    /// The string value of the first positional argument (the name string passed to the call).
    ///
    /// For `T = TypeVar("T")`, this is `Some("T")`.
    /// `None` when the first argument is not a plain string literal.
    pub string_name: Option<String>,
}

/// A `match` statement with exhaustiveness information.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchStmtInfo {
    /// The span of the `match` keyword.
    pub span: Span,
    /// `true` when at least one case is irrefutable — a bare `case _:` or a bare
    /// capture `case name:` (without a guard) — making the match exhaustive.
    pub has_wildcard: bool,
    /// `true` when at least one case decomposes structurally (a sequence or
    /// mapping pattern). Exhaustiveness checking does not apply to these.
    pub has_structural_pattern: bool,
}

/// A `reveal_type(...)` call found anywhere in the module.
#[derive(Debug, Clone, PartialEq)]
pub struct RevealTypeCallInfo {
    /// Number of positional arguments passed to `reveal_type`.
    pub arg_count: usize,
    /// The span of the entire `reveal_type(...)` call expression.
    pub span: Span,
}

/// An `assert_type(value, ExpectedType)` call found anywhere in the module.
#[derive(Debug, Clone, PartialEq)]
pub struct AssertTypeCallInfo {
    /// Number of positional arguments passed to `assert_type`.
    pub arg_count: usize,
    /// The span of the entire `assert_type(...)` call expression.
    pub span: Span,
    /// The normalized type text of the actual first argument.
    ///
    /// - For a parameter reference, this is the parameter annotation text (normalized).
    /// - For a literal, this is the inferred literal type (e.g. `"str"` for `""`).
    /// - `None` when the type cannot be determined statically.
    pub actual_type: Option<String>,
    /// The normalized type text of the second argument (the expected/declared type).
    ///
    /// `None` when there is no second argument (arity error) or the text cannot be extracted.
    pub expected_type: Option<String>,
    /// `true` when `actual_type` and `expected_type` are both known and do not match.
    pub type_mismatch: bool,
}

/// What kind of second argument was passed to a `TypedDict(...)` functional call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedDictSecondArgKind {
    /// A dict literal `{...}` was passed.
    DictLiteral,
    /// Something other than a dict literal was passed (e.g. a variable reference).
    NotDictLiteral,
}

/// Information about a module-level `TypedDict(...)` functional-syntax call.
///
/// Covers calls of the form `Name = TypedDict("Name", {...})`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedDictCallInfo {
    /// The name the result is bound to (LHS of the assignment).
    pub lhs_name: String,
    /// The first positional argument — the declared name string, if it is a string literal.
    pub declared_name: Option<String>,
    /// Whether the second positional argument is a dict literal or something else.
    pub second_arg_kind: TypedDictSecondArgKind,
    /// Whether any key in the second-arg dict literal is a non-string literal.
    pub has_non_string_key: bool,
    /// Whether there is actually a second positional argument (as opposed to keyword-only form).
    pub has_positional_dict: bool,
    /// Keyword argument names in the call (after the positional args).
    pub keyword_names: Vec<String>,
    /// The span of the entire `TypedDict(...)` call expression.
    pub span: Span,
}

/// Information about a module-level `NewType(...)` call.
///
/// Covers assignments of the form `Name = NewType("Name", BaseType)`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewTypeCallInfo {
    /// The name the result is bound to (LHS of the assignment).
    pub lhs_name: String,
    /// The name string passed as the first argument, if it is a string literal.
    pub declared_name: Option<String>,
    /// Number of positional arguments to `NewType(...)`.
    pub positional_arg_count: usize,
    /// The span of the second positional argument (the base type expression), if present.
    pub base_type_span: Option<Span>,
    /// The span of the entire `NewType(...)` call expression.
    pub span: Span,
}

/// A module-level bare assignment (`name = expr`) that may re-assign a `Final`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleBareAssignment {
    /// The simple name being assigned.
    pub name: String,
    /// Span of the target name token.
    pub name_span: Span,
}

/// A module-level attribute assignment (`Class.attr = expr`).
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleAttrAssignment {
    /// The class/object name on the left of the dot.
    pub object_name: String,
    /// The attribute name on the right of the dot.
    pub attr_name: String,
    /// Span of the entire `Class.attr` target expression.
    pub target_span: Span,
    /// Span of the right-hand-side value expression, if present.
    pub rhs_span: Option<Span>,
}

/// A module-level attribute access expression (`Name.attr` as a standalone statement).
///
/// Used to detect reads of attributes that may not be generated (e.g. `DC.__match_args__`
/// on a dataclass with `match_args=False`).
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleAttrAccessInfo {
    /// The object/class name on the left of the dot.
    pub object_name: String,
    /// The attribute name being accessed.
    pub attr_name: String,
    /// Span of the entire `Name.attr` expression.
    pub span: Span,
}

/// Comparison operators used in ordering comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareOp {
    /// `<`
    Lt,
    /// `<=`
    LtE,
    /// `>`
    Gt,
    /// `>=`
    GtE,
}

/// A module-level comparison between two simple names using an ordering operator.
///
/// Used to detect cross-type ordering comparisons of `order=True` dataclass instances.
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleOrderComparisonInfo {
    /// Name of the left operand.
    pub left_name: String,
    /// Name of the right operand.
    pub right_name: String,
    /// The comparison operator used.
    pub op: CompareOp,
    /// Span of the entire comparison expression.
    pub span: Span,
}

/// Information about a `TypeAliasType(name, rhs, ...)` call.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasTypeCallInfo {
    /// The LHS variable name.
    pub lhs_name: String,
    /// Span of the second argument (the type expression / RHS).
    pub rhs_span: Option<Span>,
    /// Span of the entire call.
    pub span: Span,
}

/// Information about a PEP 695 `type X = rhs` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeStatementInfo {
    /// The alias name (`X` in `type X = rhs`).
    pub name: String,
    /// Span of the RHS value expression.
    pub rhs_span: Span,
    /// Span of the name token.
    pub name_span: Span,
    /// The statement's own type-parameter names (`T` in `type X[T] = rhs`).
    /// PEP 695 binds these in the alias's annotation scope, shadowing any
    /// module-level binding of the same name inside the RHS.
    pub param_names: Vec<String>,
}

/// Information about an `Annotated[...]` subscription with too few arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotatedTooFewArgs {
    /// Span of the subscript expression.
    pub span: Span,
}

/// An attribute access on a `float`-typed function parameter using an `int`-only attribute.
///
/// An annotated assignment in a function body where the declared `Literal` type uses a
/// quoted string that looks like an enum member (e.g. `"Color.RED"`) but the RHS is
/// a parameter typed as the actual enum member literal (e.g. `Literal[Color.RED]`).
///
/// ```python
/// def func2(a: Literal[Color.RED]):
///     x1: Literal["Color.RED"] = a  # E — string ≠ enum member
/// ```
///
/// Used by `enums_member_values`.
#[derive(Debug, Clone, PartialEq)]
pub struct LiteralStringEnumMismatch {
    /// The variable name being assigned (e.g. `"x1"`).
    pub var_name: String,
    /// The annotation text as written (e.g. `Literal["Color.RED"]`).
    pub annotation: String,
    /// The enum-member form extracted from the annotation (e.g. `Color.RED`).
    pub enum_form: String,
    /// Span of the variable name on the LHS of the assignment.
    pub span: Span,
}

/// For example, `f.numerator` where `f: float` is invalid because `float` does not have
/// `.numerator` — that is an `int`-only attribute.  This is detected only at the
/// top level of a function body (not inside `if`/`for`/`while`/`match` blocks), so
/// that `isinstance`-guarded branches (where `f` has been narrowed to `int`) are
/// excluded.
///
/// Used by `specialtypes_promotions`.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatParamIntAttrAccess {
    /// The name of the parameter (e.g. `"f"`).
    pub param_name: String,
    /// The int-only attribute being accessed (e.g. `"numerator"`).
    pub attr_name: String,
    /// Span of the entire attribute access expression (e.g. `f.numerator`).
    pub span: Span,
}

/// A module-level `TypeAlias` annotated assignment.
///
/// Represents `MyAlias: TypeAlias = SomeGeneric[int, T]` at module level.
/// The `rhs_names` field contains all simple names referenced in the RHS expression.
/// Used by `generics_defaults_specialization` to check that subscript sites respect the alias arity.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAliasDefInfo {
    /// The alias name (e.g. `"MyAlias"`).
    pub name: String,
    /// All simple names referenced in the RHS expression (includes both `TypeVar`s and non-`TypeVar`s).
    pub rhs_names: Vec<String>,
    /// The base name of the RHS expression, if it is a subscript.
    pub rhs_base_name: Option<String>,
    /// Type argument names from the RHS subscript expression.
    pub rhs_type_arg_names: Vec<String>,
    /// Forward-reference strings found in the RHS expression.
    pub rhs_string_refs: Vec<String>,
    /// The source span of the type alias definition.
    pub span: Span,
}

/// A module-level subscript expression (e.g. `MyGeneric[int]`) used as a statement.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericSubscriptSite {
    /// The name of the subscripted type (e.g. `"MyGeneric"`).
    pub base_name: String,
    /// Number of type arguments supplied.
    pub arg_count: usize,
    /// Span of the subscript expression.
    pub span: Span,
}

/// A `TypedDict` key/value violation detected during resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedDictKeyViolation {
    /// The span of the offending expression.
    pub span: Span,
    /// The name of the `TypedDict` class.
    pub class_name: String,
    /// The kind of violation.
    pub kind: TypedDictKeyViolationKind,
}

/// Kind of `TypedDict` key/value violation.
#[derive(Debug, Clone, PartialEq)]
pub enum TypedDictKeyViolationKind {
    /// Subscript assignment with an invalid key: `td["invalid_key"] = val`.
    InvalidSubscriptKey {
        /// The invalid key name.
        key: String,
    },
    /// Subscript assignment where the value type mismatches the declared field type.
    WrongSubscriptValueType {
        /// The field being assigned.
        key: String,
        /// The declared field type annotation text.
        expected: String,
    },
    /// Annotated assignment with a dict literal containing invalid or missing keys.
    InvalidDictLiteral {
        /// Keys in the dict that are not in the `TypedDict` schema.
        invalid_keys: Vec<String>,
        /// Required `TypedDict` fields missing from the dict literal.
        missing_keys: Vec<String>,
    },
    /// Subscript read access with a key that is not a valid `TypedDict` field.
    SubscriptReadInvalidKey {
        /// The invalid key name.
        key: String,
    },
    /// Dict literal used for a `TypedDict` variable contains a non-literal (variable) key.
    NonLiteralDictKey,
    /// A call to a method that is disallowed on `TypedDict` instances (e.g. `.clear()`).
    DisallowedMethodCall {
        /// The method name.
        method: String,
    },
    /// A `del` statement on a `TypedDict` subscript.
    DeleteSubscript,
}

/// A forward-reference string found in a type alias RHS.
#[derive(Debug, Clone)]
pub struct RhsStringRef {
    /// The referenced name.
    pub name: String,
    /// The source span of the string reference.
    pub span: Span,
}
