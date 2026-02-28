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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0021",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0021",
};

/// Emits BSK-E0021 for `@overload` variants whose parameter signatures are
/// structurally identical to an earlier variant in the same group.
pub(crate) struct OverlappingOverloads;

impl Rule for OverlappingOverloads {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Group overloaded functions by name.
        let mut groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            if has_overload_decorator(&func.decorators) {
                groups.entry(&func.name).or_default().push(func);
            }
        }

        for (name, funcs) in &groups {
            check_group(name, funcs, &module.path, diagnostics);
        }
    }
}

/// Checks all pairs within a group for identical signatures.
fn check_group(func_name: &str, funcs: &[&FunctionInfo], path: &str, out: &mut Vec<Diagnostic>) {
    for (later_idx, later) in funcs.iter().enumerate().skip(1) {
        for earlier in &funcs[..later_idx] {
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
/// the same parameter names in the same order, AND at least one parameter on
/// each side has no annotation (meaning the signatures are indistinguishable
/// from a type-annotation perspective).
///
/// When all parameters on both overloads carry annotations, the overloads may
/// be distinguished by their type annotations even if names are identical, so
/// we conservatively do not flag them in Phase 1.
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

    // If every parameter on both sides is annotated the overloads might differ
    // by type annotation alone — defer to a future phase.
    let a_all_annotated = a.parameters.iter().all(|p| p.has_annotation);
    let b_all_annotated = b.parameters.iter().all(|p| p.has_annotation);

    !a_all_annotated || !b_all_annotated
}

/// Returns `true` if `"overload"` (or `"typing.overload"`) is in the list.
fn has_overload_decorator(decorators: &[String]) -> bool {
    decorators
        .iter()
        .any(|d| d == "overload" || d.ends_with(".overload"))
}

fn make_diagnostic(func: &FunctionInfo, func_name: &str, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "`@overload` variant of `{func_name}` has the same parameter signature as a previous overload"
        ),
        span: func.name_span,
        path: path.to_owned(),
        help: Some(
            "Each `@overload` variant must have a distinct parameter signature".to_owned(),
        ),
        note: Some(
            "Overlapping overloads cannot be distinguished at call sites".to_owned(),
        ),
    }
}
