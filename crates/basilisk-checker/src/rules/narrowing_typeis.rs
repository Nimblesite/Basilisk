//! Implements [`narrowing_typeis`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `narrowing_typeis`: TypeGuard/TypeIs return type incompatibility in callable arguments.
//!
//! When a function returning `TypeGuard[X]` or `TypeIs[X]` is passed as an
//! argument where the expected callable return type is NOT `bool`, this rule
//! reports the mismatch. `TypeGuard` and `TypeIs` are subtypes of `bool` in
//! callable context, so passing them where `Callable[..., bool]` is expected
//! is valid, but passing them where e.g. `Callable[..., str]` is expected is
//! an error.
//!
//! Every verdict is structural over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION], issue #408): `TypeGuard`, `TypeIs`, and
//! `Callable` resolve through the module's import cascade under any spelling,
//! and `TypeIs` invariance compares type structure, never source formatting.
//!
//! ```python
//! def takes_callable_str(f: Callable[[object], str]) -> None: ...
//! def simple_typeguard(val: object) -> TypeGuard[int]: ...
//!
//! takes_callable_str(simple_typeguard)  # E0112 — TypeGuard is bool, not str
//! ```

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;
use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::typing_form::{denotes_abc, subscript_args, subscript_of};
use crate::rules::shared::{ann_str, ExprIndex};

use super::guards::is_protocol_class;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "narrowing_typeis",
    docs_url: "https://www.basilisk-python.dev/errors/narrowing_typeis",
};

/// Emits `narrowing_typeis` when a TypeGuard/TypeIs function is passed to a callable
/// parameter whose return type is not `bool`.
///
/// Implements [TYPEINF-NARROWING-TYPEGUARD] and [TYPEINF-NARROWING-TYPEIS] — the
/// callable-context subtyping rule for narrowing functions: `TypeGuard`/`TypeIs`
/// are subtypes of `bool`, `TypeGuard[X]` is covariant in `X`, and `TypeIs[X]` is
/// invariant in `X` (and the two are not interchangeable).
pub(crate) struct TypeGuardCallableReturnMismatch;

/// A narrowing-guard return annotation: which form, and its type argument.
#[derive(Clone, Copy)]
enum Guard<'e> {
    TypeGuard(&'e Expr),
    TypeIs(&'e Expr),
}

impl<'e> Guard<'e> {
    /// Classify a return annotation as `TypeGuard[X]` / `TypeIs[X]`, resolved
    /// through the import cascade.
    fn of(resolver: &AnnotationResolver<'_>, expr: &'e Expr) -> Option<Self> {
        if let Some(inner) = subscript_of(resolver, expr, "TypeGuard") {
            return Some(Self::TypeGuard(inner));
        }
        subscript_of(resolver, expr, "TypeIs").map(Self::TypeIs)
    }

    fn kind(self) -> &'static str {
        match self {
            Self::TypeGuard(_) => "TypeGuard",
            Self::TypeIs(_) => "TypeIs",
        }
    }

    fn inner(self) -> &'e Expr {
        match self {
            Self::TypeGuard(inner) | Self::TypeIs(inner) => inner,
        }
    }

    fn render(self) -> String {
        format!("{}[{}]", self.kind(), ann_str(self.inner()))
    }
}

/// The return type of a `Callable[[...], R]` annotation, resolved through the
/// cascade (`typing.Callable` and `collections.abc.Callable` alike).
fn callable_return_type<'e>(
    resolver: &AnnotationResolver<'_>,
    annotation: &'e Expr,
) -> Option<&'e Expr> {
    let Expr::Subscript(subscript) = annotation else {
        return None;
    };
    if !denotes_abc(resolver, &subscript.value, "Callable") {
        return None;
    }
    subscript_args(&subscript.slice).last().copied()
}

/// For a Protocol class used as a callable parameter type, the return
/// annotation node of its `__call__` method.
fn protocol_call_return<'m>(
    class_name: &str,
    module: &'m ResolvedModule,
    index: &'m ExprIndex<'_>,
) -> Option<&'m Expr> {
    let cls = module
        .classes
        .iter()
        .find(|c| c.name == class_name && is_protocol_class(c))?;
    let call_method = module
        .functions
        .iter()
        .find(|f| f.class_name.as_deref() == Some(cls.name.as_str()) && f.name == "__call__")?;
    call_method
        .return_annotation_span
        .and_then(|span| index.expr(span))
}

/// Check whether the expected return type is compatible with the actual
/// TypeGuard/TypeIs return of the argument function.
///
/// - `bool` is always compatible (`TypeGuard` and `TypeIs` are subtypes of bool).
/// - `TypeGuard[X]` is **covariant**: `TypeGuard[B]` is assignable to
///   `TypeGuard[A]` when `B` is a subtype of `A` (and not to `TypeIs`).
/// - `TypeIs[X]` is only compatible with `TypeIs[X]` (not `TypeGuard`), and
///   `TypeIs` is **invariant** in its type argument — same STRUCTURE, not
///   same source spelling.
fn is_compatible_return_type(
    resolver: &AnnotationResolver<'_>,
    subtyping: &crate::subtyping::SubtypingContext,
    expected: &Expr,
    actual: Guard<'_>,
) -> bool {
    if matches!(expected, Expr::Name(name) if name.id.as_str() == "bool") {
        return true;
    }
    match (Guard::of(resolver, expected), actual) {
        (Some(Guard::TypeGuard(expected_inner)), Guard::TypeGuard(actual_inner)) => {
            subtyping.is_subtype(&ann_str(actual_inner), &ann_str(expected_inner))
        }
        (Some(Guard::TypeIs(expected_inner)), Guard::TypeIs(actual_inner)) => {
            ann_str(expected_inner) == ann_str(actual_inner)
        }
        // TypeGuard and TypeIs are NOT interchangeable, and any other
        // expected type (e.g. str, int) is incompatible.
        _ => false,
    }
}

