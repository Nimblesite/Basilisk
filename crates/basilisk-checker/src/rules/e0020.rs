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

use super::guards::is_protocol_class;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0020",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0020",
};

/// Emits BSK-E0020 when a set of `@overload` functions has no matching
/// implementation (a same-named function without `@overload`).
pub(crate) struct MissingOverloadImpl;

impl Rule for MissingOverloadImpl {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a set of Protocol/ABC class names so we can exempt their methods.
        let exempt_classes: std::collections::HashSet<&str> = module
            .classes
            .iter()
            .filter(|cls| {
                is_protocol_class(cls)
                    || cls
                        .bases
                        .iter()
                        .any(|b| b == "ABC" || b == "abc.ABC" || b == "ABCMeta")
            })
            .map(|cls| cls.name.as_str())
            .collect();

        // Group functions by (class_name, function_name) to handle overloads correctly
        // across different classes that may have the same method name.
        let mut groups: HashMap<(Option<&str>, &str), Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            groups
                .entry((func.class_name.as_deref(), &func.name))
                .or_default()
                .push(func);
        }

        for ((class_name, name), funcs) in &groups {
            let overloaded: Vec<&&FunctionInfo> = funcs
                .iter()
                .filter(|f| has_decorator(&f.decorators, "overload"))
                .collect();

            // No @overload decorators in this group — nothing to check.
            if overloaded.is_empty() {
                continue;
            }

            let non_overloaded: Vec<&&FunctionInfo> = funcs
                .iter()
                .filter(|f| !has_decorator(&f.decorators, "overload"))
                .collect();

            // Case 1: ALL definitions carry @overload (no implementation).
            // Exempt Protocol/ABC classes — they never need a concrete impl.
            // Also require at least 2 @overload signatures — a lone @overload in
            // a stub or protocol declaration is common and must not be flagged.
            if non_overloaded.is_empty() {
                if overloaded.len() < 2 {
                    continue;
                }
                let is_exempt_class = class_name.is_some_and(|cls| exempt_classes.contains(cls));
                let has_abstract = overloaded
                    .iter()
                    .any(|f| has_decorator(&f.decorators, "abstractmethod"));

                if !is_exempt_class && !has_abstract {
                    if let Some(first) = overloaded.first() {
                        diagnostics.push(make_diagnostic(
                            first,
                            name,
                            overloaded.len(),
                            &module.path,
                        ));
                    }
                }
                continue;
            }

            // Case 2: Has at least one @overload AND at least one non-overload
            // implementation, but only exactly 1 @overload — invalid because at
            // least two @overload signatures are required.
            if overloaded.len() == 1 {
                if let Some(first) = overloaded.first() {
                    diagnostics.push(make_single_overload_diagnostic(first, name, &module.path));
                }
            }

            // Case 3: 2+ @overload + at least 1 non-overload implementation — valid.
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
        provenance: None,
    }
}

fn make_single_overload_diagnostic(
    first_func: &FunctionInfo,
    name: &str,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Function `{name}` has only 1 `@overload` signature — at least two are required",
        ),
        span: first_func.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "Add at least one more `@overload`-decorated signature for `{name}`"
        )),
        note: Some(
            "Python's `@overload` protocol requires at least two type signatures before the implementation"
                .to_owned(),
        ),
        provenance: None,
    }
}
