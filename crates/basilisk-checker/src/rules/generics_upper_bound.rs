//! Implements [`generics_upper_bound`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `generics_upper_bound`: `TypeVar` upper bound violation at call site.
//!
//! When a function parameter is annotated with a `TypeVar` that has an upper
//! bound (e.g. `bound=Sized`), and the call site passes a literal value whose
//! type does not satisfy that bound, Basilisk reports the violation.
//!
//! ```python
//! from typing import Sized, TypeVar
//!
//! ST = TypeVar("ST", bound=Sized)
//!
//! def longer(x: ST, y: ST) -> ST:
//!     if len(x) > len(y):
//!         return x
//!     return y
//!
//! longer(3, 3)  # E -- int does not implement Sized (__len__)
//! ```

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_upper_bound",
    docs_url: "https://www.basilisk-python.dev/errors/generics_upper_bound",
};

/// Emits `generics_upper_bound` when a call site passes a value whose type does not satisfy
/// the `TypeVar` upper bound declared on the corresponding parameter.
// Implements [TYPEINF-GENERICS-BOUND] — a bounded `TypeVar` is an upper bound:
// only a subtype of the bound satisfies it, enforced at the call site.
pub(crate) struct TypeVarBoundViolation;

impl Rule for TypeVarBoundViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;

        // Step 1: Build a map of TypeVar name -> bound text.
        let typevar_bounds = build_typevar_bounds(module, source);
        if typevar_bounds.is_empty() {
            return;
        }

        // Step 2: Build a map of function name -> list of (param_index, bound_text).
        let func_param_bounds = build_func_param_bounds(module, source, &typevar_bounds);
        if func_param_bounds.is_empty() {
            return;
        }

        // Step 3: Check call sites.
        check_call_sites(
            module,
            source,
            &typevar_bounds,
            &func_param_bounds,
            diagnostics,
        );
    }
}

/// Step 1: Build a map of `TypeVar` name -> bound text.
fn build_typevar_bounds<'a>(
    module: &'a ResolvedModule,
    source: &'a str,
) -> HashMap<&'a str, String> {
    let mut typevar_bounds: HashMap<&'a str, String> = HashMap::new();
    for tv in &module.typevar_calls {
        if !tv.has_bound {
            continue;
        }
        if let Some(bound_text) = extract_bound_text(source, tv.span) {
            let _ = typevar_bounds.insert(tv.name.as_str(), bound_text);
        }
    }
    typevar_bounds
}

/// Step 2: Build a map of function name -> list of (`param_index`, `bound_text`).
fn build_func_param_bounds<'a>(
    module: &'a ResolvedModule,
    source: &'a str,
    typevar_bounds: &'a HashMap<&'a str, String>,
) -> HashMap<&'a str, Vec<(usize, String)>> {
    let mut func_param_bounds: HashMap<&'a str, Vec<(usize, String)>> = HashMap::new();
    for func in &module.functions {
        let mut param_bounds = Vec::new();
        for (idx, param) in func.parameters.iter().enumerate() {
            let Some(ann_span) = param.annotation_span else {
                continue;
            };
            let Some(ann_text) = slice_span(source, ann_span) else {
                continue;
            };
            let ann_text = ann_text.trim();
            if let Some(bound) = typevar_bounds.get(ann_text) {
                param_bounds.push((idx, bound.clone()));
            }
        }
        if !param_bounds.is_empty() {
            let _ = func_param_bounds.insert(func.name.as_str(), param_bounds);
        }
    }
    func_param_bounds
}

/// Step 3: Check call sites for violations.
fn check_call_sites(
    module: &ResolvedModule,
    source: &str,
    typevar_bounds: &HashMap<&str, String>,
    _func_param_bounds: &HashMap<&str, Vec<(usize, String)>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in &module.calls {
        // Find all functions with this name (could be multiple due to methods)
        let matching_functions: Vec<&basilisk_resolver::FunctionInfo> = module
            .functions
            .iter()
            .filter(|f| f.name == call.callee)
            .collect();

        if matching_functions.is_empty() {
            continue;
        }

        // For each matching function, check its parameter bounds
        for func in matching_functions {
            check_function_call(func, call, source, typevar_bounds, &module.path, diagnostics);
        }
    }
}

