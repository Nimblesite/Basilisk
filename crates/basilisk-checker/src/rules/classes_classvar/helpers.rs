//! Implements [`classes_classvar`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! Shared helpers for `classes_classvar`: STRUCTURAL `ClassVar` recognition,
//! diagnostic construction, and the `TypeParamKind` classification enum.
//!
//! `ClassVar` is recognised through the module's import cascade
//! ([LINESCANPLAN-AST-MIGRATION]), so `ClassVar`, `typing.ClassVar`,
//! `t.ClassVar`, and `from typing import ClassVar as CV` all answer alike, and
//! a user class merely NAMED `ClassVar` answers no.

use basilisk_resolver::Span;
use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::typing_form::{denotes, denotes_form, subscript_args};

/// The error code for this rule.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "classes_classvar",
    docs_url: "https://www.basilisk-python.dev/errors/classes_classvar",
};

/// Classification of a type parameter for error messaging.
#[derive(Debug, Clone, Copy)]
pub(super) enum TypeParamKind {
    /// A `TypeVar` type parameter.
    TypeVar,
    /// A `ParamSpec` type parameter.
    ParamSpec,
    /// A `TypeVarTuple` type parameter.
    TypeVarTuple,
}

impl TypeParamKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::TypeVar => "TypeVar",
            Self::ParamSpec => "ParamSpec",
            Self::TypeVarTuple => "TypeVarTuple",
        }
    }
}

/// Does this annotation node denote `ClassVar`, bare or subscripted?
pub(super) fn is_classvar(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    denotes_form(resolver, expr, "ClassVar")
}

/// The argument list of a `ClassVar[...]` annotation, or `None` when the
/// annotation is not a `ClassVar` subscript. A bare `ClassVar` yields an
/// empty list.
pub(super) fn classvar_args<'e>(
    resolver: &AnnotationResolver<'_>,
    expr: &'e Expr,
) -> Option<Vec<&'e Expr>> {
    match expr {
        Expr::Subscript(subscript) if denotes(resolver, &subscript.value, "ClassVar") => {
            Some(subscript_args(&subscript.slice))
        }
        other if denotes(resolver, other, "ClassVar") => Some(Vec::new()),
        _ => None,
    }
}

/// Does `ClassVar` appear anywhere STRICTLY INSIDE this annotation — nested
/// in another type constructor rather than at the top?
///
/// `Annotated[ClassVar[int], ""]` is the spec's sanctioned exception: the
/// qualifier order there is legal, so the `Annotated` first argument is not
/// treated as nesting.
pub(super) fn has_nested_classvar(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    let Expr::Subscript(subscript) = expr else {
        return false;
    };
    let args = subscript_args(&subscript.slice);
    if denotes(resolver, &subscript.value, "Annotated") {
        // Only the metadata arguments count as nesting; the first argument is
        // the qualified type itself.
        return args
            .iter()
            .skip(1)
            .any(|arg| contains_classvar(resolver, arg));
    }
    if denotes(resolver, &subscript.value, "ClassVar") {
        // The top-level ClassVar's own argument must not itself contain one.
        return args.iter().any(|arg| contains_classvar(resolver, arg));
    }
    args.iter().any(|arg| contains_classvar(resolver, arg))
}

/// Does `ClassVar` appear anywhere in this expression tree, at any depth?
pub(super) fn contains_classvar(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    if is_classvar(resolver, expr) {
        return true;
    }
    match expr {
        Expr::Subscript(subscript) => {
            contains_classvar(resolver, &subscript.value)
                || subscript_args(&subscript.slice)
                    .iter()
                    .any(|arg| contains_classvar(resolver, arg))
        }
        Expr::BinOp(binop) => {
            contains_classvar(resolver, &binop.left) || contains_classvar(resolver, &binop.right)
        }
        Expr::Tuple(tuple) => tuple.elts.iter().any(|e| contains_classvar(resolver, e)),
        _ => false,
    }
}

/// Every `Name` referenced anywhere in the expression tree.
pub(super) fn referenced_names(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Name(name) => out.push(name.id.to_string()),
        Expr::Subscript(subscript) => {
            referenced_names(&subscript.value, out);
            referenced_names(&subscript.slice, out);
        }
        Expr::Attribute(attr) => referenced_names(&attr.value, out),
        Expr::BinOp(binop) => {
            referenced_names(&binop.left, out);
            referenced_names(&binop.right, out);
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                referenced_names(elt, out);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                referenced_names(elt, out);
            }
        }
        Expr::Starred(starred) => referenced_names(&starred.value, out),
        _ => {}
    }
}

/// Construct a `classes_classvar` diagnostic with standard help and note text.
pub(super) fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some("`ClassVar` is only valid as a class body attribute annotation".to_owned()),
        Some(
            "PEP 526: `ClassVar` cannot appear in function signatures, local variables, \
             or module-level annotations, and cannot be nested inside another type"
                .to_owned(),
        ),
    )
}
