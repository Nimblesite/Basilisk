//! BSK-E0063: Non-hashable dataclass assigned to a `Hashable`-annotated variable.
//!
//! A `@dataclass` with `eq=True` (the default) sets `__hash__` to `None` unless
//! the class is `frozen=True`, uses `unsafe_hash=True`, or explicitly defines
//! a `__hash__` method.  Assigning such an instance to a variable annotated
//! `Hashable` is a type error.
//!
//! ```python
//! from dataclasses import dataclass
//! from typing import Hashable
//!
//! @dataclass
//! class DC1:
//!     a: int
//!
//! v: Hashable = DC1(0)  # E — DC1.__hash__ is None
//!
//! @dataclass(eq=True, frozen=True)
//! class DC2:
//!     a: int
//!
//! v2: Hashable = DC2(0)  # OK — frozen dataclasses are hashable
//! ```
//!
//! PEP 557 specifies the `__hash__` synthesis rules:
//! - If `eq` is true and `frozen` is false, `__hash__` is set to `None`.
//! - If `eq` is true and `frozen` is true, Python synthesises a `__hash__`.
//! - If `unsafe_hash` is true, Python synthesises a `__hash__` regardless.
//! - If `eq` is false, `__hash__` is left untouched (inherited from parent).
//! - If the class defines `__hash__` explicitly, that definition is used.

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0063",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0063",
};

/// Emits BSK-E0063 when a non-hashable dataclass instance is assigned to a
/// `Hashable`-annotated variable.
pub(crate) struct NonHashableDataclassAssignment;

impl Rule for NonHashableDataclassAssignment {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // Build a set of class names that are non-hashable dataclasses.
        // A dataclass is non-hashable when:
        //   - it is a dataclass (`is_dataclass`)
        //   - `eq` is not explicitly False (`!is_dataclass_eq_false`)
        //   - it is not frozen (`!is_dataclass_frozen`)
        //   - it does not use `unsafe_hash=True` (`!is_dataclass_unsafe_hash`)
        //   - it does not define `__hash__` explicitly
        let non_hashable: HashMap<&str, basilisk_resolver::Span> = module
            .classes
            .iter()
            .filter(|cls| {
                cls.is_dataclass
                    && !cls.is_dataclass_eq_false
                    && !cls.is_dataclass_frozen
                    && !cls.is_dataclass_unsafe_hash
                    && !cls.method_names.iter().any(|m| m == "__hash__")
            })
            .map(|cls| (cls.name.as_str(), cls.name_span))
            .collect();

        if non_hashable.is_empty() {
            return;
        }

        let source = &module.source;
        let path = &module.path;

        for var in &module.module_vars {
            // Only care about annotated assignments with a RHS.
            let Some(ann_span) = var.annotation_span else {
                continue;
            };
            let Some(rhs_span) = var.rhs_span else {
                continue;
            };

            // Check whether the annotation is `Hashable` (bare or qualified).
            let Some(ann_text) =
                source.get(ann_span.start as usize..ann_span.end as usize)
            else {
                continue;
            };
            if !is_hashable_annotation(ann_text.trim()) {
                continue;
            }

            // Extract the callee from the RHS (e.g. `DC1` from `DC1(0)`).
            let Some(rhs_text) =
                source.get(rhs_span.start as usize..rhs_span.end as usize)
            else {
                continue;
            };
            let callee = rhs_callee(rhs_text);
            if callee.is_empty() {
                continue;
            }

            let Some(&class_span) = non_hashable.get(callee) else {
                continue;
            };

            diagnostics.push(make_diagnostic(
                var.name_span,
                var.name.as_str(),
                callee,
                class_span,
                path,
            ));
        }
    }
}

/// Returns `true` when the annotation text names the `Hashable` protocol.
fn is_hashable_annotation(text: &str) -> bool {
    matches!(
        text,
        "Hashable"
            | "typing.Hashable"
            | "collections.abc.Hashable"
    )
}

/// Extract the simple callee name from an RHS expression string.
///
/// For `DC1(0)` this returns `"DC1"`.
/// For `module.DC1(0)` this returns `"DC1"`.
/// For anything that is not a call, this returns `""`.
fn rhs_callee(rhs: &str) -> &str {
    let before_paren = rhs.split('(').next().unwrap_or("").trim();
    if before_paren.is_empty() {
        return "";
    }
    before_paren.rsplit('.').next().unwrap_or(before_paren)
}

fn make_diagnostic(
    var_span: basilisk_resolver::Span,
    var_name: &str,
    class_name: &str,
    class_name_span: basilisk_resolver::Span,
    path: &str,
) -> Diagnostic {
    let _ = class_name_span; // span is available for future use in multi-span diagnostics
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Cannot assign `{class_name}` instance to `Hashable`-annotated variable `{var_name}`: \
             `{class_name}` is not hashable"
        ),
        span: var_span,
        path: path.to_owned(),
        help: Some(format!(
            "Make `{class_name}` hashable by adding `frozen=True`, `unsafe_hash=True`, \
             or defining a `__hash__` method"
        )),
        note: Some(
            "PEP 557: a `@dataclass` with `eq=True` (the default) sets `__hash__ = None` \
             unless the class is frozen or uses `unsafe_hash=True`"
                .to_owned(),
        ),
    }
}
