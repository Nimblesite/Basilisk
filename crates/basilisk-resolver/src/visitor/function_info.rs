//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Function Info visitor functions.

use ruff_python_ast::{Expr, Parameter, ParameterWithDefault, Stmt, StmtFunctionDef, StmtReturn};
use ruff_text_size::Ranged;

use crate::scope::{
    FunctionInfo, ParameterInfo, ReturnAnnotationKind, ReturnStmtInfo, RhsKind, Span, VariableInfo,
};

use super::annotations::{ann_assign_info_from, annotation_flags};
use super::assigns::{assign_infos_from, collect_all_assigns, collect_unconditional_assigns};
use super::class_info_ext::{
    body_is_stub, decorator_name, decorator_name_and_span, extract_docstring,
};
use super::core::{classify_rhs, source_slice_range, text_range_to_span};
use super::narrowing::collect_narrowing_guards;
use super::type_alias::type_param_name;
use super::unhashable::collect_unhashable_keys_from_stmts;
use super::yield_exprs::{collect_yield_exprs, stmt_contains_yield};

pub(super) fn function_info_from(
    func: &StmtFunctionDef,
    class_name: Option<String>,
) -> FunctionInfo {
    let params = &func.parameters;

    let positional: Vec<ParameterInfo> = params
        .posonlyargs
        .iter()
        .chain(params.args.iter())
        .map(param_with_default_to_info)
        .collect();

    let kwonly: Vec<ParameterInfo> = params
        .kwonlyargs
        .iter()
        .map(param_with_default_to_info)
        .collect();

    let all_params: Vec<ParameterInfo> = positional.into_iter().chain(kwonly).collect();
    let vararg = params.vararg.as_deref().map(parameter_to_info);
    let kwarg = params.kwarg.as_deref().map(parameter_to_info);

    let return_annotation = func
        .returns
        .as_deref()
        .map_or(ReturnAnnotationKind::Missing, return_annotation_kind);

    let return_annotation_span = func
        .returns
        .as_deref()
        .map(|e| text_range_to_span(e.range()));

    let decorators = func
        .decorator_list
        .iter()
        .filter_map(decorator_name)
        .collect();

    let decorator_spans = func
        .decorator_list
        .iter()
        .filter_map(decorator_name_and_span)
        .collect();

    let return_stmts = collect_return_stmts(&func.body);
    let all_local_assigns = collect_all_assigns(&func.body);
    let unconditional_assigns = collect_unconditional_assigns(&func.body);
    let return_name_refs = collect_return_name_refs(&func.body);
    let top_level_return_name_refs = collect_top_level_return_name_refs(&func.body);
    let unhashable_keys = collect_unhashable_keys_from_stmts(&func.body);
    let is_stub_body = body_is_stub(&func.body);
    let has_pep695_type_params = func.type_params.is_some();
    let pep695_type_param_names: Vec<String> = func
        .type_params
        .as_deref()
        .map(|tp| tp.type_params.iter().map(type_param_name).collect())
        .unwrap_or_default();
    let local_vars = collect_local_annotated_vars(&func.body);
    let local_unannotated_vars = collect_local_unannotated_vars(&func.body);
    let yield_exprs = collect_yield_exprs(&func.body);
    let narrowing_guards = collect_narrowing_guards(&func.body);

    FunctionInfo {
        name: func.name.to_string(),
        parameters: all_params,
        vararg,
        kwarg,
        return_annotation,
        decorators,
        decorator_spans,
        return_stmts,
        def_span: text_range_to_span(func.range),
        name_span: text_range_to_span(func.name.range),
        params_end: func.parameters.range().end().to_u32(),
        return_annotation_span,
        class_name,
        all_local_assigns,
        unconditional_assigns,
        return_name_refs,
        top_level_return_name_refs,
        unhashable_keys,
        is_stub_body,
        body_ends_with_return: func
            .body
            .last()
            .is_some_and(|s| matches!(s, Stmt::Return(_))),
        body_last_stmt_terminates: func.body.last().is_some_and(|s| match s {
            Stmt::Raise(_) => true,
            Stmt::Expr(e) => matches!(e.value.as_ref(), Expr::Call(_)),
            _ => false,
        }),
        has_pep695_type_params,
        pep695_type_param_names,
        local_vars,
        local_unannotated_vars,
        is_generator: func.body.iter().any(stmt_contains_yield),
        is_async: func.is_async,
        yield_exprs,
        docstring: extract_docstring(&func.body),
        narrowing_guards,
        // Set to `true` by the class-body visitor for closures nested inside
        // methods; top-level and module-function-nested closures keep `false`.
        nested_in_class: false,
    }
}

