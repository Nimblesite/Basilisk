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
use crate::types::InferredType;

const CODE: ErrorCode = ErrorCode {
    code: "narrowing_typeguard",
    docs_url: "https://www.basilisk-python.dev/errors/narrowing_typeguard",
};

/// Emits `narrowing_typeguard` when a method uses `TypeGuard` or `TypeIs` as its return
/// type but has no user-facing parameter (only `self` or `cls`).
///
/// Implements [TYPEINF-NARROWING-TYPEGUARD] and [TYPEINF-NARROWING-TYPEIS] —
/// validity precondition of a user-defined narrowing function: it must have a
/// parameter to narrow. Guard-ness is read from the RESOLVED return type
/// ([TYPEINF-ANNOTATION-RESOLUTION]), so an alias of `TypeGuard[X]` /
/// `TypeIs[X]` is a guard exactly as the spelled-out form is. The narrowing
/// *effect* (positive-only for `TypeGuard`, bidirectional for `TypeIs`) is
/// applied in the narrowing flow (see the consolidated map).
pub(crate) struct TypeGuardNoNarrowingParam;

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
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Standalone entry point (a single-rule test, or any caller outside the
        // driver): build the cascade the driver would otherwise share.
        let annotations = crate::annotation::AnnotationResolver::for_module(module);
        self.check_with_annotations(module, annotations.as_ref(), ctx, diagnostics);
    }

    fn check_with_annotations(
        &self,
        module: &ResolvedModule,
        annotations: Option<&crate::annotation::AnnotationResolver<'_>>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;
        let Some(resolver) = annotations else {
            return;
        };

        for func in &module.functions {
            // Must be a method (inside a class).
            if func.class_name.is_none() {
                continue;
            }

            // Must have a return annotation span we can inspect.
            let Some(ann_span) = func.return_annotation_span else {
                continue;
            };

            // The return type must RESOLVE to a narrowing form — through
            // aliases too, so `Guard = TypeGuard[int]` does not hide one.
            // Resolved from the indexed annotation NODE: slicing the text and
            // re-parsing it costs a `ruff` parse per annotated method.
            let Some(InferredType::Guard { type_is, .. }) = resolver.resolve_span(ann_span) else {
                continue;
            };

            // Check if the method has no user-facing parameters.
            if !has_only_self_or_cls(func) {
                continue;
            }

            let guard_kind = if type_is { "TypeIs" } else { "TypeGuard" };
            // Only the diagnostic path needs the annotation AS WRITTEN, so the
            // source slice happens here rather than on every method checked.
            let ann_text = slice_span(source, ann_span).unwrap_or(guard_kind);

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
