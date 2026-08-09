//! `annotations_forward_refs`: Invalid type expression in annotation.
//!
//! PEP 484 requires that annotations contain valid type expressions.
//! Only certain expression forms are valid as types:
//!
//! - Names (`int`, `str`, `MyClass`)
//! - Subscripts (`list[int]`, `dict[str, int]`)
//! - Binary-or unions (`int | str`)
//! - String literals (forward references)
//! - `None`
//!
//! The following are invalid and are flagged structurally, on the parsed
//! `ruff` AST ([LINESCANPLAN-AST-MIGRATION], issue #408):
//!
//! - List literals: `[int, str]`
//! - Dict literals: `{}`
//! - Tuple literals: `(int, str)`
//! - List comprehensions: `[int for i in range(1)]`
//! - Lambda expressions (called or uncalled)
//! - Conditional expressions: `int if cond else str`
//! - Boolean binary operators: `int or str`, `int and str`
//! - F-string literals: `f"int"`
//! - Calls — any call, `eval(...)` included, is a runtime expression
//! - Negative numeric literals (positive are caught by E0024)
//! - Names that refer to module objects (`import types` → `types` is a module, not a type)
//! - Names that refer to unannotated literal variables (`var1 = 3` → `var1` is `int`, not a type)
//!
//! ```python
//! def f(x: [int, str]): ...            # E — list literal not a type
//! def g(x: int if True else str): ...  # E — conditional not a type
//! y: {} = {}                            # E — dict literal not a type
//! ```

mod scope;
mod type_checks;

use std::collections::HashSet;

use basilisk_resolver::ResolvedModule;
use ruff_python_ast::Expr;

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};
use crate::rules::shared::ExprIndex;
use crate::span_util::slice_span;

use super::Rule;

use scope::{build_module_scope_names, is_circular_string_annotation};
use type_checks::{
    collect_non_type_names, is_invalid_type_annotation, is_paramspec_invalid_annotation,
};

use basilisk_resolver::Span;

const CODE: ErrorCode = ErrorCode {
    code: "annotations_forward_refs",
    docs_url: "https://www.basilisk-python.dev/errors/annotations_forward_refs",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some("Type annotations must be valid type expressions (class names, subscripts, unions)"),
        Some("PEP 484: annotations should be types, not arbitrary runtime expressions"),
    )
}

/// Emits `annotations_forward_refs` when an annotation contains an invalid type expression.
pub(crate) struct InvalidTypeAnnotation;

impl Rule for InvalidTypeAnnotation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // The module's own AST, parsed once and shared. A module that does
        // not parse is reported by the parser itself.
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);
        check_invalid_type_annotations(module, &index, diagnostics);
    }
}

fn check_invalid_type_annotations(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let non_type_names = collect_non_type_names(module);
    let module_scope_names = build_module_scope_names(module);
    // `PYTHON_BUILTIN_TYPE_NAMES` was DELETED (a builtin-spelling whitelist).
    // The set is empty only so the deleted call site below stays visible as the
    // rebuild map; its consumer panics before reading it.
    let builtin_type_names: HashSet<&str> = HashSet::new();
    let paramspec_names: HashSet<&str> =
        basilisk_resolver::collect_name_set_where(&module.typevar_calls, |tv| tv.is_paramspec);
    let paramspec_generic_bases = collect_paramspec_generic_bases(module, &paramspec_names);

    check_function_param_annotations(
        module,
        index,
        &non_type_names,
        &paramspec_names,
        &paramspec_generic_bases,
        diagnostics,
    );
    check_module_var_annotations(module, index, &non_type_names, diagnostics);
    check_local_var_annotations(module, index, &non_type_names, diagnostics);
    check_class_attr_annotations(
        module,
        index,
        &non_type_names,
        &module_scope_names,
        &builtin_type_names,
        diagnostics,
    );
}

/// Names of aliases that are generic over a `ParamSpec` — these may validly be
/// subscripted with a bare `ParamSpec` (PEP 612).
fn collect_paramspec_generic_bases<'a>(
    module: &'a ResolvedModule,
    paramspec_names: &HashSet<&str>,
) -> HashSet<&'a str> {
    module
        .type_alias_defs
        .iter()
        .filter_map(|alias| {
            alias
                .rhs_names
                .iter()
                .any(|name| paramspec_names.contains(name.as_str()))
                .then_some(alias.name.as_str())
        })
        .collect()
}

fn check_function_param_annotations(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    non_type_names: &HashSet<String>,
    paramspec_names: &HashSet<&str>,
    paramspec_generic_bases: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = &module.path;
    for func in &module.functions {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            if param.annotation_is_numeric_literal {
                continue;
            }
            let Some(ann) = param.annotation_span.and_then(|span| index.expr(span)) else {
                continue;
            };
            if is_invalid_type_annotation(ann, non_type_names)
                || is_paramspec_invalid_annotation(ann, paramspec_names, paramspec_generic_bases)
            {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for parameter `{}`",
                        param.name
                    ),
                    param.name_span,
                    path,
                ));
            }
        }
    }
}

fn check_module_var_annotations(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    non_type_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = &module.path;
    for var in &module.module_vars {
        let Some(ann) = var.annotation_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        if is_invalid_type_annotation(ann, non_type_names) {
            diagnostics.push(make_diagnostic(
                format!("Invalid type expression in annotation for `{}`", var.name),
                var.name_span,
                path,
            ));
        }
    }
}

fn check_local_var_annotations(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    non_type_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = &module.path;
    for func in &module.functions {
        for var in &func.local_vars {
            let Some(ann) = var.annotation_span.and_then(|span| index.expr(span)) else {
                continue;
            };
            if is_invalid_type_annotation(ann, non_type_names) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for local variable `{}`",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }
        }
    }
}

fn check_class_attr_annotations(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    non_type_names: &HashSet<String>,
    module_scope_names: &HashSet<&str>,
    builtin_type_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    for cls in &module.classes {
        let cls_method_names: HashSet<&str> = cls.method_names.iter().map(String::as_str).collect();
        for attr in &cls.attributes {
            let Some(ann) = attr.annotation_span.and_then(|span| index.expr(span)) else {
                continue;
            };
            if is_invalid_type_annotation(ann, non_type_names) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    path,
                ));
                continue;
            }
            if matches!(ann, Expr::Name(name) if cls_method_names.contains(name.id.as_str())) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    path,
                ));
                continue;
            }
            let ann_text = attr
                .annotation_span
                .and_then(|span| slice_span(source, span))
                .unwrap_or_default()
                .trim();
            if is_circular_string_annotation(
                ann_text,
                &attr.name,
                module_scope_names,
                builtin_type_names,
            ) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression in annotation for attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    path,
                ));
            }
        }
    }
}
