//! Implements [`qualifiers_final_decorator`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `qualifiers_final_decorator`: `@final` decorator violations.
//!
//! Two violations are detected:
//!
//! 1. **Inheriting from a `@final` class** — a class decorated with `@final`
//!    cannot be subclassed.
//!
//! 2. **Overriding a `@final` method** — a method decorated with `@final`
//!    in a base class cannot be overridden in a subclass.
//!
//! [PEP 591](https://peps.python.org/pep-0591/). Every decision here is made
//! on resolved identity: `@final` is recognised by the decorator expression
//! resolving to `typing.final` (so `@typing.final` and `from typing import
//! final as sealed` behave identically, and a local `def final` does not
//! answer at all), and a base class is the class its expression RESOLVES to.

use std::collections::HashMap;

use basilisk_resolver::{ClassGraph, ClassInfo, FunctionInfo, ResolvedBase, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "qualifiers_final_decorator",
    docs_url: "https://www.basilisk-python.dev/errors/qualifiers_final_decorator",
};

/// Emits `qualifiers_final_decorator` for `@final` decorator violations.
pub(crate) struct FinalViolation;

// ##################################################################
// # REBUILT ON DEFINITION-SITE IDENTITY.                           #
// #                                                                #
// # The deleted body decided PEP 591 `@final` inheritance with     #
// # `class_map.get(base_name.as_str())`, and `@final` methods with #
// # `imported_final_methods.get(base_name.as_str())`.              #
// #                                                                #
// # `ClassInfo::bases` holds RENDERED SIMPLE NAMES ("complex       #
// # expressions ignored") and the lookup map was keyed on          #
// # `ClassInfo::name`, so an aliased base MISSED — silently        #
// # permitting `Sealed = Locked; class Escape(Sealed)` — a dotted  #
// # base collided with any local class sharing its trailing word,  #
// # and two classes with one rendered name were a single entry.    #
// #                                                                #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs     #
// ##################################################################
impl Rule for FinalViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let graph = ClassGraph::new(&module.classes);
        let final_methods = final_methods_by_owner(&module.functions);

        for class in &module.classes {
            check_final_base(&graph, class, &module.path, diagnostics);
        }
        for method in &module.functions {
            check_final_override(&graph, method, &final_methods, &module.path, diagnostics);
        }
    }
}

/// Every `@final` method in the module, keyed by its owning class's DEFINITION
/// SITE and its own name.
fn final_methods_by_owner(functions: &[FunctionInfo]) -> HashMap<(Span, &str), &FunctionInfo> {
    functions
        .iter()
        .filter(|func| func.is_final)
        .filter_map(|func| Some(((func.class_site?, func.name.as_str()), func)))
        .collect()
}

/// A class may not name a `@final` class among its bases.
fn check_final_base(
    graph: &ClassGraph<'_>,
    class: &ClassInfo,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    for base in &class.resolved_bases {
        let ResolvedBase::LocalClass(site) = base.resolved else {
            // A base from another module: this module cannot see whether it is
            // `@final`, and must not guess ([CHKARCH-CONFORMANCE-MODE]).
            continue;
        };
        let Some(base_class) = graph.at(site) else {
            continue;
        };
        if !base_class.is_final {
            continue;
        }
        out.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`{}` cannot inherit from `{}`, which is decorated with `@final`",
                class.name, base_class.name
            ),
            base.span,
            path,
            Some(format!(
                "Remove `@final` from `{}` or stop subclassing it",
                base_class.name
            )),
            Some("PEP 591: a class decorated with `@final` cannot be subclassed".to_owned()),
        ));
    }
}

/// A method may not redefine one an ancestor of its class declared `@final`.
fn check_final_override(
    graph: &ClassGraph<'_>,
    method: &FunctionInfo,
    final_methods: &HashMap<(Span, &str), &FunctionInfo>,
    path: &str,
    out: &mut Vec<Diagnostic>,
) {
    // Not a method, or its own class is not one this module defines.
    let Some(owner) = method.class_site.and_then(|site| graph.at(site)) else {
        return;
    };
    // A `@final` method IS the declaration, not an override of one.
    if method.is_final {
        return;
    }
    // `ancestors` yields `owner` first; a class does not override its own
    // method, so its own declarations are not candidates.
    for ancestor in graph
        .ancestors(owner)
        .into_iter()
        .filter(|ancestor| ancestor.name_span != owner.name_span)
    {
        let Some(sealed) = final_methods.get(&(ancestor.name_span, method.name.as_str())) else {
            continue;
        };
        out.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`{}.{}` overrides `{}.{}`, which is decorated with `@final`",
                owner.name, method.name, ancestor.name, sealed.name
            ),
            method.def_span,
            path,
            Some(format!(
                "Remove `@final` from `{}.{}` or do not override it",
                ancestor.name, sealed.name
            )),
            Some(
                "PEP 591: a method decorated with `@final` cannot be overridden in a \
                 subclass"
                    .to_owned(),
            ),
        ));
        return;
    }
}