use super::walks::walk_function_stmts;

/// Extract the docstring from a function or class body.
///
/// The docstring is the first statement if it is a bare string literal expression.
pub(super) fn collect_local_annotated_vars(stmts: &[Stmt]) -> Vec<VariableInfo> {
    let mut out = Vec::new();
    walk_function_stmts(stmts, &mut |stmt| {
        if let Stmt::AnnAssign(node) = stmt {
            if let Some(info) = ann_assign_info_from(node) {
                out.push(info);
            }
        }
    });
    out
}

/// Collect un-annotated local `x = <expr>` bindings declared anywhere in the
/// function body (excluding nested function bodies).
///
/// The mirror of [`collect_local_annotated_vars`] for plain `Stmt::Assign`
/// targets, which carry no annotation.  Powers the LSP inlay-hints pass.
pub(super) fn collect_local_unannotated_vars(stmts: &[Stmt]) -> Vec<VariableInfo> {
    let mut out = Vec::new();
    walk_function_stmts(stmts, &mut |stmt| {
        if let Stmt::Assign(node) = stmt {
            out.extend(assign_infos_from(node));
        }
    });
    out
}

pub(super) fn param_with_default_to_info(p: &ParameterWithDefault) -> ParameterInfo {
    let mut info = parameter_to_info(&p.parameter);
    info.has_default = p.default.is_some();
    info
}

pub(super) fn parameter_to_info(p: &Parameter) -> ParameterInfo {
    let (annotation_is_any, annotation_is_numeric_literal) =
        p.annotation.as_deref().map_or((false, false), |e| {
            let (is_any, _, is_num) = annotation_flags(e);
            (is_any, is_num)
        });

    ParameterInfo {
        name: p.name.to_string(),
        has_annotation: p.annotation.is_some(),
        annotation_is_any,
        annotation_is_numeric_literal,
        has_default: false,
        name_span: text_range_to_span(p.name.range),
        annotation_span: p
            .annotation
            .as_deref()
            .map(|e| text_range_to_span(e.range())),
        annotation_text: p.annotation.as_deref().map(annotation_source_text),
    }
}

/// Produce a canonical text representation of an annotation `Expr` for
/// structural comparison (e.g. overlapping-overload detection).
pub(super) fn annotation_source_text(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Attribute(a) => format!("{}.{}", annotation_source_text(&a.value), a.attr),
        Expr::Subscript(s) => format!(
            "{}[{}]",
            annotation_source_text(&s.value),
            annotation_source_text(&s.slice)
        ),
        Expr::Tuple(t) => t
            .elts
            .iter()
            .map(annotation_source_text)
            .collect::<Vec<_>>()
            .join(", "),
        Expr::BinOp(b) => format!(
            "{} | {}",
            annotation_source_text(&b.left),
            annotation_source_text(&b.right)
        ),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::EllipsisLiteral(_) => "...".to_owned(),
        Expr::StringLiteral(s) => format!("\"{}\"", s.value),
        Expr::NumberLiteral(n) => format!("{n:?}"),
        Expr::BooleanLiteral(b) => if b.value { "True" } else { "False" }.to_owned(),
        Expr::Starred(s) => format!("*{}", annotation_source_text(&s.value)),
        Expr::List(l) => {
            let inner = l
                .elts
                .iter()
                .map(annotation_source_text)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        _ => format!("{expr:?}"),
    }
}

/// Collect `return` statements from a function body (not into nested functions).
pub(super) fn collect_return_stmts(stmts: &[Stmt]) -> Vec<ReturnStmtInfo> {
    let mut out = Vec::new();
    walk_function_stmts(stmts, &mut |stmt| {
        if let Stmt::Return(ret) = stmt {
            out.push(return_stmt_info_from(ret));
        }
    });
    out
}

