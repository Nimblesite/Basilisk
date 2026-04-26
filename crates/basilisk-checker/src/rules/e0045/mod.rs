//! BSK-E0045: Invalid first argument to `Annotated[...]`.
//!
//! PEP 593 requires that the first argument to `Annotated[...]` be a valid type
//! expression. The following are errors:
//!
//! - List literals: `Annotated[[int, str], ""]`
//! - Tuple literals: `Annotated[((int, str),), ""]`
//! - Dict literals: `Annotated[{"a": "b"}, ""]`
//! - List comprehensions: `Annotated[[x for x in ...], ""]`
//! - Lambda calls: `Annotated[(lambda: int)(), ""]`
//! - Conditional expressions: `Annotated[int if cond else str, ""]`
//! - Boolean literals: `Annotated[True, ""]`
//! - Integer literals: `Annotated[1, ""]`
//! - Binary boolean operators: `Annotated[list or set, ""]`
//! - F-strings: `Annotated[f"...", ""]`
//! - Subscript-into-subscript: `Annotated[[int][0], ""]`
//!
//! Additionally, `Annotated[int]` with fewer than 2 arguments is an error,
//! and calling `Annotated` directly (bare or parameterized) is always invalid.
//!
//! ```python
//! Bad1: Annotated[[int, str], ""]   # E — list literal not valid type
//! Bad9: Annotated[True, ""]          # E — bool literal not valid type
//! Bad13: Annotated[int]              # E — requires at least two arguments
//! Annotated()                        # E — Annotated is not callable
//! SmallInt(1)                        # E — TypeAlias is not callable
//! ```

mod helpers;

use std::collections::HashSet;

use basilisk_resolver::{CallSite, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

use helpers::{
    annotated_inner, annotation_is_type_subscript, collect_defined_names, collect_type_alias_names,
    count_args, first_arg, is_invalid_type_expr, is_undefined_bare_name, span_text,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0045",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0045",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "The first argument to `Annotated[...]` must be a valid type expression".to_owned(),
        ),
        note: Some(
            "PEP 593: `Annotated[T, metadata...]` requires T to be a type, not a literal or expression"
                .to_owned(),
        ),
        provenance: None,
    }
}

/// Emits BSK-E0045 when `Annotated[...]` has an invalid first argument, too few args,
/// or when `Annotated` (or a `TypeAlias`) is called directly as a function.
pub(crate) struct AnnotatedInvalidFirstArg;

impl Rule for AnnotatedInvalidFirstArg {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        let defined_names = collect_defined_names(
            &module.module_vars,
            &module.imports,
            &module.classes,
            &module.functions,
        );

        check_annotated_in_vars(
            &module.module_vars,
            source,
            path,
            &defined_names,
            diagnostics,
        );

        for cls in &module.classes {
            check_annotated_in_attrs(&cls.attributes, source, path, &defined_names, diagnostics);
        }

        check_annotated_in_functions(&module.functions, source, path, &defined_names, diagnostics);

        // Detect direct calls to `Annotated` or `Annotated[...]` — always invalid.
        for span in &module.annotated_direct_call_spans {
            let call_text = span_text(source, Some(*span)).unwrap_or("Annotated");
            diagnostics.push(make_diagnostic(
                format!(
                    "`Annotated` is not callable — `{call_text}` must not be called as a function"
                ),
                *span,
                path,
            ));
        }

        // Detect calls to TypeAlias names (e.g. `SmallInt(1)` where
        // `SmallInt: TypeAlias = Annotated[int, ""]`).
        let type_alias_names = collect_type_alias_names(&module.module_vars, source);
        check_type_alias_calls(&module.calls, &type_alias_names, path, diagnostics);

        // Detect `type[...] = Annotated[...]` and `type[...] = <TypeAlias>` assignments.
        // PEP 593: Annotated is not type-compatible with `type` or `type[T]`.
        check_vars_type_annotation_incompatible(
            &module.module_vars,
            source,
            path,
            &type_alias_names,
            diagnostics,
        );

        // Detect `func(Annotated[...])` and `func(TypeAlias)` call arguments.
        // Passing an Annotated expression or TypeAlias where `type[T]` is expected is invalid.
        check_calls_with_annotated_args(
            &module.calls,
            source,
            path,
            &type_alias_names,
            diagnostics,
        );
    }
}

fn check_annotated_in_vars(
    vars: &[basilisk_resolver::VariableInfo],
    source: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        check_annotated_annotation(
            ann.trim(),
            var.name_span,
            &var.name,
            path,
            defined_names,
            diagnostics,
        );
    }
}

