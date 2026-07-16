//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Function-related types: parameters, return annotations, function info.

use super::{
    narrowing_types::NarrowingGuard, rhs::RhsKind, span::Span, variable_types::VariableInfo,
};

/// Information about a single function parameter.
#[derive(Debug, Clone, PartialEq)]
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

/// A `return` statement found inside a function body.
#[derive(Debug, Clone, PartialEq)]
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
    /// Used for return type inference in BSK-0002.
    pub rhs_kind: RhsKind,
}

/// A `yield` or `yield from` expression found inside a generator function body.
#[derive(Debug, Clone, PartialEq)]
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

/// Information about a single function definition.
#[derive(Debug, Clone, PartialEq)]
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
    /// Byte offset immediately after the closing `)` of the parameter list.
    ///
    /// Used by the LSP to know where to insert `-> ReturnType` in autofixes.
    pub params_end: u32,
    /// The span of the return annotation expression, if present.
    pub return_annotation_span: Option<Span>,
    /// Name of the containing class, if this function is a method.
    pub class_name: Option<String>,
    /// `true` when this function is lexically nested inside a class body — e.g.
    /// a closure defined inside a method — even though it is not itself a method
    /// (`class_name` is `None`).  Used by `generics_self_usage` to know that `Self` still
    /// has a valid enclosing-class binding and must not be flagged as
    /// module-level `Self` usage.
    pub nested_in_class: bool,
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
    pub unhashable_keys: Vec<super::module_types::UnhashableKeyRef>,
    /// `true` when the entire function body is a stub (only `...` or `pass`).
    ///
    /// Stub bodies are exempt from BSK-0001/BSK-0002/BSK-0004: they appear in overload
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
    /// Un-annotated local `x = <expr>` bindings declared anywhere in the
    /// function body (excluding nested function bodies).
    ///
    /// For `n = 0` inside the function, this contains a `VariableInfo` with
    /// `has_annotation = false`.  Consumed by the LSP inlay-hints pass to render
    /// inferred `: <type>` hints for local variables (see
    /// `LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-INLAYHINTS`).  Kept separate
    /// from `local_vars`, which is annotated-only for the checker rules.
    pub local_unannotated_vars: Vec<VariableInfo>,
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
    /// Type narrowing guards detected in this function body.
    ///
    /// Collected during AST resolution for the planned checker narrowing
    /// engine (NARROWPLAN Phase 1); currently unconsumed. See
    /// `CHECKER-TYPE-INFERENCE-SPEC.md` §7.
    pub narrowing_guards: Vec<NarrowingGuard>,
}
