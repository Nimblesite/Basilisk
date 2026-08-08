//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! PEP 696 default-specialization mismatch for bare generic class assignments.
//!
//! A bare reference to a generic class whose remaining free type parameters
//! all carry PEP 696 defaults is equivalent to the class specialized with
//! those defaults.  Assigning the bare class to a `type[C[Arg]]` annotation
//! is therefore an error when `Arg` differs from the parameter's default:
//!
//! ```python
//! class SubclassMe(Generic[T1, DefaultStrT]): ...
//! class Bar(SubclassMe[int, DefaultStrT]): ...
//!
//! x1: type[Bar[str]] = Bar  # OK  — DefaultStrT defaults to str
//! x2: type[Bar[int]] = Bar  # E   — bare Bar specializes to Bar[str]
//! ```
//!
//! Every verdict is computed on resolved AST nodes ([ASTREBUILD-LAW]): the
//! annotation is destructured structurally (its `type` base recognised by
//! what it RESOLVES to), and the requested argument is related to the
//! `TypeVar`'s lowered default through [`equivalent`]
//! ([RESOLV-CANONICAL-RELATION]).  A diagnostic is emitted only on a
//! definite `Some(false)`; unresolvable nodes abstain.

use std::collections::HashMap;

use basilisk_resolver::{
    equivalent, BuiltinClass, ResolvedModule, TypeNode, TypeVarCallInfo, VariableInfo,
};
use ruff_python_ast::{Expr, ExprName};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::{parse_module, ExprIndex};
use crate::span_util::slice_span;

use super::CODE;

/// Check module-level and function-local annotated variables for
/// `x: type[C[Args]] = C` assignments where a defaulted type parameter's
/// default conflicts with the requested specialization.
pub(super) fn check_default_specializations(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(parsed) = parse_module(module) else {
        return;
    };
    let index = ExprIndex::build(&parsed.ast);
    let defaults = typevar_defaults(module, &index);
    if defaults.is_empty() {
        return;
    }

    let vars = module.module_vars.iter().chain(
        module
            .functions
            .iter()
            .flat_map(|func| func.local_vars.iter()),
    );
    for var in vars {
        check_var(var, module, &index, &defaults, diagnostics);
    }
}

/// Map from `TypeVar` name to its resolved `default=` type (PEP 696) plus
/// the default's recorded name (used only in diagnostic text), for typevars
/// that declare a simple default (e.g. `TypeVar("DefaultStrT", default=str)`).
fn typevar_defaults<'m>(
    module: &'m ResolvedModule,
    index: &ExprIndex<'_>,
) -> HashMap<&'m str, (TypeNode, &'m str)> {
    module
        .typevar_calls
        .iter()
        .filter_map(|tv| {
            let display = tv.default_type_name.as_deref()?;
            let node = default_node(tv, index, module)?;
            Some((tv.name.as_str(), (node, display)))
        })
        .collect()
}

/// The lowered `default=` argument of a recorded `TypeVar(...)` call.  The
/// expression is found on the call NODE and lowered through the module's
/// bindings — never read back from source text ([ASTREBUILD-LAW]).
fn default_node(
    tv: &TypeVarCallInfo,
    index: &ExprIndex<'_>,
    module: &ResolvedModule,
) -> Option<TypeNode> {
    let Expr::Call(call) = index.expr(tv.span)? else {
        return None;
    };
    let default = call
        .arguments
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|arg| arg.as_str() == "default"))?;
    Some(TypeNode::lower(&module.bindings, &default.value))
}

/// Check one annotated variable for a default-specialization mismatch.
fn check_var(
    var: &VariableInfo,
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    defaults: &HashMap<&str, (TypeNode, &str)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !var.has_annotation {
        return;
    }
    let Some(Expr::Name(rhs)) = var.rhs_span.and_then(|span| index.expr(span)) else {
        return;
    };
    let Some(annotation_span) = var.annotation_span else {
        return;
    };
    let Some(annotation) = index.expr(annotation_span) else {
        return;
    };
    let Some((class_ref, type_args)) = type_of_subscript(annotation, module) else {
        return;
    };
    if class_ref.id.as_str() != rhs.id.as_str() {
        return;
    }

    let Some(class_info) = module
        .classes
        .iter()
        .find(|c| c.name == class_ref.id.as_str())
    else {
        return;
    };
    let free_params = free_type_params(class_info, module);

    for (idx, arg) in type_args.iter().enumerate() {
        let Some(param_name) = free_params.get(idx) else {
            break;
        };
        let Some((default, default_name)) = defaults.get(param_name.as_str()) else {
            continue;
        };
        let arg_node = TypeNode::lower(&module.bindings, arg);
        // A diagnostic only on a definite mismatch between the requested
        // argument and the parameter's resolved default; `None` (either node
        // unresolvable) abstains ([ASTREBUILD-LAW]).
        if equivalent(&arg_node, default) == Some(false) {
            let annotation_text = slice_span(&module.source, annotation_span).unwrap_or("");
            let class_name = class_ref.id.as_str();
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Type mismatch: `{}` is annotated `{annotation_text}` but assigned bare \
                     `{class_name}`, whose type parameter `{param_name}` defaults to \
                     `{default_name}`",
                    var.name
                ),
                var.name_span,
                &module.path,
                Some(format!(
                    "Subscript the right-hand side explicitly or change the annotation to \
                     `type[{class_name}[{default_name}]]`"
                )),
                Some(
                    "A bare generic class is equivalent to the class specialized with its \
                     type-parameter defaults (PEP 696)"
                        .to_owned(),
                ),
            ));
            return;
        }
    }
}

/// Destructure a `type[C[args…]]` annotation NODE: the outer base must
/// denote the builtin `type` — recognised by LOWERING it through the
/// module's bindings, so `typing.Type`, an aliased import, or any other
/// spelling behaves identically ([ASTREBUILD-LAW]) — the inner base must be
/// a plain name, and the returned args are the inner subscript's elements.
fn type_of_subscript<'e>(
    annotation: &'e Expr,
    module: &ResolvedModule,
) -> Option<(&'e ExprName, Vec<&'e Expr>)> {
    let Expr::Subscript(outer) = annotation else {
        return None;
    };
    if TypeNode::lower(&module.bindings, &outer.value) != TypeNode::Builtin(BuiltinClass::Type) {
        return None;
    }
    let Expr::Subscript(inner) = outer.slice.as_ref() else {
        return None;
    };
    let Expr::Name(class_ref) = inner.value.as_ref() else {
        return None;
    };
    let args = match inner.slice.as_ref() {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        single => vec![single],
    };
    Some((class_ref, args))
}

/// The class's free type parameters, in declaration order.
///
/// `class C(Generic[T1, T2])` declares them directly; `class Bar(Base[int, T])`
/// inherits the typevars referenced in its base subscripts.
fn free_type_params(
    class_info: &basilisk_resolver::ClassInfo,
    module: &ResolvedModule,
) -> Vec<String> {
    if !class_info.generic_params.is_empty() {
        return class_info
            .generic_params
            .iter()
            .map(|p| p.name.clone())
            .collect();
    }
    let typevar_names: std::collections::HashSet<&str> =
        basilisk_resolver::collect_name_set(&module.typevar_calls);
    let mut seen = std::collections::HashSet::new();
    class_info
        .base_subscripts
        .iter()
        .flat_map(|base| base.type_arg_names.iter())
        .filter(|name| typevar_names.contains(name.as_str()))
        .filter(|name| seen.insert(name.as_str()))
        .cloned()
        .collect()
}
