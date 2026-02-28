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
pub struct ParameterInfo {
    /// The parameter name as it appears in source.
    pub name: String,
    /// `true` when a type annotation is present (`param: Type`).
    pub has_annotation: bool,
    /// `true` when the annotation is explicitly `Any` (from typing).
    pub annotation_is_any: bool,
    /// `true` when the annotation is a numeric or boolean literal (invalid type form).
    pub annotation_is_numeric_literal: bool,
    /// The source span of the parameter name token.
    pub name_span: Span,
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
}

/// A `return` statement found inside a function body.
#[derive(Debug, Clone)]
pub struct ReturnStmtInfo {
    /// The span of the `return` keyword.
    pub span: Span,
    /// `true` when the statement has a non-`None` expression (`return expr`).
    pub has_value: bool,
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
}

/// A class definition with its attributes and method names.
#[derive(Debug, Clone)]
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
    /// The source file path.
    pub path: String,
    /// The original source text (forwarded from parser for span resolution).
    pub source: String,
}
