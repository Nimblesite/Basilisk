//! Implements [`literals_literalstring`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `literals_literalstring`: `LiteralString` and `Literal` assignment incompatibilities.
//!
//! Detects annotated local variables inside function bodies where the declared
//! type is incompatible with the assigned value, specifically for `LiteralString`
//! and `Literal[...]` types.
//!
//! Covered cases:
//!
//! 1. Assigning a `Literal["X"]`-typed parameter to a `Literal["Y"]` variable
//!    where the literal values differ.
//! 2. Assigning an f-string containing non-`LiteralString` interpolations to
//!    a `LiteralString`-annotated variable.
//! 3. Assigning a generic parameterised with `str` where `LiteralString` is
//!    required (invariant generics like `list`, `Container`).
//! 4. Assigning a `list[LiteralString]` to `list[str]` — lists are invariant.
//!
//! Every verdict is structural over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION]): the previous line reconstructor skipped
//! statements by their leading keyword — including `assert_type`, a spelling
//! fitted to the conformance fixtures ([CHKARCH-CONFORMANCE-MODE], issue
//! #408) — and re-parsed annotations, f-strings, and calls from text.
//!
//! ```python
//! def func(b: Literal["two"], non_literal: str):
//!     x1: Literal[""] = b                          # E — different literal values
//!     x2: LiteralString = f"{non_literal}"          # E — non-literal in f-string
//!     x3: Container[LiteralString] = Container(s)   # E — str ≠ LiteralString
//!     x4: list[str] = val                            # E — invariant mismatch
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};
use ruff_python_ast::{Expr, InterpolatedStringElement};

use crate::annotation::AnnotationResolver;
use crate::diagnostic::Diagnostic;
use crate::rules::shared::typing_form::denotes;
use crate::rules::shared::ExprIndex;

use super::literals_literalstring_helpers::{
    emit_container_call_str_error, emit_fstring_literal_string_error,
    emit_invariant_container_mismatch, emit_literal_value_mismatch,
};
use super::Rule;

/// Emits `literals_literalstring` for `LiteralString` / `Literal[...]` assignment
/// incompatibilities found inside function bodies.
pub(crate) struct LiteralStringAssignment;

impl Rule for LiteralStringAssignment {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);
        for func in &module.functions {
            check_function_locals(func, &resolver, &index, &module.path, diagnostics);
        }
    }
}

/// Check every annotated local assignment (`x: T = expr`) in one function for
/// `LiteralString` / `Literal[...]` violations.
fn check_function_locals(
    func: &FunctionInfo,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let param_anns = param_annotation_nodes(func, index);
    for var in &func.local_vars {
        let Some(annotation) = var.annotation_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        let Some(rhs) = var.rhs_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        check_literal_value_mismatch(
            resolver,
            annotation,
            rhs,
            &param_anns,
            var.name_span,
            path,
            diagnostics,
        );
        check_literal_string_fstring(
            resolver,
            annotation,
            rhs,
            &param_anns,
            var.name_span,
            path,
            diagnostics,
        );
        check_invariant_generic_literal_string(
            resolver,
            annotation,
            rhs,
            &param_anns,
            var.name_span,
            path,
            diagnostics,
        );
    }
}

/// Map each parameter name to its annotation node.
fn param_annotation_nodes<'m, 'ast>(
    func: &'m FunctionInfo,
    index: &'m ExprIndex<'ast>,
) -> HashMap<&'m str, &'ast Expr> {
    func.parameters
        .iter()
        .chain(func.vararg.iter())
        .chain(func.kwarg.iter())
        .filter_map(|param| {
            let ann = param.annotation_span.and_then(|span| index.expr(span))?;
            Some((param.name.as_str(), ann))
        })
        .collect()
}

/// The string value of a `Literal["value"]` annotation — the annotation must
/// denote `typing.Literal` (under any import spelling) subscripted with a
/// single string literal.
fn literal_string_value<'e>(resolver: &AnnotationResolver<'_>, expr: &'e Expr) -> Option<&'e str> {
    let Expr::Subscript(subscript) = expr else {
        return None;
    };
    if !denotes(resolver, &subscript.value, "Literal") {
        return None;
    }
    let Expr::StringLiteral(lit) = subscript.slice.as_ref() else {
        return None;
    };
    Some(lit.value.to_str())
}

/// Does the annotation denote a bare `typing.LiteralString`?
fn is_literal_string(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    denotes(resolver, expr, "LiteralString")
}

/// Does the annotation denote plain builtin `str` — not `LiteralString`, not
/// `Literal[...]`, not a generic?
fn is_plain_str(expr: &Expr) -> bool {
    matches!(expr, Expr::Name(name) if name.id.as_str() == "str")
}

