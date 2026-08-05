//! Implements [LINESCANPLAN-AST-MIGRATION]. See
//! docs/plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-AST-MIGRATION
//!
//! The ONE way a rule asks "does this annotation denote `typing.X`?".
//!
//! The spec mandates recognising typing's special forms, but it never
//! mandates a SPELLING: `ClassVar`, `typing.ClassVar`, `t.ClassVar`, and
//! `from typing import ClassVar as CV` all denote the same form, and a
//! user-defined class merely NAMED `ClassVar` denotes none of them. Comparing
//! sliced source text against a hardcoded spelling (`ann.contains("CV[")`)
//! gets both directions wrong, and the wrong answers happened to match the
//! conformance fixtures' spellings ([CHKARCH-CONFORMANCE-MODE], issue #408).
//!
//! Every question here is answered on the parsed `ruff` node through the
//! module's import/binding cascade ([TYPEINF-ANNOTATION-RESOLUTION]), so no
//! verdict can depend on how a symbol was imported or how the line was
//! formatted.

use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;

/// The dotted spelling of a name/attribute chain (`ClassVar`, `typing.ClassVar`),
/// or `None` when the expression is not a dotted name.
pub(crate) fn dotted_spelling(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => Some(format!("{}.{}", dotted_spelling(&attr.value)?, attr.attr)),
        _ => None,
    }
}

/// Does `expr` denote the typing-module member `member`, under ANY spelling
/// this module's imports and value bindings allow?
pub(crate) fn denotes(resolver: &AnnotationResolver<'_>, expr: &Expr, member: &str) -> bool {
    dotted_spelling(expr).is_some_and(|spelling| resolver.decorator_denotes(&spelling, member))
}

/// Does `expr` denote the abstract collection `member` from `typing` OR
/// `collections.abc`? The spec treats the two homes as the same protocol
/// (`Mapping`, `Iterable`, ...), so recognition must too.
pub(crate) fn denotes_abc(resolver: &AnnotationResolver<'_>, expr: &Expr, member: &str) -> bool {
    dotted_spelling(expr).is_some_and(|spelling| {
        resolver.spelling_denotes_from(
            &spelling,
            member,
            &["typing", "typing_extensions", "collections.abc"],
        )
    })
}

/// When `expr` is `Member[...]` for the typing member `member`, the subscript
/// slice; otherwise `None`. Use this instead of `text.starts_with("Member[")`.
pub(crate) fn subscript_of<'e>(
    resolver: &AnnotationResolver<'_>,
    expr: &'e Expr,
    member: &str,
) -> Option<&'e Expr> {
    let Expr::Subscript(subscript) = expr else {
        return None;
    };
    denotes(resolver, &subscript.value, member).then_some(&subscript.slice)
}

/// Does `expr` denote `member`, bare or subscripted? `ClassVar` and
/// `ClassVar[int]` both answer yes for `"ClassVar"`.
pub(crate) fn denotes_form(resolver: &AnnotationResolver<'_>, expr: &Expr, member: &str) -> bool {
    match expr {
        Expr::Subscript(subscript) => denotes(resolver, &subscript.value, member),
        other => denotes(resolver, other, member),
    }
}

/// The comma-separated arguments of a subscript slice: `X[a, b]` yields
/// `[a, b]`, `X[a]` yields `[a]`.
pub(crate) fn subscript_args(slice: &Expr) -> Vec<&Expr> {
    match slice {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        single => vec![single],
    }
}

/// Peel every qualifier the spec allows to wrap a type in an annotation —
/// `Annotated[T, ...]`, `ClassVar[T]`, `Final[T]`, `Required[T]`,
/// `NotRequired[T]`, `ReadOnly[T]`, `InitVar[T]` — returning the innermost
/// type expression. A bare qualifier with no argument returns itself.
pub(crate) fn strip_qualifiers<'e>(resolver: &AnnotationResolver<'_>, expr: &'e Expr) -> &'e Expr {
    const QUALIFIERS: &[&str] = &[
        "Annotated",
        "ClassVar",
        "Final",
        "Required",
        "NotRequired",
        "ReadOnly",
        "InitVar",
    ];
    let mut current = expr;
    // Nesting is bounded by the annotation's own depth; the loop terminates
    // because each step descends one subscript.
    loop {
        let Expr::Subscript(subscript) = current else {
            return current;
        };
        let Some(matched) = QUALIFIERS
            .iter()
            .find(|member| denotes(resolver, &subscript.value, member))
        else {
            return current;
        };
        // `Annotated[T, meta...]` carries metadata after the type; every
        // other qualifier takes exactly one argument.
        let inner = if *matched == "Annotated" {
            match subscript_args(&subscript.slice).first().copied() {
                Some(first) => first,
                None => return current,
            }
        } else {
            &subscript.slice
        };
        current = inner;
    }
}