pub(super) fn return_stmt_info_from(ret: &StmtReturn) -> ReturnStmtInfo {
    let value_expr = ret.value.as_deref();
    let has_value = value_expr.is_some_and(|e| !matches!(e, Expr::NoneLiteral(_)));
    let value_is_call = value_expr.is_some_and(|e| matches!(e, Expr::Call(_)));
    let rhs_kind = value_expr.map_or(RhsKind::Other, classify_rhs);
    ReturnStmtInfo {
        span: text_range_to_span(ret.range),
        has_value,
        value_is_call,
        rhs_kind,
    }
}

// ---------------------------------------------------------------------------
// Assign name collection helpers
// ---------------------------------------------------------------------------

/// Extract all simple names from an assignment target expression.
/// Handles single names, tuples, and nested tuples (e.g. `for (x, y) in ...`).
pub(super) fn collect_return_name_refs(stmts: &[Stmt]) -> Vec<(String, Span)> {
    let mut out = Vec::new();
    walk_function_stmts(stmts, &mut |stmt| {
        if let Stmt::Return(ret) = stmt {
            if let Some(val) = ret.value.as_deref() {
                collect_name_refs_with_spans(val, &mut out);
                collect_callee_name_refs(val, &mut out);
            }
        }
    });
    out
}

/// Collect the base name of every CALLEE in a return expression — `f` in
/// `f(...)`, `obj` in `obj.method()` — which [`collect_name_refs_with_spans`]
/// deliberately skips (it walks call *arguments*, not the callee). E0018 uses
/// this so `return f()` is checked for an undefined `f`, not just bare
/// `return f`. Kept separate from the shared collector so PEP 695 scoping and
/// E0149 (which want value references, not callees) are unaffected.
fn collect_callee_name_refs(expr: &Expr, out: &mut Vec<(String, Span)>) {
    match expr {
        Expr::Call(call) => {
            collect_name_refs_with_spans(&call.func, out);
            for arg in &call.arguments.args {
                collect_callee_name_refs(arg, out);
            }
        }
        Expr::BinOp(bin) => {
            collect_callee_name_refs(&bin.left, out);
            collect_callee_name_refs(&bin.right, out);
        }
        Expr::Tuple(tup) => {
            for elt in &tup.elts {
                collect_callee_name_refs(elt, out);
            }
        }
        Expr::Subscript(sub) => collect_callee_name_refs(&sub.value, out),
        Expr::Attribute(attr) => collect_callee_name_refs(&attr.value, out),
        Expr::Starred(starred) => collect_callee_name_refs(&starred.value, out),
        _ => {}
    }
}

