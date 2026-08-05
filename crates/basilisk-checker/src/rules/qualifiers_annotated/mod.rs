//! `qualifiers_annotated`: Invalid first argument to `Annotated[...]`.
//!
//! PEP 593 requires that the first argument to `Annotated[...]` be a valid type
//! expression. Verdicts are structural, over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION], issue #408). The following are errors:
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
use ruff_python_ast::{Expr, ExprSubscript};

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};
use crate::rules::shared::{
    annotation_is_type_alias, is_type_expression, ExprIndex, StringPolicy, TypeExprJudge,
};

use super::Rule;

use helpers::{collect_defined_names, span_text};

const CODE: ErrorCode = ErrorCode {
    code: "qualifiers_annotated",
    docs_url: "https://www.basilisk-python.dev/errors/qualifiers_annotated",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some("The first argument to `Annotated[...]` must be a valid type expression"),
        Some(
            "PEP 593: `Annotated[T, metadata...]` requires T to be a type, not a literal or expression",
        ),
    )
}

/// Emits `qualifiers_annotated` when `Annotated[...]` has an invalid first argument, too few args,
/// or when `Annotated` (or a `TypeAlias`) is called directly as a function.
pub(crate) struct AnnotatedInvalidFirstArg;

impl Rule for AnnotatedInvalidFirstArg {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let path = &module.path;
        // The module's own AST and binding tables; a module that does not
        // parse is reported by the parser itself.
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);

        let defined_names = collect_defined_names(
            &module.module_vars,
            &module.imports,
            &module.classes,
            &module.functions,
        );

        check_annotated_in_vars(module, &resolver, &index, path, &defined_names, diagnostics);
        for cls in &module.classes {
            check_annotated_in_attrs(
                &cls.attributes,
                &resolver,
                &index,
                path,
                &defined_names,
                diagnostics,
            );
        }
        check_annotated_in_functions(
            &module.functions,
            &resolver,
            &index,
            path,
            &defined_names,
            diagnostics,
        );

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

        // Detect calls to `Annotated[...]` alias names (e.g. `SmallInt(1)`
        // where `SmallInt: TypeAlias = Annotated[int, ""]`). Alias-hood
        // resolves through the shared cascade, covering every import
        // spelling. Only aliases whose VALUE is an `Annotated[...]` subscript
        // participate — this rule enforces PEP 593, and an alias to a plain
        // class (`ListAlias: TypeAlias = list`) is constructible.
        let type_alias_names: HashSet<String> = module
            .module_vars
            .iter()
            .filter(|var| annotation_is_type_alias(&resolver, var.annotation_span))
            .filter(|var| {
                var.rhs_span
                    .and_then(|span| index.expr(span))
                    .and_then(|expr| annotated_subscript(&resolver, expr))
                    .is_some()
            })
            .map(|var| var.name.clone())
            .collect();
        check_type_alias_calls(&module.calls, &type_alias_names, path, diagnostics);

        // Detect `type[...] = Annotated[...]` and `type[...] = <TypeAlias>` assignments.
        // PEP 593: Annotated is not type-compatible with `type` or `type[T]`.
        check_vars_type_annotation_incompatible(
            module,
            &resolver,
            &index,
            path,
            &type_alias_names,
            diagnostics,
        );

        // Detect `func(Annotated[...])` and `func(TypeAlias)` call arguments.
        // Passing an Annotated expression or TypeAlias where `type[T]` is expected is invalid.
        check_calls_with_annotated_args(
            &module.calls,
            &resolver,
            &index,
            source,
            path,
            &type_alias_names,
            diagnostics,
        );
    }
}

