//! Implements [BSK-E0158] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-ownership
//! BSK-E0158: Inconsistent decorators across an overloaded method.
//!
//! The typing spec constrains how decorators may be spread across an
//! `@overload` group and its implementation:
//!
//! * If any signature is `@staticmethod` / `@classmethod`, *all* signatures and
//!   the implementation must carry the same decorator.
//! * `@final` and `@override` apply to the *implementation only* (or, in a stub,
//!   the first overload). Placing either on an `@overload` signature when an
//!   implementation is present is an error.
//!
//! These checks run only on groups that have a concrete implementation, so stub
//! declarations (which legitimately place `@final`/`@override` on the first
//! overload) are never flagged.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0158",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0158",
};

fn has_dec(decorators: &[String], name: &str) -> bool {
    decorators
        .iter()
        .any(|d| d == name || d.ends_with(&format!(".{name}")))
}

/// Emits BSK-E0158 for decorator inconsistencies within an overload group.
pub(crate) struct OverloadDecoratorConsistency;

impl Rule for OverloadDecoratorConsistency {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut groups: HashMap<(Option<&str>, &str), Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            groups
                .entry((func.class_name.as_deref(), func.name.as_str()))
                .or_default()
                .push(func);
        }

        for funcs in groups.values() {
            check_group(funcs, &module.path, diagnostics);
        }
    }
}

fn check_group(funcs: &[&FunctionInfo], path: &str, out: &mut Vec<Diagnostic>) {
    let overloads: Vec<&&FunctionInfo> = funcs
        .iter()
        .filter(|f| has_dec(&f.decorators, "overload"))
        .collect();
    let implementation = funcs.iter().find(|f| !has_dec(&f.decorators, "overload"));

    // Only meaningful for a real overload group WITH an implementation present.
    let (Some(impl_fn), false) = (implementation, overloads.is_empty()) else {
        return;
    };

    check_static_class_consistency(&overloads, impl_fn, out, path);
    check_impl_only_decorators(&overloads, out, path);
}

/// `@staticmethod` / `@classmethod` must be uniform across the whole group.
fn check_static_class_consistency(
    overloads: &[&&FunctionInfo],
    impl_fn: &FunctionInfo,
    out: &mut Vec<Diagnostic>,
    path: &str,
) {
    let members = || {
        overloads
            .iter()
            .map(|f| &f.decorators)
            .chain([&impl_fn.decorators])
    };
    for kind in ["staticmethod", "classmethod"] {
        let any = members().any(|d| has_dec(d, kind));
        let all = members().all(|d| has_dec(d, kind));
        if any && !all {
            out.push(make_diagnostic(
                format!(
                    "Inconsistent `@{kind}` across overloads of `{}`: it must be on every \
                     overload and the implementation, or none",
                    impl_fn.name
                ),
                impl_fn.name_span,
                path,
            ));
            return; // one decorator-consistency diagnostic per group is enough
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
