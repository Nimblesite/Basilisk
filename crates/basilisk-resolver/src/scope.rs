//! Scope and function information types produced by the resolver.

/// A byte-offset span within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the start (inclusive).
    pub start: u32,
    /// Byte offset of the end (exclusive).
    pub end: u32,
}

impl Span {
    /// Slice `source` using this span without `as` conversions.
    ///
    /// Returns `None` if the span is out of bounds.
    #[must_use]
    pub fn slice_source<'a>(&self, source: &'a str) -> Option<&'a str> {
        source.get(self.as_range())
    }

    /// Convert start offset to `usize`.
    ///
    /// Safe because u32 fits in usize on all supported (32-bit+) targets.
    #[must_use]
    #[expect(clippy::as_conversions, reason = "u32 to usize is safe on 32-bit+ targets")]
    pub const fn start_usize(&self) -> usize {
        self.start as usize
    }

    /// Convert end offset to `usize`.
    ///
    /// Safe because u32 fits in usize on all supported (32-bit+) targets.
    #[must_use]
    #[expect(clippy::as_conversions, reason = "u32 to usize is safe on 32-bit+ targets")]
    pub const fn end_usize(&self) -> usize {
        self.end as usize
    }

    /// Convert this span to a `Range<usize>` for slicing.
    ///
    /// Safe because u32 fits in usize on all supported (32-bit+) targets.
    #[must_use]
    #[expect(clippy::as_conversions, reason = "u32 to usize is safe on 32-bit+ targets")]
    pub const fn as_range(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

/// Information about a single function parameter.
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Python parameters have many boolean flags"
)]
pub struct ParameterInfo {
    /// The parameter name as it appears in source.
    pub name: String,
    /// `true` when a type annotation is present (`param: Type`).
    pub has_annotation: bool,
    /// `true` when the annotation is explicitly `Any` (from typing).
    pub annotation_is_any: bool,
    /// `true` when the annotation is a numeric or boolean literal (invalid type form).
    pub annotation_is_numeric_literal: bool,
    /// `true` when the parameter has a default value (`param = default`).
    pub has_default: bool,
    /// The source span of the parameter name token.
    pub name_span: Span,
    /// The source span of the annotation expression, if present.
    pub annotation_span: Option<Span>,
    /// The raw annotation text (e.g. `"int"`, `"str | None"`), if annotated.
    pub annotation_text: Option<String>,
}

/// How a return annotation is classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnAnnotationKind {
    /// No return annotation present.
    Missing,
    /// Return annotation is explicitly `Any`.
    Any,
    /// Return annotation is `None` or `-> None`.
    NoneType,
    /// Return annotation is a numeric or boolean literal (invalid type form).
    NumericLiteral,
    /// Return annotation is some other valid type expression.
    Other,
}

impl ReturnAnnotationKind {
    /// Returns `true` when a return annotation is present (not `Missing`).
    #[must_use]
    pub fn is_present(&self) -> bool {
        !matches!(self, Self::Missing)
    }
}