/// Map module-level function names to their guard return annotations.
fn build_guard_func_map<'m, 'ast>(
    module: &'m ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    index: &'m ExprIndex<'ast>,
) -> HashMap<&'m str, Guard<'ast>> {
    let mut guard_funcs = HashMap::new();
    for func in &module.functions {
        if func.class_name.is_some() {
            continue;
        }
        let Some(ann) = func
            .return_annotation_span
            .and_then(|span| index.expr(span))
        else {
            continue;
        };
        if let Some(guard) = Guard::of(resolver, ann) {
            let _ = guard_funcs.insert(func.name.as_str(), guard);
        }
    }
    guard_funcs
}

/// Build context-appropriate diagnostic messages for a TypeGuard/TypeIs mismatch.
fn build_mismatch_messages(
    resolver: &AnnotationResolver<'_>,
    arg_name: &str,
    actual: Guard<'_>,
    expected: &Expr,
    callee: &str,
) -> (String, String, String) {
    let guard_return_text = actual.render();
    let guard_kind = actual.kind();
    let expected_text = ann_str(expected);

    if let Some(expected_guard) = Guard::of(resolver, expected) {
        let expected_kind = expected_guard.kind();
        if guard_kind == expected_kind {
            (
                format!(
                    "Function `{arg_name}` returns `{guard_return_text}`, \
                     but `{callee}` expects `{expected_text}`"
                ),
                format!(
                    "`{guard_kind}` is invariant in its type argument; \
                     `{guard_return_text}` is not assignable to \
                     `{expected_text}`"
                ),
                format!(
                    "`{guard_kind}[B]` is not a subtype of \
                     `{guard_kind}[A]` even if `B` is a subtype of `A`"
                ),
            )
        } else {
            (
                format!(
                    "Function `{arg_name}` returns `{guard_return_text}`, \
                     but `{callee}` expects `{expected_text}`"
                ),
                format!(
                    "`{guard_kind}` and `{expected_kind}` are not \
                     interchangeable; use a function returning \
                     `{expected_text}` instead"
                ),
                "`TypeGuard` and `TypeIs` have different narrowing \
                 semantics and are not assignable to each other"
                    .to_owned(),
            )
        }
    } else {
        (
            format!(
                "Function `{arg_name}` returns `{guard_return_text}` \
                 (subtype of `bool`), but `{callee}` expects return \
                 type `{expected_text}`"
            ),
            format!(
                "`{guard_kind}` is a subtype of `bool`, not \
                 `{expected_text}`; change the expected return type \
                 to `bool` or use a compatible callable"
            ),
            format!(
                "`{guard_kind}` in callable context is treated as a \
                 subtype of `bool` per the typing specification"
            ),
        )
    }
}

impl Rule for TypeGuardCallableReturnMismatch {
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

        let mut func_map: HashMap<&str, &basilisk_resolver::FunctionInfo> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_none() {
                let _ = func_map.insert(func.name.as_str(), func);
            }
        }

        let guard_funcs = build_guard_func_map(module, &resolver, &index);
        if guard_funcs.is_empty() {
            return;
        }
        // TypeGuard covariance verdicts route through the module-seeded
        // context ([NARROWPLAN-SUBTYPING]).
        let subtyping = crate::subtyping::module_context(module);

        for call in &module.calls {
            let Some(callee_func) = func_map.get(call.callee.as_str()) else {
                continue;
            };

            for (arg_idx, (_rhs_kind, arg_span)) in call.args.iter().enumerate() {
                // The argument must be a bare reference to a guard function.
                let Some(Expr::Name(arg)) = index.expr(*arg_span) else {
                    continue;
                };
                let Some(&guard) = guard_funcs.get(arg.id.as_str()) else {
                    continue;
                };

                let Some(param_ann) = callee_func
                    .parameters
                    .get(arg_idx)
                    .and_then(|param| param.annotation_span)
                    .and_then(|span| index.expr(span))
                else {
                    continue;
                };

                // The expected return type: from `Callable[..., R]`, or from a
                // Protocol class's `__call__`.
                let expected_return = match callable_return_type(&resolver, param_ann) {
                    Some(ret) => Some(ret),
                    None => match param_ann {
                        Expr::Name(name) => protocol_call_return(name.id.as_str(), module, &index),
                        _ => None,
                    },
                };
                let Some(expected_return) = expected_return else {
                    continue;
                };

                if is_compatible_return_type(&resolver, &subtyping, expected_return, guard) {
                    continue;
                }

                let (msg, help_text, note_text) = build_mismatch_messages(
                    &resolver,
                    arg.id.as_str(),
                    guard,
                    expected_return,
                    &call.callee,
                );

                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    msg,
                    *arg_span,
                    &module.path,
                    Some(help_text),
                    Some(note_text),
                ));
            }
        }
    }
}
