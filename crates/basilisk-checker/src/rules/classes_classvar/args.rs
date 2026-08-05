//! Implements [`classes_classvar`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! Argument validation for `classes_classvar`, over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION]): `ClassVar` takes at most one argument, that
//! argument must be a type expression, and it must not reference a type
//! parameter.

use basilisk_resolver::Span;
use ruff_python_ast::Expr;

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
///
/// `non_type` answers "is this bare name known to NOT be a type?" — a name
/// bound to a runtime value, or a name bound to nothing at all.
pub(super) fn check_classvar_args(
    args: &[&Expr],
    attr_name: &str,
    name_span: Span,
    path: &str,
    type_params: &[(String, TypeParamKind)],
    non_type: &dyn Fn(&str) -> bool,
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

    // The argument must be a type expression.
    let judge = TypeExprJudge {
        non_type,
        strings: StringPolicy::EagerForwardRef,
    };
    if !is_type_expression(arg, &judge) {
        let rendered = crate::rules::shared::ann_str(arg);
        diagnostics.push(make_diagnostic(
            format!(
                "Invalid `ClassVar` argument for `{attr_name}`: `{rendered}` is not a valid type"
            ),
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
}

/// The top-level type constructor a value or annotation commits to, for the
/// structural initializer check.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TopKind {
    List,
    Dict,
    Set,
    Tuple,
    Str,
    Int,
    Float,
    Bool,
    Bytes,
}

/// The constructor a `ClassVar` type argument commits to, when knowable.
fn annotation_top_kind(
    resolver: &crate::annotation::AnnotationResolver<'_>,
    expr: &Expr,
) -> Option<TopKind> {
    use crate::rules::shared::typing_form::denotes;
    let head = match expr {
        Expr::Subscript(subscript) => subscript.value.as_ref(),
        other => other,
    };
    if let Expr::Name(name) = head {
        let builtin = match name.id.as_str() {
            "list" => Some(TopKind::List),
            "dict" => Some(TopKind::Dict),
            "set" | "frozenset" => Some(TopKind::Set),
            "tuple" => Some(TopKind::Tuple),
            "str" => Some(TopKind::Str),
            "int" => Some(TopKind::Int),
            "float" => Some(TopKind::Float),
            "bool" => Some(TopKind::Bool),
            "bytes" => Some(TopKind::Bytes),
            _ => None,
        };
        if builtin.is_some() {
            return builtin;
        }
    }
    [
        ("List", TopKind::List),
        ("Dict", TopKind::Dict),
        ("Set", TopKind::Set),
        ("FrozenSet", TopKind::Set),
        ("Tuple", TopKind::Tuple),
    ]
    .into_iter()
    .find_map(|(member, kind)| denotes(resolver, head, member).then_some(kind))
}

/// The constructor a literal initializer value commits to, when knowable.
fn literal_top_kind(expr: &Expr) -> Option<TopKind> {
    match expr {
        Expr::List(_) | Expr::ListComp(_) => Some(TopKind::List),
        Expr::Dict(_) | Expr::DictComp(_) => Some(TopKind::Dict),
        Expr::Set(_) | Expr::SetComp(_) => Some(TopKind::Set),
        Expr::Tuple(_) => Some(TopKind::Tuple),
        Expr::StringLiteral(_) | Expr::FString(_) => Some(TopKind::Str),
        Expr::BytesLiteral(_) => Some(TopKind::Bytes),
        Expr::BooleanLiteral(_) => Some(TopKind::Bool),
        Expr::NumberLiteral(lit) => match &lit.value {
            ruff_python_ast::Number::Int(_) => Some(TopKind::Int),
            ruff_python_ast::Number::Float(_) => Some(TopKind::Float),
            ruff_python_ast::Number::Complex { .. } => None,
        },
        _ => None,
    }
}

/// Is a value of `value` kind assignable where `target` kind is declared?
fn kind_assignable(value: TopKind, target: TopKind) -> bool {
    matches!(
        (value, target),
        (TopKind::Bool, TopKind::Int | TopKind::Float) | (TopKind::Int, TopKind::Float)
    ) || value == target
}

/// PEP 526: the initializer of a `ClassVar[T]` attribute must be assignable
/// to `T`. Only structurally certain mismatches are reported — a dict display
/// initialising `ClassVar[list[str]]` commits to the wrong constructor no
/// matter what its elements are.
pub(super) fn check_classvar_init(
    resolver: &crate::annotation::AnnotationResolver<'_>,
    cv_arg: &Expr,
    rhs: &Expr,
    attr_name: &str,
    name_span: Span,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target) = annotation_top_kind(resolver, cv_arg) else {
        return;
    };
    let Some(value) = literal_top_kind(rhs) else {
        return;
    };
    if kind_assignable(value, target) {
        return;
    }
    let rendered = crate::rules::shared::ann_str(cv_arg);
    diagnostics.push(make_diagnostic(
        format!(
            "Initializer for `ClassVar[{rendered}]` attribute `{attr_name}` \
             does not match the declared type"
        ),
        name_span,
        path,
    ));
}