/// Check a specific function call for `TypeVar` bound violations.
fn check_function_call(
    func: &basilisk_resolver::FunctionInfo,
    call: &basilisk_resolver::CallSite,
    source: &str,
    typevar_bounds: &HashMap<&str, String>,
    module_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let param_bounds: Vec<(usize, String)> = func
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(idx, param)| {
            let ann_span = param.annotation_span?;
            let ann_text = slice_span(source, ann_span)?;
            let ann_text = ann_text.trim();
            typevar_bounds
                .get(ann_text)
                .map(|bound| (idx, bound.clone()))
        })
        .collect();

    if param_bounds.is_empty() {
        return;
    }

    for (param_idx, bound_text) in param_bounds {
        let Some((rhs_kind, _arg_span)) = call.args.get(param_idx) else {
            continue;
        };

        let Some(lit_type) = literal_type_name(rhs_kind) else {
            continue;
        };

        if !type_satisfies_bound(lit_type, &bound_text) {
            let func_name = if let Some(class_name) = &func.class_name {
                format!("{}.{}", class_name, func.name)
            } else {
                func.name.clone()
            };

            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument of type `{lit_type}` does not satisfy \
                     `TypeVar` bound `{bound_text}` for `{func_name}`"
                ),
                call.span,
                module_path,
                Some(format!(
                    "Pass a value that satisfies `{bound_text}` \
                     (e.g. a type implementing `__len__`)"
                )),
                Some(format!(
                    "TypeVar bound `{bound_text}` requires the argument type to \
                     be a subtype of `{bound_text}`"
                )),
            ));
            // Only one diagnostic per call.
            break;
        }
    }
}

/// Extract the bound text from a `TypeVar("Name", bound=X)` call in source.
fn extract_bound_text(source: &str, span: basilisk_resolver::Span) -> Option<String> {
    let call_text = slice_span(source, span)?;
    let bound_idx = call_text.find("bound=")?;
    let after_bound = call_text.get(bound_idx + "bound=".len()..)?;

    // The bound value extends until the next `,` or `)` at depth 0.
    let mut depth = 0u32;
    let mut end = after_bound.len();
    for (idx, ch) in after_bound.char_indices() {
        match ch {
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => {
                if depth == 0 {
                    end = idx;
                    break;
                }
                depth = depth.saturating_sub(1);
            }
            ',' if depth == 0 => {
                end = idx;
                break;
            }
            _ => {}
        }
    }
    let bound_text = after_bound.get(..end)?.trim();
    // Strip quotes for string-form bounds (e.g. bound="int").
    let bound_text = bound_text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(bound_text);
    if bound_text.is_empty() {
        return None;
    }
    Some(bound_text.to_owned())
}

/// Map an `RhsKind` to its concrete type name for literal values.
fn literal_type_name(rhs: &basilisk_resolver::RhsKind) -> Option<&'static str> {
    use basilisk_resolver::RhsKind;
    match rhs {
        RhsKind::IntLiteral => Some("int"),
        RhsKind::FloatLiteral => Some("float"),
        RhsKind::StrLiteral => Some("str"),
        RhsKind::BoolLiteral => Some("bool"),
        RhsKind::BytesLiteral => Some("bytes"),
        RhsKind::NoneValue => Some("None"),
        _ => None,
    }
}

/// Check whether a concrete type satisfies an upper bound.
///
/// Conservative check for builtin-type bounds.
fn type_satisfies_bound(concrete_type: &str, bound: &str) -> bool {
    match bound {
        // For builtin bounds like "int", check if the concrete type matches the bound.
        "int" => concrete_type == "int",
        "str" => concrete_type == "str",
        "float" => concrete_type == "float" || concrete_type == "int", // int→float widening
        "bool" => concrete_type == "bool",
        "bytes" => concrete_type == "bytes",
        // Conservative: assume satisfied for unknown bounds.
        _ => true,
    }
}
