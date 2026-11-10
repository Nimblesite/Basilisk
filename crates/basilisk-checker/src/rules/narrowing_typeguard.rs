//! Implements [`narrowing_typeguard`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `narrowing_typeguard`: `TypeGuard` or `TypeIs` on method with no narrowing parameter.
//!
//! The typing spec requires that a `TypeGuard` or `TypeIs` function must have
//! at least one user-facing parameter to narrow. When a method returns
//! `TypeGuard[X]` or `TypeIs[X]` but only has `self` or `cls`, there is no
//! parameter to narrow and the guard is invalid.

use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

const CODE: ErrorCode = ErrorCode {
    code: "narrowing_typeguard",
    docs_url: "https://www.basilisk-python.dev/errors/narrowing_typeguard",
};

/// Emits `narrowing_typeguard` when a method uses `TypeGuard` or `TypeIs` as its return
/// type but has no user-facing parameter (only `self` or `cls`).
///
/// Implements [TYPEINF-NARROWING-TYPEGUARD] and [TYPEINF-NARROWING-TYPEIS] —
/// validity precondition of a user-defined narrowing function: it must have a
/// parameter to narrow. The narrowing *effect* (positive-only for `TypeGuard`,
/// bidirectional for `TypeIs`) is applied in the out-of-scope resolver narrowing
/// visitor (see the consolidated map).
pub(crate) struct TypeGuardNoNarrowingParam;

/// Returns `true` if the annotation text references `TypeGuard` or `TypeIs`.
fn is_type_guard_or_type_is(ann_text: &str) -> bool {
    ann_text.contains("TypeGuard") || ann_text.contains("TypeIs")
}

/// Returns `true` if the function has only `self` or `cls` parameters
/// (no user-facing parameters to narrow).
fn has_only_self_or_cls(func: &basilisk_resolver::FunctionInfo) -> bool {
    func.parameters
        .iter()
        .all(|param| param.name == "self" || param.name == "cls")
}

impl Rule for TypeGuardNoNarrowingParam {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;

        for func in &module.functions {
            // Must be a method (inside a class).
            if func.class_name.is_none() {
                continue;
            }

            // Must have a return annotation span we can inspect.
            let Some(ann_span) = func.return_annotation_span else {
                continue;
            };

            // Extract annotation text from source.
            let Some(ann_text) = slice_span(source, ann_span) else {
                continue;
            };

            // Check if the return type involves TypeGuard or TypeIs.
            if !is_type_guard_or_type_is(ann_text) {
                continue;
            }

            // Check if the method has no user-facing parameters.
            if !has_only_self_or_cls(func) {
                continue;
            }

            let guard_kind = if ann_text.contains("TypeIs") {
                "TypeIs"
            } else {
                "TypeGuard"
            };

            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Method `{}` returns `{guard_kind}` but has no parameter to narrow",
                    func.name
                ),
                ann_span,
                &module.path,
                Some(format!(
                    "Add a parameter to narrow: `def {}(self, value: object) -> {ann_text}:`",
                    func.name
                )),
                Some(
                    "A `TypeGuard` or `TypeIs` function must have at least one user-facing \
                     parameter to narrow; `self` and `cls` do not count"
                        .to_owned(),
                ),
            ));
        }
    }
}