/// Information about a single function definition.
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Python functions have many boolean flags"
)]
pub struct FunctionInfo {
    /// The function name.
    pub name: String,
    /// All positional-or-keyword and keyword-only parameters.
    pub parameters: Vec<ParameterInfo>,
    /// The `*args` parameter, if present.
    pub vararg: Option<ParameterInfo>,
    /// The `**kwargs` parameter, if present.
    pub kwarg: Option<ParameterInfo>,
    /// How the return annotation is classified (or absent).
    pub return_annotation: ReturnAnnotationKind,
    /// Decorator names applied to this function (e.g. `"overload"`, `"override"`).
    pub decorators: Vec<String>,
    /// Decorator name spans paired with their names (for semantic token highlighting).
    pub decorator_spans: Vec<(String, Span)>,
    /// Return statements found in this function body.
    pub return_stmts: Vec<ReturnStmtInfo>,
    /// The span of the `def` keyword (start of the function definition).
    pub def_span: Span,
    /// The span of the function name identifier.
    pub name_span: Span,
    /// The span of the return annotation expression, if present.
    pub return_annotation_span: Option<Span>,
    /// Name of the containing class, if this function is a method.
    pub class_name: Option<String>,
    /// All names assigned anywhere in the function body (for scope analysis).
    pub all_local_assigns: Vec<String>,
    /// Names assigned at the top level of the function body (unconditionally).
    pub unconditional_assigns: Vec<String>,
    /// Names referenced directly in `return` expressions (simple `return name`).
    pub return_name_refs: Vec<(String, Span)>,
    /// Names referenced in top-level (unconditional) `return` expressions only.
    ///
    /// Unlike `return_name_refs`, this excludes returns nested inside `if`/`for`/
    /// `while`/`try`/`with` blocks.  Used by E0019 to avoid false positives where
    /// a `return name` is inside the same branch that assigned `name`.
    pub top_level_return_name_refs: Vec<(String, Span)>,
    /// Unhashable expressions used as dict keys in the function body.
    pub unhashable_keys: Vec<UnhashableKeyRef>,
    /// `true` when the entire function body is a stub (only `...` or `pass`).
    ///
    /// Stub bodies are exempt from E0001/E0002/E0004: they appear in overload
    /// signatures, Protocol bodies used as stubs, and `.pyi`-style inline stubs.
    pub is_stub_body: bool,
    /// `true` when the last top-level statement in the function body unconditionally
    /// terminates: either a `raise` statement or a standalone call expression (which
    /// may be a call to a `NoReturn` function).
    ///
    /// Used to detect `-> NoReturn`/`-> Never` functions that can fall through.
    pub body_last_stmt_terminates: bool,
    /// `true` when the function uses PEP 695 type parameter syntax (`def foo[T](): ...`).
    pub has_pep695_type_params: bool,
    /// Names of the PEP 695 type parameters declared in `[...]` for this function.
    ///
    /// For `def foo[T, *Ts, **P](): ...`, this is `["T", "Ts", "P"]`.
    pub pep695_type_param_names: Vec<String>,
    /// Annotated local variables declared anywhere in the function body
    /// (excluding nested function bodies).
    ///
    /// For `x: int = 0` inside the function, this contains a `VariableInfo`
    /// with `has_annotation = true`.  Used by E0047 to check for invalid type
    /// annotations in local variable declarations.
    pub local_vars: Vec<VariableInfo>,
    /// `true` when the function body contains at least one `yield` or `yield from`.
    pub is_generator: bool,
    /// `true` when the function is declared with `async def`.
    pub is_async: bool,
    /// Yield expressions found in this function body.
    pub yield_exprs: Vec<YieldExprInfo>,
    /// `true` when the last top-level statement in the function body is a `return`
    /// statement (with or without a value).
    ///
    /// Used by E0120 to detect generators with `Generator[Y, S, R]` where R is not
    /// `None` but the function can fall through without returning.
    pub body_ends_with_return: bool,
    /// The docstring of this function, if present (first statement is a string literal).
    pub docstring: Option<String>,
}

/// A `return` statement found inside a function body.
#[derive(Debug, Clone)]
pub struct ReturnStmtInfo {
    /// The span of the `return` keyword.
    pub span: Span,
    /// `true` when the statement has a non-`None` expression (`return expr`).
    pub has_value: bool,
    /// `true` when the returned expression is a function/method call.
    ///
    /// Call expressions may return `None` (e.g. `return f(self)` where `f:
    /// Callable[..., None]`).  Without full type inference we cannot verify
    /// the callee's return type, so E0013 conservatively skips these.
    pub value_is_call: bool,
    /// What kind of expression is returned, if any.
    ///
    /// Used for return type inference in E0002.
    pub rhs_kind: RhsKind,
}

/// A `yield` or `yield from` expression found inside a generator function body.
#[derive(Debug, Clone)]
pub struct YieldExprInfo {
    /// The span of the `yield` keyword.
    pub span: Span,
    /// What kind of expression is yielded, if any.
    pub rhs_kind: RhsKind,
    /// `true` when this is a `yield from` expression.
    pub is_yield_from: bool,
    /// The name of the called function/constructor, if the yield value is a call expression.
    /// For `yield SomeClass()`, this is `Some("SomeClass")`.
    pub call_name: Option<String>,
}

/// A reference to an unhashable expression used as a dict key.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct UnhashableHashCallViolation {
    /// The class name that is not hashable.
    pub class_name: String,
    /// The span of the entire `.__hash__()` call expression.
    pub span: Span,
}

/// A call site detected in module-level code.
#[derive(Debug, Clone)]
pub struct CallSite {
    /// The name of the called function (simple name only; complex callees ignored).
    pub callee: String,
    /// Kinds and spans of positional arguments at the call site.
    pub args: Vec<(RhsKind, Span)>,
    /// Keyword arguments at the call site: `(name, rhs_kind)` pairs.
    ///
    /// For `func(a=1, b="x")`, this is `[("a", IntLiteral), ("b", StrLiteral)]`.
    /// Only populated for keyword arguments with an explicit name (`arg=val`).
    /// Star-unpacked kwargs (`**kw`) are not included.
    pub keywords: Vec<(String, RhsKind)>,
    /// The span of the entire call expression.
    pub span: Span,
}

