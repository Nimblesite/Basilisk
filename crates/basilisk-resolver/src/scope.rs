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
    /// The source span of the parameter name token.
    pub name_span: Span,
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
    /// `true` when a `-> ReturnType` annotation is present.
    pub has_return_annotation: bool,
    /// The span of the `def` keyword (start of the function definition).
    pub def_span: Span,
    /// The span of the function name identifier.
    pub name_span: Span,
}

/// The complete resolved view of a parsed module.
#[derive(Debug)]
pub struct ResolvedModule {
    /// All function definitions found at any nesting level.
    pub functions: Vec<FunctionInfo>,
    /// The source file path.
    pub path: String,
    /// The original source text (forwarded from parser for span resolution).
    pub source: String,
}
