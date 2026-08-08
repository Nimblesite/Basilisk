//! Implements [`overloads_consistency_2`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `overloads_consistency_2`: Inconsistent decorators across an overloaded method.
//!
//! The typing spec constrains how decorators may be spread across an
//! `@overload` group and its implementation:
//!
//! * If any signature is `@staticmethod` / `@classmethod`, *all* signatures and
//!   the implementation must carry the same decorator.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

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
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Bail on parse errors — those are reported separately as BSK-0000.
        if types.annotations().is_none() {
            return;
        }
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
    let overloads: Vec<&&FunctionInfo> = funcs.iter().filter(|f| f.is_overload).collect();
    if overloads.is_empty() {
        return;
    }
    let implementation = funcs.iter().find(|f| !f.is_overload);

    match implementation {
        // Group WITH an implementation.
        Some(impl_fn) => {
            check_static_class_consistency(&overloads, Some(impl_fn), out, path);
        }
        // Group WITHOUT an implementation is legal only in a stub (`.pyi`).
        None if is_stub(path) => {
            check_static_class_consistency(&overloads, None, out, path);
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
