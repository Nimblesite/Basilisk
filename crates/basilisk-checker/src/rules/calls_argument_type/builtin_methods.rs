//! Implements [`calls_argument_type`] for bound built-in methods, from
//! [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//!
//! A `receiver.method(...)` call whose receiver has a statically-known built-in
//! type is checked against every applicable overload of the active
//! `builtins.pyi` declaration ([STUBRES-PYI] #288) — never against a hand table.

use basilisk_resolver::{CallSite, ResolvedModule};
use basilisk_stubs::StubFunction;

use crate::diagnostic::Diagnostic;
use crate::rules::shared::judge::TypeJudge;
use crate::types::InferredType;

use super::make_diagnostic;

/// Validate arguments to bound built-in methods against all applicable
/// overloads from the active `builtins.pyi` declaration ([STUBRES-PYI] #288).
///
/// Arguments are judged by the type the module's engine synthesises for them
/// ([`TypeJudge`], [NARROWPLAN-INTEGRATION] Step 3), not by the syntactic
/// shape of the expression, so a display of `str`-typed elements is accepted
/// and a `list[int]` name is rejected (GitHub #356).
pub(super) fn check_builtin_method_argument_types(
    module: &ResolvedModule,
    judge: &TypeJudge<'_, '_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
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
        let argument_types: Vec<InferredType> = call
            .args
            .iter()
            .map(|(_, span)| judge.inferred(Some(*span)))
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
    let (index, ((_, span), argument)) =
        call.args
            .iter()
            .zip(argument_types)
            .enumerate()
            .find(|(index, (_, argument))| {
                declarations.iter().all(|declaration| {
                    stub_parameter_annotation(declaration, *index)
                        .is_some_and(|annotation| !stub_argument_compatible(annotation, argument))
                })
            })?;
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
        .all(|(index, (_, argument))| {
            stub_parameter_annotation(declaration, index)
                .is_none_or(|annotation| stub_argument_compatible(annotation, argument))
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

/// Is one argument's resolved type compatible with the annotation an overload
/// declares for it? Only a positively-known mismatch rejects
/// ([CHKARCH-CONFORMANCE-MODE]).
fn stub_argument_compatible(annotation: &str, argument: &InferredType) -> bool {
    if annotation.replace(' ', "") == "object" {
        return true;
    }
    !scalar_annotation_mismatch(annotation, argument)
}

/// A positively-known scalar argument type that can never satisfy a scalar
/// stub annotation — the type-level restatement of the builtin scalar
/// incompatibilities (`str` where `int` is declared, and so on).
fn scalar_annotation_mismatch(annotation: &str, argument: &InferredType) -> bool {
    let base = annotation
        .split('[')
        .next()
        .unwrap_or(annotation)
        .trim()
        .to_ascii_lowercase();
    let Some(kind) = super::scalar_type_name(argument) else {
        return false;
    };
    matches!(
        (base.as_str(), kind),
        ("int" | "bool" | "float" | "bytes", "str")
            | ("int" | "str" | "float", "bytes")
            | ("int" | "str" | "bool", "float")
            | ("str" | "bytes", "int")
    )
}
