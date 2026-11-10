//! Implements [`overloads_consistency`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `overloads_consistency`: Overlapping `@overload` signatures.
//!
//! Within a group of `@overload` functions for the same name, every overload
//! must be distinguishable.  This rule uses a structural heuristic: two
//! overloads are considered overlapping when they have the same parameter count
//! AND identical parameter names in the same order.
//!
//! A diagnostic is emitted for the *later* overload in each conflicting pair,
//! pointing at its name span.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "overloads_consistency",
    docs_url: "https://www.basilisk-python.dev/errors/overloads_consistency",
};

/// Emits `overloads_consistency` for `@overload` variants whose parameter signatures are
/// structurally identical to an earlier variant in the same group.
pub(crate) struct OverlappingOverloads;

impl Rule for OverlappingOverloads {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Bail on parse errors — those are reported separately as BSK-0000.
        if types.annotations().is_none() {
            return;
        }
        // Group overloaded functions by (class_name, function_name) so overloads
        // in different classes with the same method name don't cross-contaminate.
        let mut groups: HashMap<(Option<&str>, &str), Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            if func.is_overload {
                groups
                    .entry((func.class_name.as_deref(), &func.name))
                    .or_default()
                    .push(func);
            }
        }

        let Some(resolver) = types.annotations() else {
            return;
        };
        for ((_, name), funcs) in &groups {
            check_group(resolver, name, funcs, &module.path, diagnostics);
        }
    }
}

/// Checks all pairs within a group for identical signatures.
fn check_group(
    resolver: &crate::annotation::AnnotationResolver<'_>,
    func_name: &str,
    funcs: &[&FunctionInfo],
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    for (later_idx, later) in funcs.iter().enumerate().skip(1) {
        for earlier in funcs.get(..later_idx).unwrap_or_default() {
            if signatures_overlap(resolver, earlier, later) {
                out.push(make_diagnostic(later, func_name, path));
                // Only emit one diagnostic per later overload even if it
                // overlaps multiple earlier ones.
                break;
            }
        }
    }
}

/// Two overloads overlap when they have the same number of regular parameters,
/// the same parameter names in the same order, and cannot be told apart by
/// their parameter TYPES.
///
/// REBUILT on resolved types. The last clause used to read
///
/// ```ignore
/// pa.annotation_text == pb.annotation_text
/// ```
///
/// against a field whose serializer had been deleted and which was therefore
/// `None` for every parameter — so two fully annotated overloads compared
/// `None == None`, "matched", and were reported as overlapping whatever their
/// types actually were. Each annotation now resolves through the module's
/// cascade from its own span, so `int` and `str` are different, `list[int]`
/// and `List[int]` are the same, and an alias expands before the comparison.
///
/// An annotation the cascade cannot ground is not evidence that the two
/// signatures differ, so it is treated as indistinguishable — this rule
/// reports a DEFECT, and an ungrounded leaf must not manufacture one.
fn signatures_overlap(
    resolver: &crate::annotation::AnnotationResolver<'_>,
    a: &FunctionInfo,
    b: &FunctionInfo,
) -> bool {
    if a.parameters.len() != b.parameters.len() {
        return false;
    }

    let names_match = a
        .parameters
        .iter()
        .zip(b.parameters.iter())
        .all(|(pa, pb)| pa.name == pb.name);

    if !names_match {
        return false;
    }

    // The implicit receiver is the function's KIND, not a parameter's name: a
    // method that is not a `@staticmethod` takes one, and a module-level
    // function does not, whatever its first parameter is called. The deleted
    // version tested `p.name == "self" || p.name == "cls"`, which both missed
    // a receiver spelled anything else — legal Python — and stripped a real
    // first argument from any plain function that happened to name it `self`.
    let skip = |func: &FunctionInfo| -> usize {
        usize::from(func.class_name.is_some() && !func.is_staticmethod)
    };
    // An explicitly annotated receiver DOES distinguish overloads
    // (`def m(self: Array[Axis1]) -> ...`), so only an unannotated one is
    // dropped.
    let a_skip = usize::from(
        a.parameters
            .first()
            .is_some_and(|p| skip(a) == 1 && !p.has_annotation),
    );
    let b_skip = usize::from(
        b.parameters
            .first()
            .is_some_and(|p| skip(b) == 1 && !p.has_annotation),
    );

    let a_params = a.parameters.get(a_skip..).unwrap_or_default();
    let b_params = b.parameters.get(b_skip..).unwrap_or_default();
    if a_params.len() != b_params.len() {
        return false;
    }

    a_params.iter().zip(b_params.iter()).all(|(pa, pb)| {
        let a_type = pa
            .annotation_span
            .and_then(|span| resolver.resolve_span(span));
        let b_type = pb
            .annotation_span
            .and_then(|span| resolver.resolve_span(span));
        match (a_type, b_type) {
            // Both grounded: the overloads are indistinguishable only when the
            // resolved types agree.
            (Some(left), Some(right)) => left == right,
            // Either side ungrounded (unannotated, or a leaf the cascade
            // cannot resolve): no evidence they differ.
            _ => true,
        }
    })
}

fn make_diagnostic(func: &FunctionInfo, func_name: &str, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "`@overload` variant of `{func_name}` has the same parameter signature as a previous overload"
        ),
        func.name_span,
        path,
        Some(
            "Each `@overload` variant must have a distinct parameter signature".to_owned(),
        ),
        Some(
            "Overlapping overloads cannot be distinguished at call sites".to_owned(),
        ),
    )
}
