//! Implements [`overloads_consistency_3`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `overloads_consistency_3`: Overload implementation is inconsistent with its signatures.
//!
//! When an overload implementation is present the spec requires
//! (<https://typing.python.org/en/latest/spec/overload.html#implementation-consistency>):
//!   * the return type of every overload is assignable to the implementation's
//!     return type, and
//!   * the implementation's parameter types are assignable *from* every
//!     overload's parameter types (the implementation must accept them all).
//!
//! Both annotations are lowered through the module's binding table to
//! [`TypeNode`] and related with [`assignable`] ([ASTREBUILD-LAW]). The
//! relation abstains (`None`) on anything it does not model — `TypeVar`s,
//! user classes, callables — so a diagnostic is emitted only on a proven
//! `Some(false)`, never from the spelling of an annotation.

use std::collections::HashMap;

use basilisk_resolver::{assignable, FunctionInfo, ParameterInfo, ResolvedModule, Span, TypeNode};

use crate::rules::shared::{parse_module, ExprIndex};
use crate::span_util::node_message_text;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "overloads_consistency_3",
    docs_url: "https://www.basilisk-python.dev/errors/overloads_consistency_3",
};

/// `true` if a decorator only conveys typing intent and leaves the call
/// signature unchanged. Any *other* decorator may transform the effective
/// signature (the spec applies such transforms before consistency checks), so a
/// group carrying one cannot be compared by its declared annotations.
//
// ##########################################################################
// # DELETED BODY — `is_type_only_decorator`. DO NOT RESTORE IT AND DO NOT  #
// # RETURN EITHER ANSWER UNCONDITIONALLY.                                  #
// #                                                                        #
// #   matches!(decorator.rsplit('.').next().unwrap_or(decorator),          #
// #            "staticmethod" | "classmethod" | "property")                #
// #                                                                        #
// # `rsplit('.').next()` discards the qualifier, so ANY decorator whose    #
// # last component happens to be one of those three words was treated as   #
// # signature-preserving — `mylib.property`, `attrs.staticmethod`,         #
// # `self.classmethod`, a `Protocol` member named `property`. In the other #
// # direction, `functools.cached_property` is not `property` and neither   #
// # is an aliased `from builtins import property as prop`.                 #
// #                                                                        #
// # This decides whether a whole overload group can be compared by its     #
// # declared annotations at all, so a wrong answer here does not merely    #
// # miss a diagnostic — it compares transformed signatures as if they were #
// # untransformed and reports differences the spec does not have.          #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn is_type_only_decorator(_decorator: &str) -> bool {
    panic!(
        "basilisk-checker: `is_type_only_decorator` was DELETED because it recognised \
         `staticmethod`/`classmethod`/`property` by the LAST DOTTED COMPONENT of the \
         decorator's rendered text, so any unrelated decorator ending in one of those \
         words qualified and every aliased import of the real ones did not. It panics \
         because the real implementation — resolving the decorator expression through \
         the binding table — DOES NOT EXIST YET. Do not restore the trailing-word test \
         and do not pick a constant answer in its place."
    )
}

/// `true` if any member of the group is `async` or carries a signature-
/// transforming decorator, in which case declared-annotation comparison is
/// invalid.
fn group_is_transformed(funcs: &[&FunctionInfo]) -> bool {
    funcs
        .iter()
        .any(|f| f.is_async || !f.decorators.iter().all(|d| is_type_only_decorator(d)))
}

/// Parameters with a leading `self`/`cls` removed.
fn non_self_params(params: &[ParameterInfo]) -> &[ParameterInfo] {
    match params.first() {
        Some(p) if p.name == "self" || p.name == "cls" => params.get(1..).unwrap_or_default(),
        _ => params,
    }
}

/// Emits `overloads_consistency_3` for overload/implementation signature inconsistencies.
pub(crate) struct OverloadImplConsistency;

impl Rule for OverloadImplConsistency {
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
        let Some(parsed) = parse_module(module) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);
        let mut groups: HashMap<(Option<&str>, &str), Vec<&FunctionInfo>> = HashMap::new();
        for func in &module.functions {
            groups
                .entry((func.class_name.as_deref(), func.name.as_str()))
                .or_default()
                .push(func);
        }
        for funcs in groups.values() {
            check_group(funcs, module, &index, diagnostics);
        }
    }
}

fn check_group(
    funcs: &[&FunctionInfo],
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    out: &mut Vec<Diagnostic>,
) {
    let overloads: Vec<&&FunctionInfo> = funcs.iter().filter(|f| f.is_overload).collect();
    let Some(impl_fn) = funcs.iter().find(|f| !f.is_overload) else {
        return;
    };
    if overloads.len() < 2 || group_is_transformed(funcs) {
        return;
    }

    let impl_ret_expr = impl_fn
        .return_annotation_span
        .and_then(|span| index.expr(span));
    let impl_params = non_self_params(&impl_fn.parameters);
    let impl_has_varargs = impl_fn.vararg.is_some() || impl_fn.kwarg.is_some();

    for overload in &overloads {
        // (1) Return: each overload's return must be assignable to the impl's.
        let over_ret_expr = overload
            .return_annotation_span
            .and_then(|span| index.expr(span));
        if let (Some(over_expr), Some(impl_expr)) = (over_ret_expr, impl_ret_expr) {
            let over_node = TypeNode::lower(&module.bindings, over_expr);
            let impl_node = TypeNode::lower(&module.bindings, impl_expr);
            if assignable(&over_node, &impl_node) == Some(false) {
                // Source text appears in the MESSAGE only, never in the verdict.
                let over_text = node_message_text(&module.source, over_expr);
                let impl_text = node_message_text(&module.source, impl_expr);
                out.push(make_diagnostic(
                    format!(
                        "Overload of `{}` returns `{over_text}`, which is not assignable to the \
                         implementation's return type `{impl_text}`",
                        overload.name
                    ),
                    overload.name_span,
                    &module.path,
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
        if let Some(span) =
            param_inconsistency(over_params, impl_params, module, index, overload.name_span)
        {
            out.push(make_diagnostic(
                format!(
                    "An overload of `{}` has a parameter type the implementation cannot accept",
                    overload.name
                ),
                span,
                &module.path,
            ));
        }
    }
}

/// First positional parameter whose overload type is provably not assignable
/// to the implementation's corresponding parameter type. Undecidable pairs
/// abstain.
fn param_inconsistency(
    over_params: &[ParameterInfo],
    impl_params: &[ParameterInfo],
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    span: Span,
) -> Option<Span> {
    over_params
        .iter()
        .zip(impl_params)
        .any(|(over_param, impl_param)| {
            let (Some(over_span), Some(impl_span)) =
                (over_param.annotation_span, impl_param.annotation_span)
            else {
                return false;
            };
            let (Some(over_expr), Some(impl_expr)) = (index.expr(over_span), index.expr(impl_span))
            else {
                return false;
            };
            let over_node = TypeNode::lower(&module.bindings, over_expr);
            let impl_node = TypeNode::lower(&module.bindings, impl_expr);
            assignable(&over_node, &impl_node) == Some(false)
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
