//! BSK-E0112: TypeGuard/TypeIs return type incompatibility in callable arguments.
//!
//! When a function returning `TypeGuard[X]` or `TypeIs[X]` is passed as an
//! argument where the expected callable return type is NOT `bool`, this rule
//! reports the mismatch. `TypeGuard` and `TypeIs` are subtypes of `bool` in
//! callable context, so passing them where `Callable[..., bool]` is expected
//! is valid, but passing them where e.g. `Callable[..., str]` is expected is
//! an error.
//!
//! ```python
//! def takes_callable_str(f: Callable[[object], str]) -> None: ...
//! def simple_typeguard(val: object) -> TypeGuard[int]: ...
//!
//! takes_callable_str(simple_typeguard)  # E0112 — TypeGuard is bool, not str
//! ```

use std::collections::HashMap;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::guards::is_protocol_class;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0112",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0112",
};

/// Emits BSK-E0112 when a TypeGuard/TypeIs function is passed to a callable
/// parameter whose return type is not `bool`.
pub(crate) struct TypeGuardCallableReturnMismatch;

/// Returns `true` if the annotation text indicates a `TypeGuard` or `TypeIs`
/// return type.
fn is_typeguard_or_typeis(ann_text: &str) -> bool {
    let trimmed = ann_text.trim();
    trimmed.starts_with("TypeGuard[") || trimmed.starts_with("TypeIs[")
}

/// Extract the return type from a `Callable[[...], ReturnType]` annotation.
///
/// Finds the last `,` at bracket depth 0 (after the initial `Callable[`),
/// then takes everything after it up to the closing `]`.
fn extract_callable_return_type(ann_text: &str) -> Option<&str> {
    let inner = ann_text.trim().strip_prefix("Callable[")?;
    // Remove the trailing `]`
    let inner = inner.strip_suffix(']')?;

    // Find the last comma at depth 0
    let mut depth: i32 = 0;
    let mut last_comma_at_depth_0: Option<usize> = None;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => last_comma_at_depth_0 = Some(idx),
            _ => {}
        }
    }

    let comma_pos = last_comma_at_depth_0?;
    let return_type = inner[comma_pos + 1..].trim();
    if return_type.is_empty() {
        return None;
    }
    Some(return_type)
}

/// For a Protocol class, find the return type of its `__call__` method.
fn find_protocol_call_return_type<'a>(
    class_name: &str,
    module: &'a ResolvedModule,
) -> Option<&'a str> {
    // Verify the class is a Protocol
    let cls = module
        .classes
        .iter()
        .find(|c| c.name == class_name && is_protocol_class(c))?;

    // Find __call__ method
    let call_method = module
        .functions
        .iter()
        .find(|f| f.class_name.as_deref() == Some(cls.name.as_str()) && f.name == "__call__")?;

    let ann_span = call_method.return_annotation_span?;
    let ann_text = module
        .source
        .get(ann_span.start as usize..ann_span.end as usize)?;
    Some(ann_text.trim())
}

/// Extract the inner type argument from `TypeGuard[X]` or `TypeIs[X]`.
fn extract_guard_inner(ann: &str) -> Option<&str> {
    let inner = ann
        .strip_prefix("TypeGuard[")
        .or_else(|| ann.strip_prefix("TypeIs["))?;
    inner.strip_suffix(']')
}

/// Check whether the expected return type is compatible with the actual
/// TypeGuard/TypeIs return type of the argument function.
///
/// - `bool` is always compatible (TypeGuard and TypeIs are subtypes of bool).
/// - `TypeGuard[X]` is only compatible with `TypeGuard[Y]` (not TypeIs), and
///   TypeGuard is covariant (simplified to exact match here).
/// - `TypeIs[X]` is only compatible with `TypeIs[X]` (not TypeGuard), and
///   TypeIs is **invariant** in its type argument.
fn is_compatible_return_type(expected_return: &str, actual_return: &str) -> bool {
    if expected_return == "bool" {
        return true;
    }

    let expected_is_typeguard = expected_return.starts_with("TypeGuard[");
    let expected_is_typeis = expected_return.starts_with("TypeIs[");
    let actual_is_typeguard = actual_return.starts_with("TypeGuard[");
    let actual_is_typeis = actual_return.starts_with("TypeIs[");

    // TypeGuard expected, TypeGuard actual — check inner types match
    if expected_is_typeguard && actual_is_typeguard {
        let expected_inner = extract_guard_inner(expected_return);
        let actual_inner = extract_guard_inner(actual_return);
        return expected_inner == actual_inner;
    }

    // TypeIs expected, TypeIs actual — invariant, inner types must match exactly
    if expected_is_typeis && actual_is_typeis {
        let expected_inner = extract_guard_inner(expected_return);
        let actual_inner = extract_guard_inner(actual_return);
        return expected_inner == actual_inner;
    }

    // TypeGuard and TypeIs are NOT interchangeable
    if (expected_is_typeguard && actual_is_typeis) || (expected_is_typeis && actual_is_typeguard) {
        return false;
    }

    // Any other expected type (e.g. str, int) is incompatible
    false
}

impl Rule for TypeGuardCallableReturnMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;

        // Build map: function name -> FunctionInfo for module-level functions
        let mut func_map: HashMap<&str, &basilisk_resolver::FunctionInfo> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_none() {
                func_map.insert(func.name.as_str(), func);
            }
        }

        // Find all module-level functions that return TypeGuard or TypeIs
        let mut typeguard_funcs: HashMap<&str, &str> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_some() {
                continue;
            }
            let Some(ann_span) = func.return_annotation_span else {
                continue;
            };
            let Some(ann_text) = source.get(ann_span.start as usize..ann_span.end as usize) else {
                continue;
            };
            if is_typeguard_or_typeis(ann_text) {
                typeguard_funcs.insert(func.name.as_str(), ann_text.trim());
            }
        }

        if typeguard_funcs.is_empty() {
            return;
        }

        // Check each module-level call
        for call in &module.calls {
            let Some(callee_func) = func_map.get(call.callee.as_str()) else {
                continue;
            };

            for (arg_idx, (_rhs_kind, arg_span)) in call.args.iter().enumerate() {
                // Get the argument text (the name of the function being passed)
                let Some(arg_text) = source.get(arg_span.start as usize..arg_span.end as usize)
                else {
                    continue;
                };
                let arg_name = arg_text.trim();

                // Check if this argument is a TypeGuard/TypeIs function
                let Some(&guard_return_text) = typeguard_funcs.get(arg_name) else {
                    continue;
                };

                // Get the parameter annotation at this position
                let Some(param) = callee_func.parameters.get(arg_idx) else {
                    continue;
                };
                let Some(param_ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(param_ann_text) =
                    source.get(param_ann_span.start as usize..param_ann_span.end as usize)
                else {
                    continue;
                };
                let param_ann = param_ann_text.trim();

                // Check two cases:
                // 1. Callable[[...], ReturnType] annotation
                // 2. Protocol class with __call__ method
                let expected_return = if param_ann.starts_with("Callable[") {
                    extract_callable_return_type(param_ann)
                } else {
                    find_protocol_call_return_type(param_ann, module)
                };

                let Some(expected_return) = expected_return else {
                    continue;
                };

                if is_compatible_return_type(expected_return, guard_return_text) {
                    continue;
                }

                let guard_kind = if guard_return_text.starts_with("TypeIs[") {
                    "TypeIs"
                } else {
                    "TypeGuard"
                };

                // Build context-appropriate diagnostic messages
                let expected_is_guard = expected_return.starts_with("TypeGuard[")
                    || expected_return.starts_with("TypeIs[");

                let (msg, help_text, note_text) = if expected_is_guard {
                    let expected_kind = if expected_return.starts_with("TypeIs[") {
                        "TypeIs"
                    } else {
                        "TypeGuard"
                    };

                    if guard_kind == expected_kind {
                        // Same kind but different inner type (invariance)
                        (
                            format!(
                                "Function `{arg_name}` returns `{guard_return_text}`, \
                                 but `{callee}` expects `{expected_return}`",
                                callee = call.callee,
                            ),
                            format!(
                                "`{guard_kind}` is invariant in its type argument; \
                                 `{guard_return_text}` is not assignable to \
                                 `{expected_return}`"
                            ),
                            format!(
                                "`{guard_kind}[B]` is not a subtype of \
                                 `{guard_kind}[A]` even if `B` is a subtype of `A`"
                            ),
                        )
                    } else {
                        // TypeGuard vs TypeIs mismatch
                        (
                            format!(
                                "Function `{arg_name}` returns `{guard_return_text}`, \
                                 but `{callee}` expects `{expected_return}`",
                                callee = call.callee,
                            ),
                            format!(
                                "`{guard_kind}` and `{expected_kind}` are not \
                                 interchangeable; use a function returning \
                                 `{expected_return}` instead"
                            ),
                            "`TypeGuard` and `TypeIs` have different narrowing \
                             semantics and are not assignable to each other"
                                .to_owned(),
                        )
                    }
                } else {
                    // Plain type mismatch (e.g. expected str)
                    (
                        format!(
                            "Function `{arg_name}` returns `{guard_return_text}` \
                             (subtype of `bool`), but `{callee}` expects return \
                             type `{expected_return}`",
                            callee = call.callee,
                        ),
                        format!(
                            "`{guard_kind}` is a subtype of `bool`, not \
                             `{expected_return}`; change the expected return type \
                             to `bool` or use a compatible callable"
                        ),
                        format!(
                            "`{guard_kind}` in callable context is treated as a \
                             subtype of `bool` per the typing specification"
                        ),
                    )
                };

                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: msg,
                    span: *arg_span,
                    path: module.path.clone(),
                    help: Some(help_text),
                    note: Some(note_text),
                });
            }
        }
    }
}
