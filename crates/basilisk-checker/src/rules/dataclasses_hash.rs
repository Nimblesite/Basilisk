//! Implements [dataclasses_hash] from [CHKARCH-DIAG-COERCION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-coercion
//! dataclasses_hash: Non-hashable dataclass assigned to a `Hashable`-annotated variable.
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

use crate::diagnostic::{error_diag_help_note, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "dataclasses_hash",
    docs_url: "https://www.basilisk-python.dev/errors/dataclasses_hash",
};

/// Emits dataclasses_hash when a non-hashable dataclass instance is assigned to a
/// `Hashable`-annotated variable.
pub(crate) struct NonHashableDataclassAssignment;

impl Rule for NonHashableDataclassAssignment {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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

        if non_hashable.is_empty() && module.unhashable_hash_call_violations.is_empty() {
            return;
        }

        let source = &module.source;
        let path = &module.path;

        // Check `v: Hashable = DC(args)` assignments.
        for var in &module.module_vars {
            // Only care about annotated assignments with a RHS.
            let Some(ann_span) = var.annotation_span else {
                continue;
            };
            let Some(rhs_span) = var.rhs_span else {
                continue;
            };

            // Check whether the annotation is `Hashable` (bare or qualified).
            let Some(ann_text) = slice_span(source, ann_span) else {
                continue;
            };
            if !is_hashable_annotation(ann_text.trim()) {
                continue;
            }

            // Extract the callee from the RHS (e.g. `DC1` from `DC1(0)`).
            let Some(rhs_text) = slice_span(source, rhs_span) else {
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

        // Check `DC(args).__hash__()` calls on non-hashable dataclasses.
        for violation in &module.unhashable_hash_call_violations {
            diagnostics.push(make_hash_call_diagnostic(
                violation.span,
                &violation.class_name,
                path,
            ));
        }
    }
}

/// Returns `true` when the annotation text names the `Hashable` protocol.
fn is_hashable_annotation(text: &str) -> bool {
    matches!(
        text,
        "Hashable" | "typing.Hashable" | "collections.abc.Hashable"
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
    error_diag_help_note(
        CODE.clone(),
        format!(
            "Cannot assign `{class_name}` instance to `Hashable`-annotated variable `{var_name}`: \
             `{class_name}` is not hashable"
        ),
        var_span,
        path,
        format!(
            "Make `{class_name}` hashable by adding `frozen=True`, `unsafe_hash=True`, \
             or defining a `__hash__` method"
        ),
        "PEP 557: a `@dataclass` with `eq=True` (the default) sets `__hash__ = None` \
         unless the class is frozen or uses `unsafe_hash=True`",
    )
}

fn make_hash_call_diagnostic(
    call_span: basilisk_resolver::Span,
    class_name: &str,
    path: &str,
) -> Diagnostic {
    error_diag_help_note(
        CODE.clone(),
        format!(
            "Cannot call `.__hash__()` on `{class_name}` instance: \
             `{class_name}.__hash__` is `None`"
        ),
        call_span,
        path,
        format!(
            "Make `{class_name}` hashable by adding `frozen=True`, `unsafe_hash=True`, \
             or defining a `__hash__` method"
        ),
        "PEP 557: a `@dataclass` with `eq=True` (the default) sets `__hash__ = None` \
         unless the class is frozen or uses `unsafe_hash=True`",
    )
}
