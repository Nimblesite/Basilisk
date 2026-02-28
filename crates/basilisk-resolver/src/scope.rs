//! Scope and function information types produced by the resolver.

/// A byte-offset span within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Byte offset of the start (inclusive).
    pub start: u32,
    /// Byte offset of the end (exclusive).
    pub end: u32,
}

/// Information about a single function parameter.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
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
}

/// A reference to an unhashable expression used as a dict key.
#[derive(Debug, Clone)]
pub struct UnhashableKeyRef {
    /// The span of the unhashable key expression.
    pub span: Span,
    /// A human-readable description of the key type (`"list"`, `"set"`, `"dict"`).
    pub key_type: &'static str,
}

/// A call site detected in module-level code.
#[derive(Debug, Clone)]
pub struct CallSite {
    /// The name of the called function (simple name only; complex callees ignored).
    pub callee: String,
    /// Kinds and spans of positional arguments at the call site.
    pub args: Vec<(RhsKind, Span)>,
    /// Number of keyword arguments at the call site.
    pub keyword_count: usize,
    /// The span of the entire call expression.
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
    /// Empty list literal `[]` — element type unknown without annotation.
    EmptyList,
    /// Empty dict literal `{}` — key/value types unknown without annotation.
    EmptyDict,
    /// `None` literal — type is `None` / `NoneType`.
    NoneValue,
    /// A function or constructor call — return type may be unknown.
    CallExpr,
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
pub struct AttributeInfo {
    /// The attribute name.
    pub name: String,
    /// The span of the attribute name token.
    pub name_span: Span,
    /// `true` when an explicit type annotation is present.
    pub has_annotation: bool,
    /// The span of the annotation expression, if present.
    pub annotation_span: Option<Span>,
}

/// A class definition with its attributes and method names.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
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
    /// Type parameter names extracted from a `Generic[...]` base, if present.
    pub generic_params: Vec<GenericParamInfo>,
    /// `true` when the class inherits from `TypedDict`.
    pub is_typed_dict: bool,
    /// Keyword argument names in the class definition (e.g. `metaclass`, `total`, `other`).
    pub class_keywords: Vec<String>,
    /// `true` when the class is decorated with `@dataclass` or `@dataclass(...)`.
    pub is_dataclass: bool,
    /// `true` when the class is decorated with `@final` or `typing.final`.
    pub is_final: bool,
    /// `true` when the class directly or transitively inherits from an `Enum` family class.
    pub is_enum: bool,
}

/// Type parameters declared in a `Generic[T1, T2, ...]` base expression.
#[derive(Debug, Clone)]
pub struct GenericParamInfo {
    /// The name of the type parameter (e.g. `"T"`, `"T_co"`).
    pub name: String,
    /// The source span of this parameter name inside `Generic[...]`.
    pub span: Span,
}

/// Information about a module-level `TypeVar(...)` call.
#[derive(Debug, Clone)]
pub struct TypeVarCallInfo {
    /// The name the `TypeVar` is bound to (LHS of assignment).
    pub name: String,
    /// Number of positional constraint arguments (excludes the name string).
    pub constraint_count: usize,
    /// Whether a `default=` keyword argument is present (PEP 696).
    pub has_default: bool,
    /// Whether a `bound=` keyword argument is present.
    pub has_bound: bool,
    /// The span of the entire `TypeVar` call expression.
    pub span: Span,
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

/// The complete resolved view of a parsed module.
#[derive(Debug)]
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
    pub assert_type_calls: Vec<RevealTypeCallInfo>,
    /// Module-level `TypedDict(...)` functional-syntax call sites.
    pub typeddict_calls: Vec<TypedDictCallInfo>,
    /// The source file path.
    pub path: String,
    /// The original source text (forwarded from parser for span resolution).
    pub source: String,
}