/// Case 1: `x: Literal["X"] = param` where param is `Literal["Y"]` and X ≠ Y.
fn check_literal_value_mismatch(
    resolver: &AnnotationResolver<'_>,
    annotation: &Expr,
    rhs: &Expr,
    param_anns: &HashMap<&str, &Expr>,
    name_span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target_value) = literal_string_value(resolver, annotation) else {
        return;
    };
    // RHS must be a simple name referring to a parameter.
    let Expr::Name(rhs_name) = rhs else {
        return;
    };
    let Some(param_ann) = param_anns.get(rhs_name.id.as_str()) else {
        return;
    };
    let Some(source_value) = literal_string_value(resolver, param_ann) else {
        return;
    };
    if target_value != source_value {
        emit_literal_value_mismatch(name_span, target_value, source_value, path, diagnostics);
    }
}

/// Case 2: `x: LiteralString = f"... {non_literal} ..."` where an
/// interpolated variable has type `str` (not `LiteralString`).
fn check_literal_string_fstring(
    resolver: &AnnotationResolver<'_>,
    annotation: &Expr,
    rhs: &Expr,
    param_anns: &HashMap<&str, &Expr>,
    name_span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !is_literal_string(resolver, annotation) {
        return;
    }
    let Expr::FString(fstring) = rhs else {
        return;
    };
    // Any interpolated name that is a parameter of plain type `str` breaks
    // the `LiteralString` guarantee.
    for element in fstring.value.elements() {
        let InterpolatedStringElement::Interpolation(interpolation) = element else {
            continue;
        };
        let Expr::Name(name) = interpolation.expression.as_ref() else {
            continue;
        };
        if let Some(param_ann) = param_anns.get(name.id.as_str()) {
            if is_plain_str(param_ann) {
                emit_fstring_literal_string_error(
                    name_span,
                    name.id.as_str(),
                    "str",
                    path,
                    diagnostics,
                );
                return; // one diagnostic per assignment is enough
            }
        }
    }
}

/// The container head and single type argument of a generic annotation like
/// `list[str]` — `None` for non-subscript annotations.
fn split_generic(expr: &Expr) -> Option<(&Expr, &Expr)> {
    let Expr::Subscript(subscript) = expr else {
        return None;
    };
    Some((&subscript.value, &subscript.slice))
}

/// Is this container head an invariant (mutable) generic — builtin
/// `list`/`dict`/`set`/`deque` or its `typing` capitalised form?
fn is_invariant_container(resolver: &AnnotationResolver<'_>, head: &Expr) -> bool {
    if let Expr::Name(name) = head {
        if matches!(name.id.as_str(), "list" | "dict" | "set" | "deque") {
            return true;
        }
    }
    ["List", "Dict", "Set", "Deque"]
        .iter()
        .any(|member| denotes(resolver, head, member))
}

/// Do two container heads name the same container?
fn same_container(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Name(a), Expr::Name(b)) => a.id == b.id,
        _ => false,
    }
}

/// Cases 3 & 4: invariant generic mismatches involving `LiteralString`.
///
/// - `x: Container[LiteralString] = Container(s)` where `s: str`
/// - `x: list[str] = val` where `val: list[LiteralString]`
fn check_invariant_generic_literal_string(
    resolver: &AnnotationResolver<'_>,
    annotation: &Expr,
    rhs: &Expr,
    param_anns: &HashMap<&str, &Expr>,
    name_span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((ann_head, ann_arg)) = split_generic(annotation) else {
        return;
    };

    // Case 4: `list[str] = param` where param is `list[LiteralString]`.
    if is_invariant_container(resolver, ann_head) && is_plain_str(ann_arg) {
        if let Expr::Name(rhs_name) = rhs {
            if let Some(param_ann) = param_anns.get(rhs_name.id.as_str()) {
                if let Some((param_head, param_arg)) = split_generic(param_ann) {
                    if same_container(ann_head, param_head)
                        && is_literal_string(resolver, param_arg)
                    {
                        emit_invariant_container_mismatch(
                            name_span,
                            &crate::rules::shared::ann_str(param_ann),
                            &crate::rules::shared::ann_str(annotation),
                            &crate::rules::shared::ann_str(ann_head),
                            path,
                            diagnostics,
                        );
                        return;
                    }
                }
            }
        }
    }

    // Case 3: `Container[LiteralString] = Container(s)` where `s: str`.
    if is_literal_string(resolver, ann_arg) {
        let Expr::Call(call) = rhs else {
            return;
        };
        for arg in &*call.arguments.args {
            let Expr::Name(arg_name) = arg else {
                continue;
            };
            if let Some(param_ann) = param_anns.get(arg_name.id.as_str()) {
                if is_plain_str(param_ann) {
                    emit_container_call_str_error(
                        name_span,
                        &crate::rules::shared::ann_str(rhs),
                        &crate::rules::shared::ann_str(annotation),
                        arg_name.id.as_str(),
                        "str",
                        path,
                        diagnostics,
                    );
                    return;
                }
            }
        }
    }
}
