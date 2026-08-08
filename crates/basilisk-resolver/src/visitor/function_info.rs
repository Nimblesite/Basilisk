//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Function Info visitor functions.

use ruff_python_ast::{
    Decorator, Expr, Parameter, ParameterWithDefault, Stmt, StmtFunctionDef, StmtReturn,
};
use ruff_text_size::Ranged;

use crate::canonical::{BindingTable, TypingForm};
use crate::scope::{
    FunctionInfo, ParameterInfo, ReturnAnnotationKind, ReturnStmtInfo, RhsKind, Span, VariableInfo,
};

use super::annotations::{ann_assign_info_from, annotation_flags};
use super::assigns::{assign_infos_from, collect_all_assigns};
use super::class_info_ext::{
    body_is_stub, decorator_name, decorator_name_and_span, extract_docstring,
};
use super::core::{classify_rhs, text_range_to_span};
use super::narrowing::collect_narrowing_guards;
use super::type_alias::type_param_name;
use super::unhashable::collect_unhashable_keys_from_stmts;
use super::yield_exprs::{collect_yield_exprs, stmt_contains_yield};

/// Does any decorator on this definition resolve to `form`?
///
/// The question is the decorator **node**, resolved through the module's
/// bindings — never its spelling. Implements [RESOLV-CANONICAL-BINDING].
fn has_decorator_form(bindings: &BindingTable, decorators: &[Decorator], form: TypingForm) -> bool {
    decorators
        .iter()
        .any(|decorator| bindings.form_of(&decorator.expression) == Some(form))
}

/// Does any decorator resolve to `form`, extending resolution to the builtin
/// scope for a bare, unrebound builtin name (`@staticmethod` needs no import)?
fn has_builtin_decorator_form(
    bindings: &BindingTable,
    decorators: &[Decorator],
    form: TypingForm,
) -> bool {
    decorators
        .iter()
        .any(|decorator| bindings.form_of_with_builtins(&decorator.expression) == Some(form))
}

pub(super) fn function_info_from(
    bindings: &BindingTable,
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
    let return_name_refs = collect_return_name_refs(&func.body);
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
        return_name_refs,
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
        is_overload: has_decorator_form(bindings, &func.decorator_list, TypingForm::Overload),
        is_staticmethod: has_builtin_decorator_form(
            bindings,
            &func.decorator_list,
            TypingForm::StaticMethod,
        ),
        is_classmethod: has_builtin_decorator_form(
            bindings,
            &func.decorator_list,
            TypingForm::ClassMethod,
        ),
        is_abstractmethod: has_decorator_form(
            bindings,
            &func.decorator_list,
            TypingForm::AbstractMethod,
        ),
        is_no_type_check: has_decorator_form(
            bindings,
            &func.decorator_list,
            TypingForm::NoTypeCheck,
        ),
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
    // Classified default kind feeds the BSK-0001 inference exemption
    // ([TYPEINF-FUNC-DEFAULTS]): a literal default already determines the type.
    info.default_rhs_kind = p.default.as_deref().map(classify_rhs);
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
        default_rhs_kind: None,
        name_span: text_range_to_span(p.name.range),
        annotation_span: p
            .annotation
            .as_deref()
            .map(|e| text_range_to_span(e.range())),
        annotation_text: p.annotation.as_deref().map(annotation_source_text),
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
        value_span: value_expr.map(|expr| text_range_to_span(expr.range())),
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

/// Classify a return annotation by the shape rules downstream key off.
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

/// Recursively collect every `Name` reference in an expression tree.
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
/// (e.g. `"SomeClass"` in `list["SomeClass"]`).
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