fn check_annotated_in_attrs(
    attrs: &[basilisk_resolver::AttributeInfo],
    source: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for attr in attrs {
        let Some(ann) = span_text(source, attr.annotation_span) else {
            continue;
        };
        check_annotated_annotation(
            ann.trim(),
            attr.name_span,
            &attr.name,
            path,
            defined_names,
            diagnostics,
        );
    }
}

fn check_annotated_in_functions(
    funcs: &[basilisk_resolver::FunctionInfo],
    source: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in funcs {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            let Some(ann) = span_text(source, param.annotation_span) else {
                continue;
            };
            check_annotated_annotation(
                ann.trim(),
                param.name_span,
                &param.name,
                path,
                defined_names,
                diagnostics,
            );
        }
    }
}

fn check_annotated_annotation(
    ann: &str,
    span: Span,
    name: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(inner) = annotated_inner(ann) else {
        return;
    };

    let arg_count = count_args(inner);

    // Annotated[int] — too few arguments
    if arg_count < 2 {
        diagnostics.push(make_diagnostic(
            format!("`Annotated` requires at least two arguments for `{name}`"),
            span,
            path,
        ));
        return;
    }

    // Check that the first argument is a valid type expression
    let first = first_arg(inner);
    if is_invalid_type_expr(first) || is_undefined_bare_name(first, defined_names) {
        diagnostics.push(make_diagnostic(
            format!("Invalid type expression as first argument to `Annotated` for `{name}`"),
            span,
            path,
        ));
    }
}

/// Emit E0045 for module-level calls where the callee is a known `TypeAlias` name.
///
/// A `TypeAlias` variable holds a type expression, not a callable. Calling it is always
/// an error (`SmallInt(1)` where `SmallInt: TypeAlias = Annotated[int, ""]`).
fn check_type_alias_calls(
    calls: &[CallSite],
    type_alias_names: &HashSet<String>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in calls {
        if type_alias_names.contains(&call.callee) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`{}` is a type alias and cannot be called as a function",
                    call.callee
                ),
                call.span,
                path,
            ));
        }
    }
}

/// Emit E0045 for module variables annotated `type[...]` whose RHS is an `Annotated[...]`
/// expression or a known `TypeAlias` name.
///
/// PEP 593: `Annotated[T, ...]` is not compatible with `type[T]` — it is a value that
/// carries metadata, not a type constructor.
fn check_vars_type_annotation_incompatible(
    vars: &[basilisk_resolver::VariableInfo],
    source: &str,
    path: &str,
    type_alias_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        if !annotation_is_type_subscript(ann.trim()) {
            continue;
        }
        let Some(rhs) = span_text(source, var.rhs_span) else {
            continue;
        };
        let rhs = rhs.trim();
        if rhs.starts_with("Annotated[") {
            diagnostics.push(make_diagnostic(
                format!(
                    "`Annotated[...]` is not compatible with `type[...]` for `{}`",
                    var.name
                ),
                var.name_span,
                path,
            ));
        } else if type_alias_names.contains(rhs) {
            diagnostics.push(make_diagnostic(
                format!(
                    "Type alias `{rhs}` (an `Annotated[...]` alias) is not compatible with `type[...]` for `{}`",
                    var.name
                ),
                var.name_span,
                path,
            ));
        }
    }
}

/// Emit E0045 for module-level call sites where a positional argument is an `Annotated[...]`
/// subscript expression, or a known `TypeAlias` name.
///
/// PEP 593: `Annotated[T, ...]` is not type-compatible with `type[T]` — passing it where
/// a `type[T]` value is expected is always a type error.
fn check_calls_with_annotated_args(
    calls: &[CallSite],
    source: &str,
    path: &str,
    type_alias_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in calls {
        // Skip calls whose callee is already a TypeAlias name (handled by check_type_alias_calls).
        if type_alias_names.contains(&call.callee) {
            continue;
        }
        for (_kind, arg_span) in &call.args {
            let Some(arg_text) = span_text(source, Some(*arg_span)) else {
                continue;
            };
            let arg_text = arg_text.trim();
            if arg_text.starts_with("Annotated[") {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Annotated[...]` is not compatible with `type[T]` — \
                         `{arg_text}` cannot be used where a type constructor is expected"
                    ),
                    call.span,
                    path,
                ));
            } else if type_alias_names.contains(arg_text) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Type alias `{arg_text}` (an `Annotated[...]` alias) is not \
                         compatible with `type[T]`"
                    ),
                    call.span,
                    path,
                ));
            }
        }
    }
}
