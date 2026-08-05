//! Implements [`qualifiers_final_annotation`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! `qualifiers_final_annotation`: `Final` used in an invalid position.
//!
//! PEP 591 restricts `Final[T]` to:
//!
//! - Module-level variable annotations (`x: Final[int] = 1`)
//! - Class body attribute annotations (`VALUE: Final[int] = 1`)
//! - Instance attribute annotations in `__init__` (`self.x: Final[int] = 1`)
//!
//! The following are all errors:
//!
//! 1. `Final` used in a function parameter annotation
//! 2. `Final` nested inside another type constructor (e.g. `list[Final[int]]`)
//! 3. `Final[ClassVar[...]]` or `ClassVar[Final[...]]` — mutually exclusive
//! 4. `Final[T1, T2]` — more than one type argument
//! 5. Bare `Final` (no type arg, no initializer) at module level
//!
//! Every verdict is structural over the parsed `ruff` AST, with `Final`,
//! `ClassVar`, and `Annotated` resolved through the module's import cascade
//! ([LINESCANPLAN-AST-MIGRATION], issue #408) — no check compares annotation
//! source text against a hardcoded spelling.
//!
//! ```python
//! x: list[Final[int]] = []    # E — Final nested in list
//! def f(x: Final[int]): ...   # E — Final in param
//! VALUE2: ClassVar[Final] = 1 # E — Final with ClassVar
//! BAD1: Final                  # E — bare Final, no assignment
//! BAD2: Final[str, int] = ""  # E — too many type args
//! ```

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};
use crate::rules::shared::typing_form::{denotes, denotes_form, subscript_args, subscript_of};
use crate::rules::shared::ExprIndex;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "qualifiers_final_annotation",
    docs_url: "https://www.basilisk-python.dev/errors/qualifiers_final_annotation",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some(
            "`Final` is only valid as the outermost qualifier in variable or attribute annotations",
        ),
        Some("PEP 591: `Final` cannot be nested, used in parameters, or combined with `ClassVar`"),
    )
}

/// Does `Final` appear anywhere in this expression tree, bare or subscripted?
fn contains_final(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    if denotes_form(resolver, expr, "Final") {
        return true;
    }
    match expr {
        Expr::Subscript(subscript) => {
            contains_final(resolver, &subscript.value)
                || subscript_args(&subscript.slice)
                    .iter()
                    .any(|arg| contains_final(resolver, arg))
        }
        Expr::BinOp(binop) => {
            contains_final(resolver, &binop.left) || contains_final(resolver, &binop.right)
        }
        Expr::Tuple(tuple) => tuple.elts.iter().any(|e| contains_final(resolver, e)),
        _ => false,
    }
}

/// Is `Final` nested inside another type constructor — e.g. `list[Final[int]]`,
/// `Optional[Final[int]]`?
///
/// `Final[...]` at the top level is NOT nested. `ClassVar[Final[...]]` is
/// handled separately (and exempt in dataclasses). `Annotated[Final[...], ...]`
/// is explicitly valid per PEP 591.
fn has_nested_final(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    // Annotated[Final[...], ...] is explicitly valid — skip it.
    if subscript_of(resolver, expr, "Annotated").is_some() {
        return false;
    }
    // ClassVar[Final[...]] is handled by classvar_wrapping_final — skip here
    // to avoid double-reporting (and to respect the dataclass exemption).
    if subscript_of(resolver, expr, "ClassVar").is_some() {
        return false;
    }
    let Expr::Subscript(subscript) = expr else {
        return false;
    };
    if denotes(resolver, &subscript.value, "Final") {
        // The top-level Final's own argument must not itself contain one.
        return subscript_args(&subscript.slice)
            .iter()
            .any(|arg| contains_final(resolver, arg));
    }
    subscript_args(&subscript.slice)
        .iter()
        .any(|arg| contains_final(resolver, arg))
}

/// Is the annotation `ClassVar[...Final...]` — `Final` inside `ClassVar`?
fn classvar_wrapping_final(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    subscript_of(resolver, expr, "ClassVar").is_some_and(|slice| {
        subscript_args(slice)
            .iter()
            .any(|arg| contains_final(resolver, arg))
    })
}

