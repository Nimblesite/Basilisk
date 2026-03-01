//! BSK-E0080: TypeVar upper bound violation at call site.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0080",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0080",
};

/// Emits BSK-E0080 when a call site passes a value whose type does not satisfy
/// the `TypeVar` upper bound declared on the corresponding parameter.
pub(crate) struct TypeVarBoundViolation;

impl Rule for TypeVarBoundViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;

        // Step 1: Build a map of TypeVar name -> bound text.
        let mut typevar_bounds: HashMap<&str, String> = HashMap::new();
        for tv in &module.typevar_calls {
            if !tv.has_bound {
                continue;
            }
            if let Some(bound_text) = extract_bound_text(source, tv.span) {
                typevar_bounds.insert(tv.name.as_str(), bound_text);
            }
        }

        if typevar_bounds.is_empty() {
            return;
        }

        // Step 2: Build a map of function name -> list of (param_index, bound_text).
        let mut func_param_bounds: HashMap<&str, Vec<(usize, String)>> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_some() {
                continue;
            }
            let mut param_bounds = Vec::new();
            for (idx, param) in func.parameters.iter().enumerate() {
                let Some(ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(ann_text) =
                    source.get(ann_span.start as usize..ann_span.end as usize)
                else {
                    continue;
                };
                let ann_text = ann_text.trim();
                if let Some(bound) = typevar_bounds.get(ann_text) {
                    param_bounds.push((idx, bound.clone()));
                }
            }
            if !param_bounds.is_empty() {
                func_param_bounds.insert(func.name.as_str(), param_bounds);
            }
        }

        if func_param_bounds.is_empty() {
            return;
        }

        // Step 3: Check call sites.
        for call in &module.calls {
            let Some(param_bounds) = func_param_bounds.get(call.callee.as_str()) else {
                continue;
            };

            for &(param_idx, ref bound_text) in param_bounds {
                let Some((rhs_kind, _arg_span)) = call.args.get(param_idx) else {
                    continue;
                };

                let Some(lit_type) = literal_type_name(rhs_kind) else {
                    continue;
                };

                if !type_satisfies_bound(lit_type, bound_text) {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Argument of type `{lit_type}` does not satisfy \
                             `TypeVar` bound `{bound_text}` for `{}`",
                            call.callee
                        ),
                        span: call.span,
                        path: module.path.clone(),
                        help: Some(format!(
                            "Pass a value that satisfies `{bound_text}` \
                             (e.g. a type implementing `__len__`)"
                        )),
                        note: Some(format!(
                            "TypeVar bound `{bound_text}` requires the argument type to \
                             be a subtype of `{bound_text}`"
                        )),
                    });
                    // Only one diagnostic per call.
                    break;
                }
            }
        }
    }
}

/// Extract the bound text from a `TypeVar("Name", bound=X)` call in source.
fn extract_bound_text(source: &str, span: basilisk_resolver::Span) -> Option<String> {
    let call_text = source.get(span.start as usize..span.end as usize)?;
    let bound_idx = call_text.find("bound=")?;
    let after_bound = &call_text[bound_idx + "bound=".len()..];

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
    let bound_text = after_bound[..end].trim();
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
/// Conservative check for well-known bounds only.
fn type_satisfies_bound(concrete_type: &str, bound: &str) -> bool {
    match bound {
        // `Sized` requires `__len__` -- only collection types satisfy it.
        "Sized" => matches!(
            concrete_type,
            "str" | "bytes" | "list" | "tuple" | "dict" | "set" | "frozenset"
        ),
        // For other bounds, be conservative and assume satisfied.
        _ => true,
    }
}
