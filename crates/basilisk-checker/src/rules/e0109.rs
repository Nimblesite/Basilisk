//! Implements [BSK-E0109] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0109: `TypeVar` bound violation at call site.
//!
//! When a function has a parameter typed with a `TypeVar` that has a `bound`,
//! and a call passes an argument whose type is not a subtype of that bound,
//! this rule reports the mismatch.
//!
//! ```python
//! TLiteral = TypeVar("TLiteral", bound=LiteralString)
//!
//! def literal_identity(s: TLiteral) -> TLiteral:
//!     return s
//!
//! def func5(s: str):
//!     literal_identity(s)  # E — str is not a subtype of LiteralString
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0109",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0109",
};

/// Emits BSK-E0109 when a call-site argument type violates a `TypeVar`'s bound.
pub(crate) struct TypeVarBoundCallViolation;

/// Map of bound type → list of types that are NOT subtypes of that bound.
/// This is a conservative list — we only flag cases we're sure about.
fn bound_incompatible(bound: &str, arg_type: &str) -> bool {
    match bound {
        "LiteralString" => arg_type == "str",
        _ => false,
    }
}

impl Rule for TypeVarBoundCallViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;

        // Build map: TypeVar name → bound type name
        let typevar_bounds: HashMap<&str, &str> = module
            .typevar_calls
            .iter()
            .filter_map(|tv| {
                tv.bound_type_name
                    .as_ref()
                    .map(|bound| (tv.name.as_str(), bound.as_str()))
            })
            .collect();

        if typevar_bounds.is_empty() {
            return;
        }

        // Group module-level functions by name
        let mut func_map: HashMap<&str, &FunctionInfo> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_none() {
                let _ = func_map.insert(func.name.as_str(), func);
            }
        }

        // For each function, build: param name → enclosing annotation text
        // We need this to know the type of arguments at call sites.

        for call in &module.calls {
            let Some(callee_func) = func_map.get(call.callee.as_str()) else {
                continue;
            };

            for (arg_idx, (_rhs_kind, arg_span)) in call.args.iter().enumerate() {
                let Some(param) = callee_func.parameters.get(arg_idx) else {
                    break;
                };

                // Get the parameter's annotation text
                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann_text) = slice_span(source, ann_span) else {
                    continue;
                };
                let ann_trimmed = ann_text.trim();

                // Check if the annotation is a TypeVar with a bound
                let Some(&bound_type) = typevar_bounds.get(ann_trimmed) else {
                    continue;
                };

                // Get the argument source text to identify what's being passed
                let Some(arg_text) = slice_span(source, *arg_span) else {
                    continue;
                };
                let arg_name = arg_text.trim();

                // Try to find the type of the argument by looking at:
                // 1. Enclosing function parameters (using call span containment)
                // 2. Module-level annotated variables
                let arg_type = find_arg_type(arg_name, call.span, module, source);

                if let Some(arg_type_str) = arg_type {
                    if bound_incompatible(bound_type, arg_type_str) {
                        diagnostics.push(error_diagnostic_owned(
                            CODE.clone(),
                            format!(
                                "Argument `{arg_name}` has type `{arg_type_str}` which is not \
                                 compatible with TypeVar bound `{bound_type}`"
                            ),
                            *arg_span,
                            &module.path,
                            Some(format!(
                                "Pass a value of type `{bound_type}` or a subtype thereof"
                            )),
                            Some(format!(
                                "TypeVar `{ann_trimmed}` requires its argument to be a subtype of `{bound_type}`"
                            )),
                        ));
                    }
                }
            }
        }
    }
}

/// Try to find the type annotation of a name by searching the enclosing
/// function's parameters (determined by span containment) and module-level variables.
fn find_arg_type<'a>(
    name: &str,
    call_span: basilisk_resolver::Span,
    module: &'a ResolvedModule,
    source: &'a str,
) -> Option<&'a str> {
    // Find the enclosing function (the one whose def_span contains the call)
    let enclosing_func = module
        .functions
        .iter()
        .find(|func| func.def_span.start <= call_span.start && call_span.end <= func.def_span.end);

    // Check enclosing function parameters
    if let Some(func) = enclosing_func {
        for param in &func.parameters {
            if param.name == name {
                let ann_span = param.annotation_span?;
                let ann_text = slice_span(source, ann_span)?;
                return Some(ann_text.trim());
            }
        }
    }

    // Check module-level annotated variables
    for var in &module.module_vars {
        if var.name == name && var.has_annotation {
            let ann_span = var.annotation_span?;
            let ann_text = slice_span(source, ann_span)?;
            return Some(ann_text.trim());
        }
    }

    None
}
