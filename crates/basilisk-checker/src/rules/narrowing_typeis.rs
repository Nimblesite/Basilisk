//! Implements [narrowing_typeis] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! narrowing_typeis: TypeGuard/TypeIs return type incompatibility in callable arguments.
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

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::guards::is_protocol_class;
use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "narrowing_typeis",
    docs_url: "https://www.basilisk-python.dev/errors/narrowing_typeis",
};

/// Emits narrowing_typeis when a TypeGuard/TypeIs function is passed to a callable
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
    let return_type = inner.get(comma_pos + 1..)?.trim();
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
    let ann_text = slice_span(&module.source, ann_span)?;
    Some(ann_text.trim())
}

/// Extract the inner type argument from `TypeGuard[X]` or `TypeIs[X]`.
fn extract_guard_inner(ann: &str) -> Option<&str> {
    let inner = ann
        .strip_prefix("TypeGuard[")
        .or_else(|| ann.strip_prefix("TypeIs["))?;
    inner.strip_suffix(']')
}

/// `true` when `sub` is a subtype of `sup` for the implicit numeric tower
/// (`bool <: int <: float <: complex`), or they are identical.
fn is_subtype(sub: &str, sup: &str) -> bool {
    sub == sup
        || matches!(
            (sub, sup),
            ("bool", "int" | "float" | "complex")
                | ("int", "float" | "complex")
                | ("float", "complex")
        )
}

/// Check whether the expected return type is compatible with the actual
/// TypeGuard/TypeIs return type of the argument function.
///
/// - `bool` is always compatible (`TypeGuard` and `TypeIs` are subtypes of bool).
/// - `TypeGuard[X]` is **covariant**: `TypeGuard[B]` is assignable to
///   `TypeGuard[A]` when `B` is a subtype of `A` (and not to `TypeIs`).
/// - `TypeIs[X]` is only compatible with `TypeIs[X]` (not `TypeGuard`), and
///   `TypeIs` is **invariant** in its type argument.
fn is_compatible_return_type(expected_return: &str, actual_return: &str) -> bool {
    if expected_return == "bool" {
        return true;
    }

    let expected_is_typeguard = expected_return.starts_with("TypeGuard[");
    let expected_is_typeis = expected_return.starts_with("TypeIs[");
    let actual_is_typeguard = actual_return.starts_with("TypeGuard[");
    let actual_is_typeis = actual_return.starts_with("TypeIs[");

    // TypeGuard expected, TypeGuard actual — covariant: actual inner must be a
    // subtype of the expected inner.
    if expected_is_typeguard && actual_is_typeguard {
        return match (
            extract_guard_inner(expected_return),
            extract_guard_inner(actual_return),
        ) {
            (Some(expected_inner), Some(actual_inner)) => is_subtype(actual_inner, expected_inner),
            _ => false,
        };
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

/// Build a map of module-level function names to functions returning TypeGuard/TypeIs.
fn build_typeguard_func_map(module: &ResolvedModule) -> HashMap<&str, &str> {
    let source = &module.source;
    let mut typeguard_funcs: HashMap<&str, &str> = HashMap::new();
    for func in &module.functions {
        if func.class_name.is_some() {
            continue;
        }
        let Some(ann_span) = func.return_annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        if is_typeguard_or_typeis(ann_text) {
            let _ = typeguard_funcs.insert(func.name.as_str(), ann_text.trim());
        }
    }
    typeguard_funcs
}

/// Build context-appropriate diagnostic messages for a TypeGuard/TypeIs mismatch.
fn build_mismatch_messages(
    arg_name: &str,
    guard_return_text: &str,
    guard_kind: &str,
    expected_return: &str,
    callee: &str,
) -> (String, String, String) {
    let expected_is_guard =
        expected_return.starts_with("TypeGuard[") || expected_return.starts_with("TypeIs[");

    if expected_is_guard {
        let expected_kind = if expected_return.starts_with("TypeIs[") {
            "TypeIs"
        } else {
            "TypeGuard"
        };

        if guard_kind == expected_kind {
            (
                format!(
                    "Function `{arg_name}` returns `{guard_return_text}`, \
                     but `{callee}` expects `{expected_return}`"
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
            (
                format!(
                    "Function `{arg_name}` returns `{guard_return_text}`, \
                     but `{callee}` expects `{expected_return}`"
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
        (
            format!(
                "Function `{arg_name}` returns `{guard_return_text}` \
                 (subtype of `bool`), but `{callee}` expects return \
                 type `{expected_return}`"
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
    }
}

impl Rule for TypeGuardCallableReturnMismatch {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let source = &module.source;

        let mut func_map: HashMap<&str, &basilisk_resolver::FunctionInfo> = HashMap::new();
        for func in &module.functions {
            if func.class_name.is_none() {
                let _ = func_map.insert(func.name.as_str(), func);
            }
        }

        let typeguard_funcs = build_typeguard_func_map(module);
        if typeguard_funcs.is_empty() {
            return;
        }

        for call in &module.calls {
            let Some(callee_func) = func_map.get(call.callee.as_str()) else {
                continue;
            };

            for (arg_idx, (_rhs_kind, arg_span)) in call.args.iter().enumerate() {
                let Some(arg_text) = slice_span(source, *arg_span) else {
                    continue;
                };
                let arg_name = arg_text.trim();

                let Some(&guard_return_text) = typeguard_funcs.get(arg_name) else {
                    continue;
                };

                let Some(param) = callee_func.parameters.get(arg_idx) else {
                    continue;
                };
                let Some(param_ann_span) = param.annotation_span else {
                    continue;
                };
                let Some(param_ann_text) = slice_span(source, param_ann_span) else {
                    continue;
                };
                let param_ann = param_ann_text.trim();

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

                let (msg, help_text, note_text) = build_mismatch_messages(
                    arg_name,
                    guard_return_text,
                    guard_kind,
                    expected_return,
                    &call.callee,
                );

                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    msg,
                    *arg_span,
                    &module.path,
                    Some(help_text),
                    Some(note_text),
                ));
            }
        }
    }
}