/// A `NamedTuple` definition collected from module-level code.
///
/// Covers calls of the form `N = NamedTuple("N", [(field1, type1), ...])`.
/// Field names are resolved by substituting `Final` string-literal constants.
#[derive(Debug, Clone)]
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
    /// Used by `BSK-E0056` to detect mutation of read-only `TypedDict` fields.
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
}

/// Information about an enum `_value_` type mismatch detected during resolution.
///
/// Populated by the resolver visitor; used by `BSK-E0063` to emit diagnostics
/// without re-walking the AST.
#[derive(Debug, Clone)]
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

/// A class definition with its attributes and method names.
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Python classes have many boolean flags"
)]
pub struct ClassInfo {
    /// The class name.
    pub name: String,
    /// The span of the class name token.
    pub name_span: Span,
    /// The span of the `class` keyword.
    pub def_span: Span,
    /// Base class names (simple names only; complex expressions ignored).
    pub bases: Vec<String>,
    /// Attributes declared directly in the class body.
    pub attributes: Vec<AttributeInfo>,
    /// Names of methods defined in the class body.
    pub method_names: Vec<String>,
    /// Decorators for each method: `(method_name, [decorator_names])`.
    pub method_decorators: Vec<(String, Vec<String>)>,
    /// Class-level decorator name spans (for semantic token highlighting).
    pub decorator_spans: Vec<(String, Span)>,
    /// Type parameter names extracted from a `Generic[...]` base, if present.
    pub generic_params: Vec<GenericParamInfo>,
    /// `true` when the class inherits from `TypedDict`.
    pub is_typed_dict: bool,
    /// `true` when the `TypedDict` has `total=True` (the default). `false` when `total=False`.
    pub is_typeddict_total: bool,
    /// Keyword argument names in the class definition (e.g. `metaclass`, `total`, `other`).
    pub class_keywords: Vec<String>,
    /// `true` when the class is decorated with `@dataclass` or `@dataclass(...)`.
    pub is_dataclass: bool,
    /// `true` when the dataclass is decorated with `frozen=True`.
    pub is_dataclass_frozen: bool,
    /// `true` when the dataclass is decorated with `kw_only=True`.
    ///
    /// When `kw_only=True`, all fields are keyword-only in `__init__`
    /// unless individually overridden with `field(kw_only=False)`.
    pub is_dataclass_kw_only: bool,
    /// `true` when the dataclass is decorated with `match_args=False`
    /// (suppresses `__match_args__` generation).
    pub is_dataclass_match_args_false: bool,
    /// `true` when the dataclass is decorated with `order=True`
    /// (synthesizes ordering comparison methods).
    pub is_dataclass_order: bool,
    /// `true` when the dataclass is decorated with `unsafe_hash=True`.
    ///
    /// `unsafe_hash=True` forces generation of `__hash__` even when `eq=True`
    /// and the class is not frozen.
    pub is_dataclass_unsafe_hash: bool,
    /// `true` when the dataclass is decorated with `eq=False`.
    ///
    /// When `eq=False`, `__hash__` is not touched by the dataclass machinery,
    /// so the class retains the inherited `__hash__` from `object`.
    pub is_dataclass_eq_false: bool,
    /// `true` when the dataclass is decorated with `init=False`.
    ///
    /// When `init=False`, no `__init__` is synthesized.  If the class also
    /// defines no explicit `__init__`, calling it with arguments is an error.
    pub is_dataclass_init_false: bool,
    /// `true` when the class is decorated with `@final` or `typing.final`.
    pub is_final: bool,
    /// `true` when the class directly or transitively inherits from an `Enum` family class.
    pub is_enum: bool,
    /// `true` when the class uses PEP 695 type parameter syntax (`class Foo[T]: ...`).
    pub has_pep695_type_params: bool,
    /// Names of the PEP 695 type parameters declared in `[...]` for this class.
    ///
    /// For `class Foo[T, *Ts, **P]: ...`, this is `["T", "Ts", "P"]`.
    pub pep695_type_param_names: Vec<String>,
    /// All simple names referenced inside base class expressions (including subscript arguments).
    ///
    /// For `class Foo[V](dict[K, V])`, this would include `"dict"`, `"K"`, and `"V"`.
    /// Used by E0042 to detect traditional `TypeVars` mixed into PEP 695 classes.
    pub base_expression_names: Vec<String>,
    /// Spans of arguments in `Generic[...]` or `Protocol[...]` that are NOT simple names
    /// (i.e. not plain `TypeVar` references, but literals, subscripts, etc.).
    ///
    /// For `class Foo(Generic[int])`, this would contain the span of `int`.
    /// Used by E0043 to detect non-TypeVar arguments to Generic/Protocol.
    pub generic_non_typevar_args: Vec<Span>,
    /// The metaclass name if specified via `metaclass=Meta` keyword.
    ///
    /// For `class Foo(metaclass=Meta): ...`, this is `Some("Meta")`.
    /// `None` when no explicit metaclass is specified.
    pub metaclass_name: Option<String>,
    /// `true` when at least one base class expression is a subscript.
    ///
    /// For example, `class Foo(SubclassMe[float])` has a subscript base.
    /// Used by `BSK-E0092` to detect classes that have fully specialised their
    /// generic bases and therefore cannot be further subscripted.
    pub has_subscript_base: bool,
    /// Structured information about subscripted base classes.
    ///
    /// For `class Foo(Base[T, int])`, this contains an entry with `base_name = "Base"`.
    /// Used by `BSK-E0107` for variance checking.
    pub base_subscripts: Vec<BaseSubscriptEntry>,
    /// `true` when the dataclass is decorated with `slots=True`.
    pub is_dataclass_slots: bool,
    /// `true` when the class has a manual `__slots__` definition in the body.
    pub has_manual_slots: bool,
    /// The docstring of this class, if present (first statement is a string literal).
    pub docstring: Option<String>,
}

