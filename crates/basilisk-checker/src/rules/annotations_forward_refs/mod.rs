//! annotations_forward_refs: Invalid type expression in annotation.
//!
//! PEP 484 requires that annotations contain valid type expressions.
//! Only certain expression forms are valid as types:
//!
//! - Names (`int`, `str`, `MyClass`)
//! - Subscripts (`list[int]`, `dict[str, int]`)
//! - Binary-or unions (`int | str`)
//! - String literals (forward references)
//! - `None`
//! - `...` (Ellipsis, in Callable signatures)
//!
//! The following are invalid and should be flagged:
//!
//! - List literals: `[int, str]`
//! - Dict literals: `{}`
//! - Tuple literals: `(int, str)`
//! - List comprehensions: `[int for i in range(1)]`
//! - Lambda expressions (called or uncalled)
//! - Conditional expressions: `int if cond else str`
//! - Boolean binary operators: `int or str`, `int and str`
//! - F-string literals: `f"int"`
//! - Explicit function calls like `eval(...)`
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

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

use scope::{
    build_module_scope_names, is_bare_identifier, is_circular_string_annotation,
    PYTHON_BUILTIN_TYPE_NAMES,
};
use type_checks::{
    collect_non_type_names, is_invalid_type_annotation, is_non_type_name,
    is_paramspec_invalid_annotation,
};

use basilisk_resolver::Span;

const CODE: ErrorCode = ErrorCode {
    code: "annotations_forward_refs",
    docs_url: "https://www.basilisk-python.dev/errors/annotations_forward_refs",
};

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

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

/// Emits annotations_forward_refs when an annotation contains an invalid type expression.
pub(crate) struct InvalidTypeAnnotation;

impl Rule for InvalidTypeAnnotation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        check_invalid_type_annotations(module, diagnostics);
    }
}

fn check_invalid_type_annotations(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let non_type_names = collect_non_type_names(module);
    let module_scope_names = build_module_scope_names(module);
    let builtin_type_names: HashSet<&str> = PYTHON_BUILTIN_TYPE_NAMES.iter().copied().collect();
    let paramspec_names: HashSet<&str> =
        basilisk_resolver::collect_name_set_where(&module.typevar_calls, |tv| tv.is_paramspec);
    let paramspec_generic_bases = collect_paramspec_generic_bases(module, &paramspec_names);

    check_function_param_annotations(
        module,
        &non_type_names,
        &paramspec_names,
        &paramspec_generic_bases,
        diagnostics,
    );
    check_module_var_annotations(module, &non_type_names, &paramspec_names, diagnostics);
    check_local_var_annotations(module, &non_type_names, diagnostics);
    check_class_attr_annotations(
        module,
        &non_type_names,
        &module_scope_names,
        &builtin_type_names,
        diagnostics,
    );
}

/// Names of classes and aliases that are generic over a `ParamSpec` — these
/// may validly be subscripted with a bare `ParamSpec` (PEP 612).
fn collect_paramspec_generic_bases<'a>(
    module: &'a ResolvedModule,
    paramspec_names: &HashSet<&str>,
) -> HashSet<&'a str> {
    let class_bases = module.classes.iter().filter_map(|cls| {
        let is_paramspec_generic = cls.base_subscripts.iter().any(|base| {
            matches!(base.base_name.as_str(), "Protocol" | "Generic")
                && base
                    .type_arg_names
                    .iter()
                    .any(|arg| paramspec_names.contains(arg.as_str()))
        });
        is_paramspec_generic.then_some(cls.name.as_str())
    });
    let alias_bases = module.type_alias_defs.iter().filter_map(|alias| {
        alias
            .rhs_names
            .iter()
            .any(|name| paramspec_names.contains(name.as_str()))
            .then_some(alias.name.as_str())
    });
    class_bases.chain(alias_bases).collect()
}

fn check_function_param_annotations(
    module: &ResolvedModule,
    non_type_names: &HashSet<String>,
    paramspec_names: &HashSet<&str>,
    paramspec_generic_bases: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
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
            let Some(ann) = span_text(source, param.annotation_span) else {
                continue;
            };
            let ann_trimmed = ann.trim();
            if is_invalid_type_annotation(ann_trimmed)
                || is_non_type_name(ann_trimmed, non_type_names)
                || is_paramspec_invalid_annotation(
                    ann_trimmed,
                    paramspec_names,
                    paramspec_generic_bases,
                )
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
    non_type_names: &HashSet<String>,
    paramspec_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    for var in &module.module_vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        let ann_trimmed = ann.trim();
        if is_invalid_type_annotation(ann_trimmed) || is_non_type_name(ann_trimmed, non_type_names)
        {
            diagnostics.push(make_diagnostic(
                format!("Invalid type expression in annotation for `{}`", var.name),
                var.name_span,
                path,
            ));
            continue;
        }
        if ann_trimmed == "TypeAlias" {
            if let Some(rhs) = span_text(source, var.rhs_span) {
                let rhs_trimmed = rhs.trim();
                if paramspec_names.contains(rhs_trimmed) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "`TypeAlias` `{}` has a `ParamSpec` as its type, which is invalid; \
                             `ParamSpec` can only be used in `Callable[P, ReturnType]`",
                            var.name
                        ),
                        var.name_span,
                        path,
                    ));
                }
            }
        }
    }
}

fn check_local_var_annotations(
    module: &ResolvedModule,
    non_type_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;
    for func in &module.functions {
        for var in &func.local_vars {
            let Some(ann) = span_text(source, var.annotation_span) else {
                continue;
            };
            let ann_trimmed = ann.trim();
            if is_invalid_type_annotation(ann_trimmed)
                || is_non_type_name(ann_trimmed, non_type_names)
            {
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
            let Some(ann) = span_text(source, attr.annotation_span) else {
                continue;
            };
            let ann_trimmed = ann.trim();
            if is_invalid_type_annotation(ann_trimmed)
                || is_non_type_name(ann_trimmed, non_type_names)
            {
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
            if is_bare_identifier(ann_trimmed) && cls_method_names.contains(ann_trimmed) {
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
            if is_circular_string_annotation(
                ann_trimmed,
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
