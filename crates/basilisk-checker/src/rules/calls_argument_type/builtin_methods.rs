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
///
/// DELETED — panics. The body opened with
/// `annotation.replace(' ', "") == "object"`: it STRIPPED WHITESPACE OUT OF
/// THE SOURCE and then compared the remainder to a builtin's spelling, so the
/// verdict depended on how the stub was formatted and a user class named
/// `object` was treated as the top type.
fn stub_argument_compatible(_annotation: &str, _argument: &InferredType) -> bool {
    panic!(
        "basilisk-checker: `stub_argument_compatible` was DELETED because it decided \
         compatibility by deleting spaces from the annotation TEXT and comparing the \
         result to `\"object\"`. It panics because the real implementation — resolving \
         the stub's declared parameter type and asking the ordinary assignability \
         question — DOES NOT EXIST YET. Do not restore the comparison and do not \
         answer `true`/`false` in its place."
    )
}

// `scalar_annotation_mismatch` is GONE — no panic shell, because it has no
// callers left to keep visible. The body split the annotation TEXT at `[`,
// trimmed it, LOWER-CASED it, and matched the result against a table of builtin
// name spellings paired with a second spelling rendered from the resolved type.
// Two spelling tables meeting in a `matches!` is not a type judgment: a user
// class `Str` was read as builtin `str`, an aliased import was read as nothing
// at all, and `int [x]` disagreed with `int[x]`. The replacement resolves both
// sides through the binding table and asks the ordinary assignability question.