/// Type parameters declared in a `Generic[T1, T2, ...]` base expression.
#[derive(Debug, Clone)]
pub struct GenericParamInfo {
    /// The name of the type parameter (e.g. `"T"`, `"T_co"`).
    pub name: String,
    /// The source span of this parameter name inside `Generic[...]`.
    pub span: Span,
    /// `true` when this param was extracted from a starred expression (`*Ts`),
    /// indicating it is a `TypeVarTuple` unpack in `Generic[...]`.
    pub is_typevartuple: bool,
}

/// A module-level `TypeAlias` annotated assignment.
///
/// Represents `MyAlias: TypeAlias = SomeGeneric[int, T]` at module level.
/// The `rhs_names` field contains all simple names referenced in the RHS expression.
/// Used by `BSK-E0092` to check that subscript sites respect the alias arity.
#[derive(Debug, Clone)]
pub struct TypeAliasDefInfo {
    /// The alias name (e.g. `"MyAlias"`).
    pub name: String,
    /// All simple names referenced in the RHS expression (includes both `TypeVar`s and non-`TypeVar`s).
    pub rhs_names: Vec<String>,
    /// The base name of the RHS expression, if it is a subscript (e.g. `"Generic"` from `Generic[T]`).
    pub rhs_base_name: Option<String>,
    /// Type argument names from the RHS subscript expression.
    pub rhs_type_arg_names: Vec<String>,
    /// Forward-reference strings found in the RHS expression.
    pub rhs_string_refs: Vec<String>,
    /// The source span of the type alias definition.
    pub span: Span,
}

/// A module-level subscript expression (e.g. `MyGeneric[int]`) used as a statement.
#[derive(Debug, Clone)]
pub struct GenericSubscriptSite {
    /// The name of the subscripted type (e.g. `"MyGeneric"`).
    pub base_name: String,
    /// Number of type arguments supplied.
    pub arg_count: usize,
    /// Span of the subscript expression.
    pub span: Span,
}

/// Information about a module-level `TypeVar(...)` call.
#[derive(Debug, Clone)]
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

/// A `match` statement with exhaustiveness information.
#[derive(Debug, Clone)]
pub struct MatchStmtInfo {
    /// The span of the `match` keyword.
    pub span: Span,
    /// `true` when at least one case uses a wildcard pattern (`case _:`).
    pub has_wildcard: bool,
}

/// A `reveal_type(...)` call found anywhere in the module.
#[derive(Debug, Clone)]
pub struct RevealTypeCallInfo {
    /// Number of positional arguments passed to `reveal_type`.
    pub arg_count: usize,
    /// The span of the entire `reveal_type(...)` call expression.
    pub span: Span,
}

/// An `assert_type(value, ExpectedType)` call found anywhere in the module.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

/// A violation of `Final` typing rules, collected during resolution.
///
/// These are gathered in the resolver so that the checker rule (`E0047`) can
/// emit them without duplicating AST-walking logic.
#[derive(Debug, Clone)]
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

/// A module-level bare assignment (`name = expr`) that may re-assign a `Final`.
#[derive(Debug, Clone)]
pub struct ModuleBareAssignment {
    /// The simple name being assigned.
    pub name: String,
    /// Span of the target name token.
    pub name_span: Span,
}