/// Is the annotation `Final[...ClassVar...]` — `ClassVar` inside `Final`?
fn final_wrapping_classvar(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    subscript_of(resolver, expr, "Final").is_some_and(|slice| {
        subscript_args(slice)
            .iter()
            .any(|arg| contains_classvar_form(resolver, arg))
    })
}

/// Does `ClassVar` appear anywhere in this expression tree?
fn contains_classvar_form(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    if denotes_form(resolver, expr, "ClassVar") {
        return true;
    }
    match expr {
        Expr::Subscript(subscript) => subscript_args(&subscript.slice)
            .iter()
            .any(|arg| contains_classvar_form(resolver, arg)),
        Expr::BinOp(binop) => {
            contains_classvar_form(resolver, &binop.left)
                || contains_classvar_form(resolver, &binop.right)
        }
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .any(|e| contains_classvar_form(resolver, e)),
        _ => false,
    }
}

/// Is the annotation `Final[T1, T2, ...]` — more than one type argument?
fn final_multiple_type_args(resolver: &AnnotationResolver<'_>, expr: &Expr) -> bool {
    subscript_of(resolver, expr, "Final").is_some_and(|slice| subscript_args(slice).len() > 1)
}

/// Emits `qualifiers_final_annotation` for `Final` used in an invalid position.
pub(crate) struct FinalInvalidPosition;

impl Rule for FinalInvalidPosition {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);
        check_parameters(module, &resolver, &index, diagnostics);
        check_module_vars(module, &resolver, &index, diagnostics);
        check_class_attributes(module, &resolver, &index, diagnostics);
    }
}

/// Function parameters: `Final` is never allowed.
fn check_parameters(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for func in &module.functions {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            let is_final = param
                .annotation_span
                .and_then(|span| index.expr(span))
                .is_some_and(|ann| denotes_form(resolver, ann, "Final"));
            if is_final {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Final` is not allowed in parameter annotation for `{}`",
                        param.name
                    ),
                    param.name_span,
                    &module.path,
                ));
            }
        }
    }
}

/// Module-level variables: bare `Final` without initializer, multiple type
/// arguments, and `Final` nested inside another constructor.
fn check_module_vars(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let Some(ann) = var.annotation_span.and_then(|span| index.expr(span)) else {
            continue;
        };

        // Bare `Final` with no assignment (no type arg, no initializer).
        if denotes(resolver, ann, "Final") && var.rhs_span.is_none() {
            diagnostics.push(make_diagnostic(
                format!(
                    "Bare `Final` annotation for `{}` requires an explicit type argument or initializer",
                    var.name
                ),
                var.name_span,
                &module.path,
            ));
        }

        if final_multiple_type_args(resolver, ann) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`Final` accepts at most one type argument for `{}`",
                    var.name
                ),
                var.name_span,
                &module.path,
            ));
        }

        if has_nested_final(resolver, ann) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`Final` cannot be nested inside another type constructor for `{}`",
                    var.name
                ),
                var.name_span,
                &module.path,
            ));
        }
    }
}

/// Class attributes: the `Final`/`ClassVar` mutual-exclusion rules and
/// nested `Final`.
fn check_class_attributes(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &ExprIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for cls in &module.classes {
        // PEP 681 / dataclasses spec: `ClassVar[Final[int]]` is explicitly valid
        // in dataclasses as a way to declare a final class variable.
        let is_dataclass = cls.is_dataclass;

        for attr in &cls.attributes {
            let Some(ann) = attr.annotation_span.and_then(|span| index.expr(span)) else {
                continue;
            };

            // `ClassVar[Final[...]]` — invalid except in dataclasses.
            if !is_dataclass && classvar_wrapping_final(resolver, ann) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Final` cannot be used inside `ClassVar` for attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    &module.path,
                ));
            }

            // `Final[ClassVar[...]]`
            if final_wrapping_classvar(resolver, ann) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`ClassVar` cannot be used inside `Final` for attribute `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    &module.path,
                ));
            }

            // Final nested in another type.
            if has_nested_final(resolver, ann) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "`Final` cannot be nested inside another type constructor for `{}`",
                        attr.name
                    ),
                    attr.name_span,
                    &module.path,
                ));
            }
        }
    }
}