/// Collects `return <name>` references from the TOP LEVEL of a function body only.
///
/// Unlike [`collect_return_name_refs`], this does NOT recurse into `if`/`for`/
/// `while`/`try`/`with` blocks.  A `return name` inside a conditional branch will
/// only execute when that branch is taken, so `name` is always bound at that point
/// if it was assigned earlier in the same branch.  Recursing would produce false
/// positives; this conservative variant is used by E0019.
pub(super) fn collect_top_level_return_name_refs(stmts: &[Stmt]) -> Vec<(String, Span)> {
    stmts
        .iter()
        .filter_map(|stmt| {
            if let Stmt::Return(ret) = stmt {
                if let Some(Expr::Name(name)) = ret.value.as_deref() {
                    return Some((name.id.to_string(), text_range_to_span(name.range)));
                }
            }
            None
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Unhashable key collection
// ---------------------------------------------------------------------------

/// Walk all statements in a function body looking for dict literals with unhashable keys.
pub(super) fn return_annotation_kind(expr: &Expr) -> ReturnAnnotationKind {
    let (is_any, is_none, is_num) = annotation_flags(expr);
    if is_any {
        ReturnAnnotationKind::Any
    } else if is_none {
        ReturnAnnotationKind::NoneType
    } else if is_num {
        ReturnAnnotationKind::NumericLiteral
    } else {
        ReturnAnnotationKind::Other
    }
}

/// Returns `(is_any, is_none, is_numeric_literal)` for an annotation expression.
pub(super) fn parse_defaults_count(keywords: &[ruff_python_ast::Keyword]) -> usize {
    for kw in keywords {
        if kw
            .arg
            .as_ref()
            .is_some_and(|arg| arg.as_str() == "defaults")
        {
            return match &kw.value {
                Expr::Tuple(t) => t.elts.len(),
                Expr::List(l) => l.elts.len(),
                _ => 0,
            };
        }
    }
    0
}

/// Collect `Final` string-literal constant bindings from module-level statements.
///
/// Returns a map from variable name to the string value (e.g., `X: Final = "x"` yields
/// `"X" -> "x"`).  Only `Final` / `Final[str]` annotations with string-literal RHS
/// are included.
pub(super) fn collect_name_refs_from_expr(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Name(name) => out.push(name.id.to_string()),
        Expr::Subscript(sub) => {
            collect_name_refs_from_expr(&sub.value, out);
            collect_name_refs_from_expr(&sub.slice, out);
        }
        Expr::Attribute(attr) => collect_name_refs_from_expr(&attr.value, out),
        Expr::Tuple(tup) => {
            for elt in &tup.elts {
                collect_name_refs_from_expr(elt, out);
            }
        }
        Expr::BinOp(bin) => {
            collect_name_refs_from_expr(&bin.left, out);
            collect_name_refs_from_expr(&bin.right, out);
        }
        Expr::Call(call) => {
            collect_name_refs_from_expr(&call.func, out);
            for arg in &call.arguments.args {
                collect_name_refs_from_expr(arg, out);
            }
        }
        Expr::Starred(starred) => collect_name_refs_from_expr(&starred.value, out),
        _ => {}
    }
}

/// Recursively collect all `Name` references with their spans from an expression tree.
///
/// Like [`collect_name_refs_from_expr`] but also returns the span of each name.
pub(super) fn collect_name_refs_with_spans(expr: &Expr, out: &mut Vec<(String, Span)>) {
    match expr {
        Expr::Name(name) => out.push((name.id.to_string(), text_range_to_span(name.range))),
        Expr::Subscript(sub) => {
            collect_name_refs_with_spans(&sub.value, out);
        }
        Expr::Attribute(attr) => collect_name_refs_with_spans(&attr.value, out),
        Expr::Tuple(tup) => {
            for elt in &tup.elts {
                collect_name_refs_with_spans(elt, out);
            }
        }
        Expr::BinOp(bin) => {
            collect_name_refs_with_spans(&bin.left, out);
            collect_name_refs_with_spans(&bin.right, out);
        }
        Expr::Call(call) => {
            for arg in &call.arguments.args {
                collect_name_refs_with_spans(arg, out);
            }
        }
        Expr::Starred(starred) => collect_name_refs_with_spans(&starred.value, out),
        _ => {}
    }
}

/// Recursively collect string literal references from an expression tree.
///
/// Finds string literals used as forward references in type annotations
/// (e.g. `"SomeClass"` in `Optional["SomeClass"]`).
pub(super) fn collect_string_refs_from_expr(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::StringLiteral(s) => {
            out.push(s.value.to_str().to_owned());
        }
        Expr::Subscript(sub) => {
            collect_string_refs_from_expr(&sub.value, out);
            collect_string_refs_from_expr(&sub.slice, out);
        }
        Expr::Tuple(tup) => {
            for elt in &tup.elts {
                collect_string_refs_from_expr(elt, out);
            }
        }
        Expr::BinOp(bin) => {
            collect_string_refs_from_expr(&bin.left, out);
            collect_string_refs_from_expr(&bin.right, out);
        }
        _ => {}
    }
}

/// Convert an expression into a structured [`TypeArg`].
///
/// Simple names become `TypeArg::Simple`, subscript expressions become
/// `TypeArg::Subscript { base, args }`. Other expression forms fall back to
/// `TypeArg::Simple` using whatever simple name can be extracted (or an empty
/// string as a last resort).
pub(super) fn build_param_scope_owned(
    parameters: &ruff_python_ast::Parameters,
    source: &str,
) -> Vec<(String, String)> {
    super::walks::iter_all_params(parameters)
        .filter_map(|p| {
            let name = p.parameter.name.to_string();
            let ann = p.parameter.annotation.as_deref()?;
            let range = ann.range();
            let ann_text = source_slice_range(source, range)?;
            Some((name, ann_text.to_owned()))
        })
        .collect()
}