/// A module-level attribute assignment (`Class.attr = expr`).
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

/// A violation of `ReadOnly` `TypedDict` field mutation rules.
///
/// Covers module-level subscript assignment (`td["key"] = val`) and `.update()` calls
/// on `TypedDict` variables that have `ReadOnly` fields.
#[derive(Debug, Clone)]
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

/// A module-level comparison between two simple names using an ordering operator.
///
/// Used to detect cross-type ordering comparisons of `order=True` dataclass instances.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct TypeAliasTypeCallInfo {
    /// The LHS variable name.
    pub lhs_name: String,
    /// Span of the second argument (the type expression / RHS).
    pub rhs_span: Option<Span>,
    /// Span of the entire call.
    pub span: Span,
}

/// Information about a PEP 695 `type X = rhs` statement.
#[derive(Debug, Clone)]
pub struct TypeStatementInfo {
    /// The alias name (`X` in `type X = rhs`).
    pub name: String,
    /// Span of the RHS value expression.
    pub rhs_span: Span,
    /// Span of the name token.
    pub name_span: Span,
}

/// Information about an `Annotated[...]` subscription with too few arguments.
#[derive(Debug, Clone)]
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
/// Used by `BSK-E0066`.
#[derive(Debug, Clone)]
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

/// A `ClassVar` annotation used inside a function body (local variable or
/// self-attribute assignment) where it is not valid.
///
/// PEP 526 forbids `ClassVar` in function bodies, including:
/// - `x: ClassVar[str] = ""` — local variable annotation
/// - `self.xx: ClassVar[str] = ""` — attribute annotation on `self` in a method
///
/// Used by `BSK-E0036`.
#[derive(Debug, Clone)]
pub struct LocalClassVarViolation {
    /// The variable or attribute name being annotated.
    pub name: String,
    /// The source span of the name token.
    pub name_span: Span,
    /// Whether this is a self-attribute annotation (e.g. `self.xx: ClassVar[str]`).
    pub is_self_attr: bool,
}

