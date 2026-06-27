//! Implements [BSK-E0021] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! BSK-E0021: Overlapping `@overload` signatures.
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
    code: "BSK-E0021",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0021",
};

/// Emits BSK-E0021 for `@overload` variants whose parameter signatures are
/// structurally identical to an earlier variant in the same group.
pub(crate) struct OverlappingOverloads;

impl Rule for OverlappingOverloads {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Group overloaded functions by (class_name, function_name) so overloads
        // in different classes with the same method name don't cross-contaminate.
        let mut groups: HashMap<(Option<&str>, &str), Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            if has_overload_decorator(&func.decorators) {
                groups
                    .entry((func.class_name.as_deref(), &func.name))
                    .or_default()
                    .push(func);
            }
        }

        for ((_, name), funcs) in &groups {
            check_group(name, funcs, &module.path, diagnostics);
        }
    }
}

/// Checks all pairs within a group for identical signatures.
fn check_group(func_name: &str, funcs: &[&FunctionInfo], path: &str, out: &mut Vec<Diagnostic>) {
    for (later_idx, later) in funcs.iter().enumerate().skip(1) {
        for earlier in funcs.get(..later_idx).unwrap_or_default() {
            if signatures_overlap(earlier, later) {
                out.push(make_diagnostic(later, func_name, path));
                // Only emit one diagnostic per later overload even if it
                // overlaps multiple earlier ones.
                break;
            }
        }
    }
}

/// Two overloads overlap when they have the same number of regular parameters,
/// the same parameter names in the same order, AND at least one non-self/cls
/// parameter on each side has no annotation (meaning the signatures are
/// indistinguishable from a type-annotation perspective).
///
/// `self` and `cls` are always unannotated by convention and must be excluded
/// from the annotation-coverage check.  When all non-self/cls parameters on
/// both overloads carry annotations, the overloads may be distinguished by
/// their type annotations even if names are identical, so we conservatively
/// do not flag them in Phase 1.
fn signatures_overlap(a: &FunctionInfo, b: &FunctionInfo) -> bool {
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

    // Exclude `self` and `cls` only when they lack an explicit annotation —
    // unannotated self/cls carry no type information, but explicitly annotated
    // ones (e.g. `self: "Array[Axis1, Axis2]"`) distinguish overloads.
    let is_implicit = |p: &&basilisk_resolver::ParameterInfo| {
        (p.name == "self" || p.name == "cls") && !p.has_annotation
    };

    let a_typed = a
        .parameters
        .iter()
        .filter(|p| !is_implicit(p))
        .all(|p| p.has_annotation);
    let b_typed = b
        .parameters
        .iter()
        .filter(|p| !is_implicit(p))
        .all(|p| p.has_annotation);

    // If at least one side has unannotated params, they definitely overlap
    // (can't be distinguished). If both are fully annotated, they overlap
    // only when every annotation is textually identical.
    if !a_typed || !b_typed {
        return true;
    }

    // Both fully annotated — overlap only if annotations are identical per param.
    a.parameters
        .iter()
        .zip(b.parameters.iter())
        .filter(|(pa, _)| !is_implicit(pa))
        .all(|(pa, pb)| pa.annotation_text == pb.annotation_text)
}

/// Returns `true` if `"overload"` (or `"typing.overload"`) is in the list.
fn has_overload_decorator(decorators: &[String]) -> bool {
    decorators
        .iter()
        .any(|d| d == "overload" || d.ends_with(".overload"))
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
