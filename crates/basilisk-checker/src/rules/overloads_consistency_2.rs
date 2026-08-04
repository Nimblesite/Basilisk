//! Implements [`overloads_consistency_2`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `overloads_consistency_2`: Inconsistent decorators across an overloaded method.
//!
//! The typing spec constrains how decorators may be spread across an
//! `@overload` group and its implementation:
//!
//! * If any signature is `@staticmethod` / `@classmethod`, *all* signatures and
//!   the implementation must carry the same decorator.
//! * `@final` and `@override` apply to the *implementation only* (or, in a stub,
//!   the first overload). Placing either on an `@overload` signature when an
//!   implementation is present is an error; in a stub (no implementation),
//!   placing either on any but the first overload is an error.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::shared::overload_decorated;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "overloads_consistency_2",
    docs_url: "https://www.basilisk-python.dev/errors/overloads_consistency_2",
};

fn has_dec(decorators: &[String], name: &str) -> bool {
    decorators
        .iter()
        .any(|d| d == name || d.ends_with(&format!(".{name}")))
}

/// Emits `overloads_consistency_2` for decorator inconsistencies within an overload group.
pub(crate) struct OverloadDecoratorConsistency;

impl Rule for OverloadDecoratorConsistency {
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
        // Overload membership is a binding question ([#380]); the
        // staticmethod/final/override checks below stay spelling-based.
        let Some(resolver) = annotations else {
            return;
        };
        let mut groups: HashMap<(Option<&str>, &str), Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            groups
                .entry((func.class_name.as_deref(), func.name.as_str()))
                .or_default()
                .push(func);
        }

        for funcs in groups.values() {
            check_group(funcs, resolver, &module.path, diagnostics);
        }
    }
}

fn check_group(
    funcs: &[&FunctionInfo],
    resolver: &AnnotationResolver<'_>,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    let overloads: Vec<&&FunctionInfo> = funcs
        .iter()
        .filter(|f| overload_decorated(resolver, &f.decorators))
        .collect();
    if overloads.is_empty() {
        return;
    }
    let implementation = funcs
        .iter()
        .find(|f| !overload_decorated(resolver, &f.decorators));

    match implementation {
        // Group WITH an implementation: `@final`/`@override` belong on the
        // implementation, never on an overload signature.
        Some(impl_fn) => {
            check_static_class_consistency(&overloads, Some(impl_fn), out, path);
            check_impl_only_decorators(&overloads, out, path);
        }
        // Group WITHOUT an implementation is legal only in a stub (`.pyi`); there
        // `@final`/`@override` must appear on the FIRST overload only.
        None if is_stub(path) => {
            check_static_class_consistency(&overloads, None, out, path);
            check_first_overload_only_decorators(&overloads, out, path);
        }
        None => {}
    }
}

/// Stub files (`.pyi`) declare overloads without an implementation.
fn is_stub(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pyi"))
}

/// `@staticmethod` / `@classmethod` must be uniform across the whole group
/// (every overload plus the implementation, if present). With no implementation
/// the diagnostic is reported on the first overload.
fn check_static_class_consistency(
    overloads: &[&&FunctionInfo],
    impl_fn: Option<&FunctionInfo>,
    out: &mut Vec<Diagnostic>,
    path: &str,
) {
    let members = || {
        overloads
            .iter()
            .map(|f| &f.decorators)
            .chain(impl_fn.map(|f| &f.decorators))
    };
    let Some((owner_name, report_span)) = impl_fn
        .map(|f| (f.name.as_str(), f.name_span))
        .or_else(|| overloads.first().map(|f| (f.name.as_str(), f.name_span)))
    else {
        return;
    };
    for kind in ["staticmethod", "classmethod"] {
        let any = members().any(|d| has_dec(d, kind));
        let all = members().all(|d| has_dec(d, kind));
        if any && !all {
            out.push(make_diagnostic(
                format!(
                    "Inconsistent `@{kind}` across overloads of `{owner_name}`: it must be on \
                     every overload and the implementation, or none"
                ),
                report_span,
                path,
            ));
            return; // one decorator-consistency diagnostic per group is enough
        }
    }
}

/// In a stub the `@final`/`@override` decorator must be on the FIRST overload
/// only. Flag the first later overload that carries either decorator.
fn check_first_overload_only_decorators(
    overloads: &[&&FunctionInfo],
    out: &mut Vec<Diagnostic>,
    path: &str,
) {
    for overload in overloads.iter().skip(1) {
        for kind in ["final", "override"] {
            if let Some((_, span)) = overload
                .decorator_spans
                .iter()
                .find(|(name, _)| name == kind || name.ends_with(&format!(".{kind}")))
            {
                out.push(make_diagnostic(
                    format!(
                        "`@{kind}` on a later overload of `{}`: in a stub it must appear only on \
                         the first overload",
                        overload.name
                    ),
                    *span,
                    path,
                ));
                return; // one placement diagnostic per group is enough
            }
        }
    }
}

/// `@final` / `@override` belong on the implementation, not an overload signature.
fn check_impl_only_decorators(overloads: &[&&FunctionInfo], out: &mut Vec<Diagnostic>, path: &str) {
    for overload in overloads {
        for kind in ["final", "override"] {
            if let Some((_, span)) = overload
                .decorator_spans
                .iter()
                .find(|(name, _)| name == kind || name.ends_with(&format!(".{kind}")))
            {
                out.push(make_diagnostic(
                    format!(
                        "`@{kind}` on an `@overload` signature of `{}`: it should be applied only \
                         to the implementation",
                        overload.name
                    ),
                    *span,
                    path,
                ));
            }
        }
    }
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some("Move the decorator to the overload implementation, or apply it uniformly".to_owned()),
        Some("Type checkers require consistent decorators across an `@overload` group".to_owned()),
    )
}