/// For example, `f.numerator` where `f: float` is invalid because `float` does not have
/// `.numerator` — that is an `int`-only attribute.  This is detected only at the
/// top level of a function body (not inside `if`/`for`/`while`/`match` blocks), so
/// that `isinstance`-guarded branches (where `f` has been narrowed to `int`) are
/// excluded.
///
/// Used by `BSK-E0065`.
#[derive(Debug, Clone)]
pub struct FloatParamIntAttrAccess {
    /// The name of the parameter (e.g. `"f"`).
    pub param_name: String,
    /// The int-only attribute being accessed (e.g. `"numerator"`).
    pub attr_name: String,
    /// Span of the entire attribute access expression (e.g. `f.numerator`).
    pub span: Span,
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
/// Used by `BSK-E0073`.
#[derive(Debug, Clone)]
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

/// An invalid string annotation detected during AST resolution.
#[derive(Debug, Clone)]
pub struct InvalidStringAnnotation {
    /// The kind of invalidity.
    pub kind: InvalidStringAnnotationKind,
    /// The span of the annotation expression.
    pub span: Span,
}

/// A direct instantiation of a Protocol class or a concrete class that fails
/// to implement all required members of its Protocol base(s).
///
/// The typing spec forbids instantiating Protocol classes directly, and
/// concrete subclasses that do not implement all abstract/stub methods or
/// required `ClassVar` attributes are effectively abstract and cannot be
/// instantiated.
#[derive(Debug, Clone)]
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
/// Used by `BSK-E0114`.
#[derive(Debug, Clone)]
pub struct ProtocolRtcViolation {
    /// The span of the offending call expression.
    pub span: Span,
    /// The kind of violation.
    pub kind: ProtocolRtcViolationKind,
}

/// The kind of `@runtime_checkable` protocol violation.
#[derive(Debug, Clone)]
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
/// Used by `BSK-E0115`.
#[derive(Debug, Clone)]
pub struct GeneratorViolation {
    /// The span of the offending expression.
    pub span: Span,
    /// The kind of violation.
    pub kind: GeneratorViolationKind,
}

/// The kind of generator violation.
#[derive(Debug, Clone)]
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

/// The complete resolved view of a parsed module.
#[derive(Debug, Clone)]
pub struct ResolvedModule {
    /// All function definitions found at any nesting level.
    pub functions: Vec<FunctionInfo>,
    /// All class definitions found at module level.
    pub classes: Vec<ClassInfo>,
    /// All module-level variable assignments.
    pub module_vars: Vec<VariableInfo>,
    /// All import statements.
    pub imports: Vec<ImportInfo>,
    /// All match statements found at any nesting level.
    pub match_stmts: Vec<MatchStmtInfo>,
    /// Module-level call sites (calls appearing in module-level expressions or assignments).
    pub calls: Vec<CallSite>,
    /// Module-level `TypeVar(...)` call sites.
    pub typevar_calls: Vec<TypeVarCallInfo>,
    /// All `reveal_type(...)` call sites found anywhere in the module.
    pub reveal_type_calls: Vec<RevealTypeCallInfo>,
    /// All `assert_type(...)` call sites found anywhere in the module.
    pub assert_type_calls: Vec<AssertTypeCallInfo>,
    /// Module-level `TypedDict(...)` functional-syntax call sites.
    pub typeddict_calls: Vec<TypedDictCallInfo>,
    /// Module-level `NewType(...)` call sites.
    pub newtype_calls: Vec<NewTypeCallInfo>,
    /// Spans of type annotations that contain multiple unbounded tuple unpacks.
    ///
    /// A tuple type is invalid if it contains more than one unbounded component,
    /// e.g. `tuple[*tuple[str, ...], *tuple[int, ...]]` — two `*tuple[T, ...]` unpacks.
    pub multiple_unbounded_tuple_spans: Vec<Span>,
    /// `Final` violations detected during AST resolution.
    ///
    /// Populated by the resolver visitor so that `BSK-E0047` can emit them
    /// without re-walking the AST.
    pub final_violations: Vec<FinalViolationInfo>,
    /// Module-level bare assignments (`name = expr`) — used to detect re-assignments
    /// to `Final`-annotated module variables.
    pub module_bare_assignments: Vec<ModuleBareAssignment>,
    /// Module-level attribute assignments (`Class.attr = expr`).
    pub module_attr_assignments: Vec<ModuleAttrAssignment>,
    /// Module-level attribute accesses (`Name.attr` as a standalone expression statement).
    ///
    /// Used by `BSK-E0057` to detect reads of `__match_args__` on dataclasses where
    /// `match_args=False` was set.
    pub module_attr_accesses: Vec<ModuleAttrAccessInfo>,
    /// Module-level ordering comparisons (`a < b`, `a <= b`, etc.) between two simple names.
    ///
    /// Used by `BSK-E0058` to detect cross-type comparisons of `order=True` dataclass instances.
    pub module_order_comparisons: Vec<ModuleOrderComparisonInfo>,
    /// `ReadOnly` `TypedDict` field mutation violations detected at module level.
    ///
    /// Used by `BSK-E0056`.
    pub readonly_violations: Vec<ReadOnlyViolationInfo>,
    /// Spans of direct calls to `Annotated` (whether bare or parameterized).
    ///
    /// PEP 593 forbids calling `Annotated` as a callable:
    /// - `Annotated()` — bare call with no type argument
    /// - `Annotated[int, ""]()` — calling a parameterized `Annotated[...]` subscript
    ///
    /// Populated by the resolver; used by `BSK-E0045`.
    pub annotated_direct_call_spans: Vec<Span>,
    /// Names that were imported from other modules where they were declared `Final`.
    ///
    /// E.g. `from _qualifiers_final_annotation_1 import TEN` when `TEN: Final[int] = 10`
    /// in that module — `TEN` would appear here.  Any bare re-assignment to such a name
    /// in the current module is a violation.
    ///
    /// Populated lazily by the resolver when the imported module can be found.
    pub imported_final_names: std::collections::HashSet<String>,
    /// Module-level `TypeAliasType(...)` call sites.
    pub type_alias_type_calls: Vec<TypeAliasTypeCallInfo>,
    /// PEP 695 `type X = rhs` alias statements.
    pub type_statements: Vec<TypeStatementInfo>,
    /// `Annotated[...]` subscriptions with too few type arguments (< 2).
    pub annotated_too_few_args: Vec<AnnotatedTooFewArgs>,
    /// `NamedTuple(...)` definitions at module level.
    ///
    /// Each entry represents a `N = NamedTuple("N", [(field, type), ...])` call
    /// with field names resolved from Final string constants.
    /// Used by checker rules to validate `NamedTuple` call sites.
    pub namedtuple_defs: Vec<NamedTupleDefInfo>,
    /// Attribute accesses on `float`-typed parameters using `int`-only attributes.
    ///
    /// Only top-level accesses in function bodies are collected (not inside
    /// `if`/`for`/`while`/`match` blocks) so that `isinstance`-guarded uses are excluded.
    ///
    /// Used by `BSK-E0065`.
    pub float_param_int_attr_accesses: Vec<FloatParamIntAttrAccess>,
    /// Annotated local assignments where the declared type is `Literal["X.Y"]` (a string
    /// that resembles an enum member) but the RHS is a parameter typed as `Literal[X.Y]`
    /// (the actual enum member).  `Literal["Color.RED"]` ≠ `Literal[Color.RED]`.
    ///
    /// Used by `BSK-E0066`.
    pub literal_string_enum_mismatches: Vec<LiteralStringEnumMismatch>,
    /// Enum `_value_` type violations detected during AST resolution.
    ///
    /// Populated by the resolver visitor so that `BSK-E0063` can emit them
    /// without re-walking the AST.
    pub enum_value_type_violations: Vec<EnumValueTypeViolationInfo>,
    /// `ClassVar` annotations used in function-local variable or self-attribute
    /// positions, where they are forbidden by PEP 526.
    ///
    /// Populated by the resolver visitor; used by `BSK-E0036`.
    pub local_classvar_violations: Vec<LocalClassVarViolation>,
    /// PEP 695 type parameter bound violations detected during AST resolution.
    ///
    /// Covers invalid bound/constraint expressions in `class Foo[T: ...]` syntax.
    /// Used by `BSK-E0067`.
    pub pep695_bound_violations: Vec<Pep695BoundViolation>,
    /// Historical positional-only parameter violations.
    ///
    /// Covers the pre-PEP 570 `__`-prefix convention for positional-only parameters.
    /// Used by `BSK-E0068`.
    pub historical_positional_violations: Vec<HistoricalPositionalViolation>,
    /// Invalid string annotations detected during AST resolution.
    ///
    /// String annotations that contain non-type expressions (e.g. list literals,
    /// lambda calls, conditional expressions, etc.).
    /// Used by `BSK-E0069`.
    pub invalid_string_annotations: Vec<InvalidStringAnnotation>,
    /// Protocol `Self`-return conformance violations detected during resolution.
    ///
    /// When a class is passed where a `Protocol` with `Self`-returning methods
    /// is expected, but the class's corresponding method returns a different type.
    /// Used by `BSK-E0073`.
    pub protocol_self_violations: Vec<ProtocolSelfViolation>,
    /// Protocol instantiation violations: direct `Proto()` calls or instantiation
    /// of concrete subclasses that fail to implement all required protocol members.
    ///
    /// Used by `BSK-E0099`.
    pub protocol_instantiation_violations: Vec<ProtocolInstantiationViolation>,
    /// Spans of `isinstance(x, T)` calls where `T` is a `TypedDict` class.
    ///
    /// PEP 589: `TypedDict` type objects cannot be used in `isinstance()` tests.
    /// Used by `BSK-E0088`.
    pub isinstance_typeddict_violations: Vec<Span>,
    /// `TypedDict` subscript key/value violations and invalid dict-literal assignments.
    ///
    /// Covers:
    /// - `td["invalid_key"] = val` where `"invalid_key"` is not a `TypedDict` field.
    /// - `td["field"] = wrong_type_val` where the value type mismatches the field type.
    /// - `var: TypedDict = {invalid/missing keys}` where the literal doesn't match the schema.
    ///   Used by `BSK-E0089`.
    pub typeddict_key_violations: Vec<TypedDictKeyViolation>,
    /// Module-level `TypeAlias` annotated assignments.
    ///
    /// Each entry represents `Name: TypeAlias = expr` at module level.
    /// Used by `BSK-E0092` to check that subscript sites respect the alias arity.
    pub type_alias_defs: Vec<TypeAliasDefInfo>,
    /// Module-level subscript expression sites (`Name[args...]` used as a statement).
    ///
    /// Collected so that rules can check whether user-defined generic types are
    /// subscripted with the correct number of type arguments.
    /// Used by `BSK-E0092`.
    pub generic_subscript_sites: Vec<GenericSubscriptSite>,
    /// Augmented-assignment violations on `Literal`-typed variables.
    ///
    /// Used by `BSK-E0100`.
    pub literal_augmented_assign_violations: Vec<LiteralAugmentedAssignViolation>,
    /// Tuple index out-of-bounds violations.
    ///
    /// Used by `BSK-E0103`.
    pub tuple_index_violations: Vec<TupleIndexViolation>,
    /// Invalid attribute accesses on bounded type variables.
    ///
    /// Used by `BSK-E0105`.
    pub bounded_typevar_attr_violations: Vec<BoundedTypeVarAttrViolation>,
    /// Protocol class used where `type[Proto]` is expected.
    ///
    /// Used by `BSK-E0106`.
    pub protocol_class_object_violations: Vec<ProtocolClassObjectViolation>,
    /// `ClassName(args).__hash__()` calls on non-hashable dataclasses.
    ///
    /// A `@dataclass` with `eq=True` (the default) sets `__hash__` to `None`
    /// unless `frozen=True`, `unsafe_hash=True`, or `__hash__` is defined explicitly.
    /// Calling `.__hash__()` on such an instance is an error.
    ///
    /// Used by `BSK-E0063`.
    pub unhashable_hash_call_violations: Vec<UnhashableHashCallViolation>,
    /// Protocol `isinstance`/`issubclass` violations.
    ///
    /// Covers:
    /// - `isinstance(x, Proto)` / `issubclass(x, Proto)` where `Proto` is not
    ///   decorated with `@runtime_checkable`.
    /// - `issubclass(x, Proto)` where `Proto` is a data protocol (has attributes).
    ///
    /// Used by `BSK-E0114`.
    pub protocol_runtime_checkable_violations: Vec<ProtocolRtcViolation>,
    /// Generator-related type violations (invalid return type, yield mismatches).
    ///
    /// Used by `BSK-E0115`.
    pub generator_violations: Vec<GeneratorViolation>,
    /// Unbound type variable usages detected during AST resolution.
    ///
    /// Covers inner class `TypeVar` reuse and function-nested Generic classes
    /// that cannot be detected from the flattened `ResolvedModule` data alone.
    ///
    /// Used by `BSK-E0117`.
    pub unbound_typevar_usages: Vec<UnboundTypeVarUsage>,
    /// The source file path.
    pub path: String,
    /// The original source text (forwarded from parser for span restoration).
    pub source: String,
}

/// A `TypedDict` key/value violation detected during resolution.
#[derive(Debug, Clone)]
pub struct TypedDictKeyViolation {
    /// The span of the offending expression.
    pub span: Span,
    /// The name of the `TypedDict` class.
    pub class_name: String,
    /// The kind of violation.
    pub kind: TypedDictKeyViolationKind,
}

/// Kind of `TypedDict` key/value violation.
#[derive(Debug, Clone)]
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

/// A type argument in a subscript expression, possibly nested.
///
/// Represents both simple names (`T`) and parameterised types (`list[T]`).
#[derive(Debug, Clone)]
pub enum TypeArg {
    /// A simple name reference (e.g. `T`, `int`).
    Simple(String),
    /// A subscript expression (e.g. `list[T]`, `Mapping[K, V]`).
    Subscript {
        /// The base name of the subscript.
        base: String,
        /// The type arguments inside the brackets.
        args: Vec<TypeArg>,
    },
}

/// A base class subscript entry, recording the base name and its type arguments.
///
/// For `class Foo(Base[T, int])`, this captures `base_name = "Base"`,
/// `type_arg_names = ["T", "int"]`, and the structured `type_args`.
#[derive(Debug, Clone)]
pub struct BaseSubscriptEntry {
    /// The base class name being subscripted.
    pub base_name: String,
    /// Flat list of type argument names (for simple cases).
    pub type_arg_names: Vec<String>,
    /// Rich structured type arguments (for nested generics).
    pub type_args: Vec<TypeArg>,
    /// The source span of the subscript expression.
    pub span: Span,
}

/// A violation where augmented assignment widens a `Literal`-typed variable.
#[derive(Debug, Clone)]
pub struct LiteralAugmentedAssignViolation {
    /// The source span of the augmented assignment.
    pub span: Span,
    /// The name of the variable being augmented.
    pub var_name: String,
}

/// A violation where a tuple is indexed out of bounds.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct ProtocolClassObjectViolation {
    /// The source span of the violation.
    pub span: Span,
    /// The name of the Protocol class.
    pub protocol_name: String,
    /// Description of the context (e.g. "argument" or "assignment").
    pub context: String,
}

/// A forward-reference string found in a type alias RHS.
#[derive(Debug, Clone)]
pub struct RhsStringRef {
    /// The referenced name.
    pub name: String,
    /// The source span of the string reference.
    pub span: Span,
}

/// An unbound type variable usage detected during AST resolution.
///
/// Captures cases where a `TypeVar` is used outside its binding scope, such as:
/// - Inner class reusing an outer class's `TypeVar` in `Generic[T]`
/// - Inner class body annotations using outer class `TypeVars`
/// - Function-nested class using function-scoped `TypeVars` in `Generic[T]`
#[derive(Debug, Clone)]
pub struct UnboundTypeVarUsage {
    /// The source span of the unbound usage.
    pub span: Span,
    /// The name of the type variable that is unbound.
    pub typevar_name: String,
    /// Human-readable context description (e.g. "inner class `Bad`").
    pub context: String,
}