fn check_annotated_in_vars(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let Some(ann) = var.annotation_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        check_annotated_annotation(
            resolver,
            ann,
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
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for attr in attrs {
        let Some(ann) = attr.annotation_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        check_annotated_annotation(
            resolver,
            ann,
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
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
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
            let Some(ann) = param.annotation_span.and_then(|span| index.expr(span)) else {
                continue;
            };
            check_annotated_annotation(
                resolver,
                ann,
                param.name_span,
                &param.name,
                path,
                defined_names,
                diagnostics,
            );
        }
    }
}

/// The subscript node when `expr` is `Annotated[...]` (bare or dotted base).
fn annotated_subscript<'e>(
    resolver: &AnnotationResolver<'_>,
    expr: &'e Expr,
) -> Option<&'e ExprSubscript> {
    let Expr::Subscript(subscript) = expr else {
        return None;
    };
    crate::rules::shared::typing_form::denotes(resolver, &subscript.value, "Annotated")
        .then_some(subscript)
}

fn check_annotated_annotation(
    resolver: &AnnotationResolver<'_>,
    ann: &Expr,
    span: Span,
    name: &str,
    path: &str,
    defined_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(subscript) = annotated_subscript(resolver, ann) else {
        return;
    };
    let args: Vec<&Expr> = match &*subscript.slice {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        single => vec![single],
    };

    // Annotated[int] — too few arguments
    if args.len() < 2 {
        diagnostics.push(make_diagnostic(
            format!("`Annotated` requires at least two arguments for `{name}`"),
            span,
            path,
        ));
        return;
    }

    // The first argument must be a valid type expression. A bare name must
    // also be defined somewhere the module can see.
    let judge = TypeExprJudge {
        non_type: &|_| false,
        strings: StringPolicy::EagerForwardRef,
    };
    let Some(first) = args.first().copied() else {
        return;
    };
    let undefined_bare_name =
        matches!(first, Expr::Name(n) if !defined_names.contains(n.id.as_str()));
    if !is_type_expression(first, &judge) || undefined_bare_name {
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

/// Whether the annotation node is a `type[...]` subscript (bare `type`,
/// `Type`, or dotted `typing.Type`).
fn annotation_is_type_subscript(ann: &Expr) -> bool {
    let Expr::Subscript(subscript) = ann else {
        return false;
    };
    // Only the builtin `type` is matched by name — it needs no import.
    // `typing.Type` must come from the cascade ([TYPEINF-ANNOTATION-RESOLUTION]).
    matches!(&*subscript.value, Expr::Name(name) if name.id.as_str() == "type")
}

/// Emit E0045 for module variables annotated `type[...]` whose RHS is an `Annotated[...]`
/// expression or a known `TypeAlias` name.
///
/// PEP 593: `Annotated[T, ...]` is not compatible with `type[T]` — it is a value that
/// carries metadata, not a type constructor.
fn check_vars_type_annotation_incompatible(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    path: &str,
    type_alias_names: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let is_type_subscript = var
            .annotation_span
            .and_then(|span| index.expr(span))
            .is_some_and(annotation_is_type_subscript);
        if !is_type_subscript {
            continue;
        }
        let Some(rhs) = var.rhs_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        if annotated_subscript(resolver, rhs).is_some() {
            diagnostics.push(make_diagnostic(
                format!(
                    "`Annotated[...]` is not compatible with `type[...]` for `{}`",
                    var.name
                ),
                var.name_span,
                path,
            ));
        } else if let Expr::Name(rhs_name) = rhs {
            if type_alias_names.contains(rhs_name.id.as_str()) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Type alias `{}` (an `Annotated[...]` alias) is not compatible with `type[...]` for `{}`",
                        rhs_name.id, var.name
                    ),
                    var.name_span,
                    path,
                ));
            }
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
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
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
            let Some(arg) = index.expr(*arg_span) else {
                continue;
            };
            if annotated_subscript(resolver, arg).is_some() {
                let arg_text = span_text(source, Some(*arg_span)).unwrap_or("Annotated[...]");
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Annotated[...]` is not compatible with `type[T]` — \
                         `{arg_text}` cannot be used where a type constructor is expected"
                    ),
                    call.span,
                    path,
                ));
            } else if let Expr::Name(arg_name) = arg {
                if type_alias_names.contains(arg_name.id.as_str()) {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "Type alias `{}` (an `Annotated[...]` alias) is not \
                             compatible with `type[T]`",
                            arg_name.id
                        ),
                        call.span,
                        path,
                    ));
                }
            }
        }
    }
}
