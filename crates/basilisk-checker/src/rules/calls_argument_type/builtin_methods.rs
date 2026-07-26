//! Implements [`calls_argument_type`] for bound built-in methods, from
//! [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//!
//! A `receiver.method(...)` call whose receiver has a statically-known built-in
//! type is checked against every applicable overload of the active
//! `builtins.pyi` declaration ([STUBRES-PYI] #288) — never against a hand table.

use basilisk_resolver::{CallSite, ResolvedModule, RhsKind};
use basilisk_stubs::StubFunction;

use crate::diagnostic::Diagnostic;
use crate::types::InferredType;

use super::arg_types::{satisfies_str_iterable, ScopedTypes};
use super::{arg_rhs_mismatch, make_diagnostic};

/// Validate arguments to bound built-in methods against all applicable
/// overloads from the active `builtins.pyi` declaration ([STUBRES-PYI] #288).
///
/// Arguments are judged by their resolved *type* ([`ScopedTypes`]), not by the
/// syntactic shape of the expression, so a display of `str`-typed elements is
/// accepted and a `list[int]` name is rejected (GitHub #356).
pub(super) fn check_builtin_method_argument_types(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Built once, and only for a module that actually calls a built-in method.
    let mut scoped: Option<ScopedTypes<'_>> = None;
    for call in &module.calls {
        let declarations: Vec<_> = module
            .builtin_methods_for_call(call)
            .into_iter()
            .filter(|declaration| {
                crate::rules::calls_argument_count::stub_arity_accepts(declaration, call.args.len())
            })
            .collect();
        if declarations.is_empty() {
            continue;
        }
        let types = scoped.get_or_insert_with(|| ScopedTypes::from_module(module));
        let argument_types: Vec<InferredType> = call
            .args
            .iter()
            .map(|(rhs, span)| types.argument_type(*span, rhs))
            .collect();
        diagnostics.extend(incompatible_argument(
            call,
            &declarations,
            &argument_types,
            &module.path,
        ));
    }
}

/// The diagnostic for the first argument that no applicable overload accepts,
/// or `None` when some overload accepts the whole call.
fn incompatible_argument(
    call: &CallSite,
    declarations: &[&StubFunction],
    argument_types: &[InferredType],
    path: &str,
) -> Option<Diagnostic> {
    if declarations
        .iter()
        .any(|declaration| stub_accepts_call(declaration, call, argument_types))
    {
        return None;
    }
    let (index, ((_, span), argument)) = call.args.iter().zip(argument_types).enumerate().find(
        |(index, ((rhs, _), argument))| {
            declarations.iter().all(|declaration| {
                stub_parameter_annotation(declaration, *index)
                    .is_some_and(|annotation| !stub_argument_compatible(annotation, rhs, argument))
            })
        },
    )?;
    let expected = expected_annotations(declarations, index);
    let description = describe_argument(argument, &expected);
    Some(make_diagnostic(
        &call.callee,
        declarations
            .first()
            .and_then(|declaration| declaration.params.get(index))
            .map_or("argument", |parameter| parameter.name.as_str()),
        &expected,
        &description,
        *span,
        path,
    ))
}

/// Does this overload accept every argument at the call site?
fn stub_accepts_call(
    declaration: &StubFunction,
    call: &CallSite,
    argument_types: &[InferredType],
) -> bool {
    call.args
        .iter()
        .zip(argument_types)
        .enumerate()
        .all(|(index, ((rhs, _), argument))| {
            stub_parameter_annotation(declaration, index)
                .is_none_or(|annotation| stub_argument_compatible(annotation, rhs, argument))
        })
}

/// The declared annotation of an overload's parameter at `index`.
fn stub_parameter_annotation(declaration: &StubFunction, index: usize) -> Option<&str> {
    declaration
        .params
        .get(index)
        .and_then(|parameter| parameter.annotation.as_deref())
}

/// Every distinct annotation the applicable overloads declare at `index`,
/// rendered as the union a caller must satisfy.
fn expected_annotations(declarations: &[&StubFunction], index: usize) -> String {
    declarations
        .iter()
        .filter_map(|declaration| stub_parameter_annotation(declaration, index))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Render the rejected argument as the Python type it resolved to — never as a
/// dump of the resolver's internal expression-kind enum (GitHub #356).
fn describe_argument(argument: &InferredType, expected: &str) -> String {
    match argument {
        InferredType::Unknown => format!("an argument incompatible with `{expected}`"),
        resolved => format!("`{resolved}`"),
    }
}

/// Is one argument compatible with the annotation an overload declares for it?
///
/// `argument` is the resolved type of the expression; `rhs` its syntactic shape,
/// still consulted by the literal-kind comparison in [`arg_rhs_mismatch`].
fn stub_argument_compatible(annotation: &str, rhs: &RhsKind, argument: &InferredType) -> bool {
    let normalized = annotation.replace(' ', "");
    if normalized == "Any" || normalized == "object" {
        return true;
    }
    if normalized.contains("Iterable[str]") || normalized.contains("Iterable[LiteralString]") {
        return satisfies_str_iterable(argument);
    }
    if normalized.contains("LiteralString") {
        return matches!(
            rhs,
            RhsKind::StrLiteral | RhsKind::Other | RhsKind::CallExpr
        );
    }
    arg_rhs_mismatch(annotation, rhs, None).is_none()
}
