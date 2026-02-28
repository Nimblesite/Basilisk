//! BSK-E0020: Missing `@overload` implementation.
//!
//! When a function name is defined multiple times and every definition carries
//! the `@overload` decorator, there is no concrete implementation body.
//! Python's `typing.overload` protocol requires exactly one implementation
//! function without `@overload`.
//!
//! This rule fires once per overload group that lacks a plain implementation.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0020",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0020",
};

/// Emits BSK-E0020 when a set of `@overload` functions has no matching
/// implementation (a same-named function without `@overload`).
pub(crate) struct MissingOverloadImpl;

impl Rule for MissingOverloadImpl {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Group functions by name, preserving the first occurrence for span reporting.
        let mut groups: HashMap<&str, Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            groups.entry(&func.name).or_default().push(func);
        }

        for (name, funcs) in &groups {
            // A group is interesting only if it has 2+ overloads.
            if funcs.len() < 2 {
                continue;
            }

            let all_overloaded = funcs
                .iter()
                .all(|func| has_decorator(&func.decorators, "overload"));

            if all_overloaded {
                // Use the span of the first definition as the diagnostic anchor.
                if let Some(first) = funcs.first() {
                    diagnostics.push(make_diagnostic(first, name, funcs.len(), &module.path));
                }
            }
        }
    }
}

/// Returns `true` if `decorator_name` is in `decorators` (simple name match,
/// ignoring the `typing.` prefix if callers have already stripped it).
fn has_decorator(decorators: &[String], decorator_name: &str) -> bool {
    decorators
        .iter()
        .any(|d| d == decorator_name || d.ends_with(&format!(".{decorator_name}")))
}

fn make_diagnostic(
    first_func: &FunctionInfo,
    name: &str,
    overload_count: usize,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Function `{}` has {} `@overload` signature{} but no implementation",
            name,
            overload_count,
            if overload_count == 1 { "" } else { "s" },
        ),
        span: first_func.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "Add an implementation function `def {name}(...)` without `@overload`"
        )),
        note: Some(
            "`@overload` signatures are type-only; a concrete implementation is required at runtime"
                .to_owned(),
        ),
    }
}
