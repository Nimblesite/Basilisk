//! Implements [`classes_classvar`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! Argument validation for `classes_classvar`, over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION]): `ClassVar` takes at most one argument, that
//! argument must be a type expression, and it must not reference a type
//! parameter.

use basilisk_resolver::Span;
use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::Diagnostic;
use crate::rules::shared::{is_type_expression, StringPolicy, TypeExprJudge};

use super::helpers::{make_diagnostic, referenced_names, TypeParamKind};

/// The type parameter kind referenced by this argument, if any.
fn referenced_type_param(
    arg: &Expr,
    type_params: &[(String, TypeParamKind)],
) -> Option<TypeParamKind> {
    let mut names = Vec::new();
    referenced_names(arg, &mut names);
    names.iter().find_map(|name| {
        type_params
            .iter()
            .find(|(param, _)| param == name)
            .map(|(_, kind)| *kind)
    })
}

/// Validate the arguments of one `ClassVar[...]` annotation.
pub(super) fn check_classvar_args(
    resolver: &AnnotationResolver<'_>,
    args: &[&Expr],
    attr_name: &str,
    name_span: Span,
    path: &str,
    type_params: &[(String, TypeParamKind)],
    runtime_names: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if args.len() > 1 {
        diagnostics.push(make_diagnostic(
            format!(
                "`ClassVar` accepts at most one type argument, but `{attr_name}` has {}",
                args.len()
            ),
            name_span,
            path,
        ));
        return;
    }
    let Some(arg) = args.first().copied() else {
        return;
    };

    // The argument must be a type expression. A name bound to a module-level
    // runtime value is not one.
    let judge = TypeExprJudge {
        non_type: &|name| runtime_names.iter().any(|known| known == name),
        strings: StringPolicy::EagerForwardRef,
    };
    if !is_type_expression(arg, &judge) {
        let rendered = crate::rules::shared::ann_str(arg);
        diagnostics.push(make_diagnostic(
            format!("Invalid `ClassVar` argument for `{attr_name}`: `{rendered}` is not a valid type"),
            name_span,
            path,
        ));
        return;
    }

    // PEP 526: a ClassVar's type must not reference the enclosing class's type
    // parameters — a class variable is shared by every specialization.
    if let Some(kind) = referenced_type_param(arg, type_params) {
        diagnostics.push(make_diagnostic(
            format!(
                "`ClassVar` parameter for `{attr_name}` cannot contain {}",
                kind.label()
            ),
            name_span,
            path,
        ));
    }
    let _ = resolver;
}
