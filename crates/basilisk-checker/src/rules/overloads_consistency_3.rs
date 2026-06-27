//! Implements [overloads_consistency_3] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! overloads_consistency_3: Overload implementation is inconsistent with its signatures.
//!
//! When an overload implementation is present the spec requires:
//!   * the return type of every overload is assignable to the implementation's
//!     return type, and
//!   * the implementation's parameter types are assignable *from* every
//!     overload's parameter types (the implementation must accept them all).
//!
//! To remain false-positive free this only compares **known primitive types**
//! (`int`/`str`/`bytes`/`float`/`bool`/`complex`/`object`/`None` and unions of
//! them). Any `TypeVar`, generic (`list[int]`), `Callable`, or otherwise
//! non-primitive annotation is skipped, since text-level assignability cannot be
//! decided for it.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule, Span};

use crate::rules::shared::is_type_compatible;
use crate::span_util::slice_span;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "overloads_consistency_3",
    docs_url: "https://www.basilisk-python.dev/errors/overloads_consistency_3",
};

fn has_overload(decorators: &[String]) -> bool {
    decorators
        .iter()
        .any(|d| d == "overload" || d.ends_with(".overload"))
}

/// `true` if a decorator only conveys typing intent and leaves the call
/// signature unchanged. Any *other* decorator may transform the effective
/// signature (the spec applies such transforms before consistency checks), so a
/// group carrying one cannot be compared by raw annotation text.
fn is_type_only_decorator(decorator: &str) -> bool {
    matches!(
        decorator.rsplit('.').next().unwrap_or(decorator),
        "overload"
            | "final"
            | "override"
            | "staticmethod"
            | "classmethod"
            | "abstractmethod"
            | "property"
    )
}

/// `true` if any member of the group is `async` or carries a signature-
/// transforming decorator, in which case raw annotation comparison is invalid.
fn group_is_transformed(funcs: &[&FunctionInfo]) -> bool {
    funcs
        .iter()
        .any(|f| f.is_async || !f.decorators.iter().all(|d| is_type_only_decorator(d)))
}

/// `true` when every `|`-separated part of `text` is a known primitive type, so
/// `is_type_compatible` can decide assignability with confidence.
fn is_known_primitive(text: &str) -> bool {
    text.split('|').map(str::trim).all(|part| {
        matches!(
            part,
            "int" | "str" | "bytes" | "float" | "bool" | "complex" | "object" | "None"
        )
    })
}

/// The return annotation text of `func`, if present.
fn return_text(func: &FunctionInfo, source: &str) -> Option<String> {
    func.return_annotation_span
        .and_then(|span| slice_span(source, span))
        .map(|text| text.trim().to_owned())
}

/// Parameters with a leading `self`/`cls` removed.
fn non_self_params(params: &[ParameterInfo]) -> &[ParameterInfo] {
    match params.first() {
        Some(p) if p.name == "self" || p.name == "cls" => params.get(1..).unwrap_or_default(),
        _ => params,
    }
}

/// Emits overloads_consistency_3 for overload/implementation signature inconsistencies.
pub(crate) struct OverloadImplConsistency;

impl Rule for OverloadImplConsistency {
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
            check_group(funcs, &module.source, &module.path, diagnostics);
        }
    }
}

fn check_group(funcs: &[&FunctionInfo], source: &str, path: &str, out: &mut Vec<Diagnostic>) {
    let overloads: Vec<&&FunctionInfo> = funcs
        .iter()
        .filter(|f| has_overload(&f.decorators))
        .collect();
    let Some(impl_fn) = funcs.iter().find(|f| !has_overload(&f.decorators)) else {
        return;
    };
    if overloads.len() < 2 || group_is_transformed(funcs) {
        return;
    }

    let impl_ret = return_text(impl_fn, source);
    let impl_params = non_self_params(&impl_fn.parameters);
    let impl_has_varargs = impl_fn.vararg.is_some() || impl_fn.kwarg.is_some();

    for overload in &overloads {
        // (1) Return: each overload's return must be assignable to the impl's.
        if let (Some(over_ret), Some(impl_ret)) =
            (return_text(overload, source), impl_ret.as_deref())
        {
            if is_known_primitive(&over_ret)
                && is_known_primitive(impl_ret)
                && !is_type_compatible(&over_ret, impl_ret)
            {
                out.push(make_diagnostic(
                    format!(
                        "Overload of `{}` returns `{over_ret}`, which is not assignable to the \
                         implementation's return type `{impl_ret}`",
                        overload.name
                    ),
                    overload.name_span,
                    path,
                ));
                continue; // one diagnostic per overload is enough
            }
        }

        // (2) Parameters: the impl must accept every overload's parameter type.
        if impl_has_varargs {
            continue;
        }
        let over_params = non_self_params(&overload.parameters);
        if over_params.len() != impl_params.len() {
            continue;
        }
        if let Some(span) = param_inconsistency(over_params, impl_params, overload.name_span) {
            out.push(make_diagnostic(
                format!(
                    "An overload of `{}` has a parameter type the implementation cannot accept",
                    overload.name
                ),
                span,
                path,
            ));
        }
    }
}

/// First positional parameter whose overload type is a known primitive not
/// assignable to the implementation's corresponding primitive type.
fn param_inconsistency(
    over_params: &[ParameterInfo],
    impl_params: &[ParameterInfo],
    span: Span,
) -> Option<Span> {
    over_params
        .iter()
        .zip(impl_params)
        .any(|(op, ip)| {
            matches!(
                (op.annotation_text.as_deref(), ip.annotation_text.as_deref()),
                (Some(o), Some(i))
                    if is_known_primitive(o) && is_known_primitive(i) && !is_type_compatible(o, i)
            )
        })
        .then_some(span)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message,
        span,
        path,
        Some("Widen the implementation signature, or fix the inconsistent overload".to_owned()),
        Some(
            "An overload implementation must accept all overload inputs and produce all overload \
             outputs"
                .to_owned(),
        ),
    )
}
