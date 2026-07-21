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

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, VariableInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::rules::shared::split_top_level_commas;
use crate::span_util::slice_span;

use super::{extract_annotation, CODE};

/// Check module-level and function-local annotated variables for
/// `x: type[C[Args]] = C` assignments where a defaulted type parameter's
/// default conflicts with the requested specialization.
pub(super) fn check_default_specializations(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let defaults = typevar_defaults(module);
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
        check_var(var, module, &defaults, diagnostics);
    }
}

/// Map from `TypeVar` name to its `default=` type name, for typevars that
/// declare a simple default (e.g. `TypeVar("DefaultStrT", default=str)`).
fn typevar_defaults(module: &ResolvedModule) -> HashMap<&str, &str> {
    module
        .typevar_calls
        .iter()
        .filter_map(|tv| {
            tv.default_type_name
                .as_deref()
                .map(|default| (tv.name.as_str(), default))
        })
        .collect()
}

/// Check one annotated variable for a default-specialization mismatch.
fn check_var(
    var: &VariableInfo,
    module: &ResolvedModule,
    defaults: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !var.has_annotation {
        return;
    }
    let Some(rhs_span) = var.rhs_span else {
        return;
    };
    let Some(rhs_text) = slice_span(&module.source, rhs_span) else {
        return;
    };
    let rhs_name = rhs_text.trim();
    if !is_identifier(rhs_name) {
        return;
    }

    let Some(annotation) = extract_annotation(&module.source, var.name_span) else {
        return;
    };
    let Some((class_name, type_args)) = parse_type_of_subscript(annotation) else {
        return;
    };
    if class_name != rhs_name {
        return;
    }

    let Some(class_info) = module.classes.iter().find(|c| c.name == class_name) else {
        return;
    };
    let free_params = free_type_params(class_info, module);

    for (idx, arg) in type_args.iter().enumerate() {
        let Some(param_name) = free_params.get(idx) else {
            break;
        };
        let Some(default) = defaults.get(param_name.as_str()) else {
            continue;
        };
        if is_identifier(arg) && arg != default {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Type mismatch: `{}` is annotated `{annotation}` but assigned bare \
                     `{class_name}`, whose type parameter `{param_name}` defaults to `{default}`",
                    var.name
                ),
                var.name_span,
                &module.path,
                Some(format!(
                    "Subscript the right-hand side explicitly (`{class_name}[{arg}]`) or \
                     change the annotation to `type[{class_name}[{default}]]`"
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

/// Parse `type[C[A1, A2, ...]]` into `("C", ["A1", "A2", ...])`.
fn parse_type_of_subscript(annotation: &str) -> Option<(&str, Vec<&str>)> {
    let inner = annotation
        .trim()
        .strip_prefix("type[")?
        .strip_suffix(']')?
        .trim();
    let bracket = inner.find('[')?;
    let class_name = inner.get(..bracket)?.trim();
    let args_text = inner.get(bracket + 1..)?.strip_suffix(']')?;
    let args = split_top_level_commas(args_text);
    if args.is_empty() || !is_identifier(class_name) {
        return None;
    }
    Some((class_name, args.into_iter().map(str::trim).collect()))
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

/// `true` when `text` is a plain Python identifier (no subscripts, dots, etc.).
fn is_identifier(text: &str) -> bool {
    !text.is_empty()
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !text.starts_with(|c: char| c.is_ascii_digit())
}
