//! Helper functions for BSK-E0137: Generic protocol violations.

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ParameterInfo};

use crate::rules::shared::contains_typevar_reference;
use crate::span_util::slice_span;

/// Extract the constructor class name from an expression like `ClassName(...)`.
pub(super) fn extract_constructor_name(expr: &str) -> Option<&str> {
    let paren_pos = expr.find('(')?;
    let name = expr[..paren_pos].trim();
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some(name)
}

/// Substitute `TypeVar` names in a type annotation text.
///
/// For each `TypeVar` in the substitution map, replaces standalone occurrences
/// of the `TypeVar` name with the concrete type. This is a best-effort text
/// substitution that works for simple annotations.
pub(super) fn substitute_typevars(
    text: &str,
    substitution: &HashMap<&str, &str>,
    typevar_info: &HashMap<&str, bool>,
) -> String {
    let mut result = text.to_owned();

    for (tv_name, concrete_type) in substitution {
        // Only substitute if the name is a TypeVar.
        let is_typevar = typevar_info.contains_key(tv_name);
        if !is_typevar {
            continue;
        }

        let mut new_result = String::new();
        let mut remaining = result.as_str();

        while let Some(pos) = remaining.find(tv_name) {
            let before_ok = pos == 0
                || remaining
                    .as_bytes()
                    .get(pos - 1)
                    .is_none_or(|&b| !is_ident_char(b));
            let after_pos = pos + tv_name.len();
            let after_ok = after_pos >= remaining.len()
                || remaining
                    .as_bytes()
                    .get(after_pos)
                    .is_none_or(|&b| !is_ident_char(b));

            if before_ok && after_ok {
                new_result.push_str(&remaining[..pos]);
                new_result.push_str(concrete_type);
                remaining = &remaining[after_pos..];
            } else {
                new_result.push_str(&remaining[..=pos]);
                remaining = &remaining[pos + 1..];
            }
        }
        new_result.push_str(remaining);
        result = new_result;
    }

    result
}

/// Returns `true` when `actual` and `expected` types are compatible.
///
/// A conservative check: returns `true` when we cannot determine incompatibility.
pub(super) fn types_compatible(expected: &str, actual: &str) -> bool {
    let expected = expected.trim();
    let actual = actual.trim();

    if expected == actual {
        return true;
    }

    // Unknown types: skip.
    if expected.is_empty() || actual.is_empty() {
        return true;
    }

    // Any is always compatible.
    if expected == "Any" || actual == "Any" {
        return true;
    }

    // Self is compatible with Self.
    if expected == "Self" && actual == "Self" {
        return true;
    }

    // int is compatible with float.
    if expected == "float" && actual == "int" {
        return true;
    }

    // bool is compatible with int.
    if expected == "int" && actual == "bool" {
        return true;
    }

    // If the expected type still contains a TypeVar name (was not substituted),
    // we cannot determine compatibility.
    if expected.chars().next().is_some_and(char::is_uppercase) && !expected.contains('[') {
        // Could be an unresolved TypeVar — be conservative.
        return true;
    }

    false
}

/// Returns `true` for ASCII alphanumeric or underscore characters.
pub(super) const fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Skip the `self` or `cls` parameter from a parameter list.
pub(super) fn skip_self_param(params: &[ParameterInfo]) -> &[ParameterInfo] {
    let Some(first) = params.first() else {
        return params;
    };
    if first.name == "self" || first.name == "cls" {
        params.get(1..).unwrap_or(&[])
    } else {
        params
    }
}

/// Returns `true` when the method has a typed `self` parameter (e.g. `self: T`).
pub(super) fn method_has_typed_self(method: &FunctionInfo) -> bool {
    if let Some(first_param) = method.parameters.first() {
        if first_param.name == "self" && first_param.has_annotation {
            return true;
        }
    }
    false
}

/// Get the `TypeVar` name used in a `self: T` annotation, if present.
pub(super) fn get_self_typevar_name(method: &FunctionInfo, source: &str) -> Option<String> {
    let first_param = method.parameters.first()?;
    if first_param.name != "self" || !first_param.has_annotation {
        return None;
    }
    let ann_span = first_param.annotation_span?;
    let ann_text = slice_span(source, ann_span)?.trim();
    // The TypeVar name is a simple identifier.
    if ann_text.chars().all(|c| c.is_alphanumeric() || c == '_') {
        Some(ann_text.to_owned())
    } else {
        None
    }
}

/// Compare protocol methods against concrete methods, returning the first mismatch detail.
pub(super) fn find_method_mismatch(
    proto_methods: &[&FunctionInfo],
    rhs_methods: &[&FunctionInfo],
    source: &str,
    substitution: &HashMap<&str, &str>,
    typevar_info: &HashMap<&str, bool>,
) -> Option<String> {
    for proto_method in proto_methods {
        if proto_method.name == "__init__" {
            continue;
        }

        let Some(concrete_method) = rhs_methods.iter().find(|m| m.name == proto_method.name) else {
            continue;
        };

        if let Some(detail) = check_return_type_mismatch(
            proto_method,
            concrete_method,
            source,
            substitution,
            typevar_info,
        ) {
            return Some(detail);
        }

        if let Some(detail) = check_param_type_mismatch(
            proto_method,
            concrete_method,
            source,
            substitution,
            typevar_info,
        ) {
            return Some(detail);
        }
    }
    None
}

/// Check return type compatibility between a protocol method and a concrete method.
pub(super) fn check_return_type_mismatch(
    proto_method: &FunctionInfo,
    concrete_method: &FunctionInfo,
    source: &str,
    substitution: &HashMap<&str, &str>,
    typevar_info: &HashMap<&str, bool>,
) -> Option<String> {
    let (Some(proto_ret_span), Some(concrete_ret_span)) = (
        proto_method.return_annotation_span,
        concrete_method.return_annotation_span,
    ) else {
        return None;
    };

    let proto_ret = slice_span(source, proto_ret_span).map_or("", str::trim);
    let concrete_ret = slice_span(source, concrete_ret_span).map_or("", str::trim);

    let expected_ret = substitute_typevars(proto_ret, substitution, typevar_info);

    if !types_compatible(&expected_ret, concrete_ret) {
        return Some(format!(
            "method `{}` return type: expected `{expected_ret}`, found `{concrete_ret}`",
            proto_method.name
        ));
    }
    None
}

/// Check parameter type compatibility between a protocol method and a concrete method.
pub(super) fn check_param_type_mismatch(
    proto_method: &FunctionInfo,
    concrete_method: &FunctionInfo,
    source: &str,
    substitution: &HashMap<&str, &str>,
    typevar_info: &HashMap<&str, bool>,
) -> Option<String> {
    let proto_params = skip_self_param(&proto_method.parameters);
    let concrete_params = skip_self_param(&concrete_method.parameters);

    if proto_params.len() != concrete_params.len() {
        return None;
    }

    for (proto_param, concrete_param) in proto_params.iter().zip(concrete_params.iter()) {
        if let (Some(proto_ann_span), Some(concrete_ann_span)) =
            (proto_param.annotation_span, concrete_param.annotation_span)
        {
            let proto_ann = slice_span(source, proto_ann_span).map_or("", str::trim);
            let concrete_ann = slice_span(source, concrete_ann_span).map_or("", str::trim);

            let expected_ann = substitute_typevars(proto_ann, substitution, typevar_info);

            if !types_compatible(&expected_ann, concrete_ann) {
                return Some(format!(
                    "method `{}` parameter `{}`: expected `{expected_ann}`, found `{concrete_ann}`",
                    proto_method.name, proto_param.name
                ));
            }
        }
    }
    None
}

/// Check if a concrete method is incompatible with a self-typed protocol method.
///
/// Returns a human-readable description of the mismatch, or `None` if the
/// concrete method is compatible.
///
/// The protocol method uses `self: T` where T is a `TypeVar`, so in the concrete
/// class, every use of T in parameters and return type must use the same concrete
/// type consistently.
pub(super) fn check_self_typed_method_incompatibility(
    proto_method: &FunctionInfo,
    concrete_method: &FunctionInfo,
    tv_name: &str,
    source: &str,
) -> Option<String> {
    // Get the concrete method's self annotation, if any.
    let concrete_self_ann = concrete_method
        .parameters
        .first()
        .filter(|p| p.name == "self")
        .and_then(|p| p.annotation_span)
        .and_then(|span| slice_span(source, span))
        .map(str::trim);

    // Get the proto method's return type.
    let proto_ret = proto_method
        .return_annotation_span
        .and_then(|span| slice_span(source, span))
        .map(str::trim);

    // Get the concrete method's return type.
    let concrete_ret = concrete_method
        .return_annotation_span
        .and_then(|span| slice_span(source, span))
        .map(str::trim);

    // If the protocol return type is the TypeVar itself (T returns T),
    // the concrete method must return Self or its own class type.
    if let Some(proto_ret_text) = proto_ret {
        if proto_ret_text.trim() == tv_name {
            let concrete_ret_text = concrete_ret.unwrap_or("").trim();
            let concrete_self_text = concrete_self_ann.unwrap_or("self").trim();

            if concrete_self_text == "self" {
                let is_self_type = concrete_ret_text == "Self"
                    || concrete_ret_text == tv_name
                    || concrete_ret_text
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_');

                if !is_self_type
                    || concrete_ret_text == "int"
                    || concrete_ret_text == "str"
                    || concrete_ret_text == "float"
                    || concrete_ret_text == "bool"
                    || concrete_ret_text == "bytes"
                    || concrete_ret_text == "None"
                {
                    return Some(format!(
                        "method `{}` must return a self-typed value (TypeVar `{tv_name}` or \
                         `Self`), but returns `{concrete_ret_text}`",
                        proto_method.name
                    ));
                }
            }
        }
    }

    check_self_typed_param_consistency(proto_method, concrete_method, tv_name, source, concrete_ret)
}

/// Check non-self parameters for `TypeVar` consistency in self-typed methods.
fn check_self_typed_param_consistency(
    proto_method: &FunctionInfo,
    concrete_method: &FunctionInfo,
    tv_name: &str,
    source: &str,
    concrete_ret: Option<&str>,
) -> Option<String> {
    let proto_params = skip_self_param(&proto_method.parameters);
    let concrete_params = skip_self_param(&concrete_method.parameters);

    if proto_params.len() != concrete_params.len() {
        return None; // Arity mismatch handled by other rules.
    }

    for (proto_param, concrete_param) in proto_params.iter().zip(concrete_params.iter()) {
        let proto_ann = proto_param
            .annotation_span
            .and_then(|span| slice_span(source, span))
            .map_or("", str::trim);

        if proto_ann != tv_name {
            continue;
        }

        let concrete_ann = concrete_param
            .annotation_span
            .and_then(|span| slice_span(source, span))
            .map_or("", str::trim);

        let concrete_ret_text = concrete_ret.unwrap_or("").trim();

        if !concrete_ann.is_empty()
            && concrete_ret_text != "Self"
            && concrete_ret_text != tv_name
            && concrete_ret_text != concrete_ann
            && !concrete_ann.is_empty()
        {
            return Some(format!(
                "method `{}` uses `{concrete_ann}` for the `{tv_name}` parameter but \
                 returns `{concrete_ret_text}`; these must match for a self-typed protocol",
                proto_method.name
            ));
        }
    }

    None
}

/// Check non-self-typed methods that reference the self-TypeVar `T` from a
/// `self: T` method.
///
/// When a protocol has `def f(self: T) -> T` and `def m(self, item: T, ...) -> str`,
/// the concrete class must use `T` generically in `m` or use consistent concrete
/// types that match the self-type binding.
pub(super) fn check_typevar_methods_consistency(
    proto_methods: &[&FunctionInfo],
    concrete_methods: &[&FunctionInfo],
    tv_name: &str,
    source: &str,
) -> Option<String> {
    for proto_method in proto_methods {
        // Skip self-typed methods (handled separately) and __init__.
        if method_has_typed_self(proto_method) || proto_method.name == "__init__" {
            continue;
        }

        // Check if this method references the TypeVar in any parameter annotation.
        let proto_params = skip_self_param(&proto_method.parameters);
        let uses_tv = proto_params.iter().any(|p| {
            p.annotation_span
                .and_then(|span| slice_span(source, span))
                .is_some_and(|text| contains_typevar_reference(text.trim(), tv_name))
        });

        if !uses_tv {
            continue;
        }

        // Find the concrete method.
        let Some(concrete_method) = concrete_methods
            .iter()
            .find(|m| m.name == proto_method.name)
        else {
            continue;
        };

        let concrete_params = skip_self_param(&concrete_method.parameters);
        if proto_params.len() != concrete_params.len() {
            continue;
        }

        // Collect the concrete types used where the protocol uses `T`.
        // If the concrete method also uses `T` generically, it's OK.
        let mut bound_types: Vec<&str> = Vec::new();
        let mut concrete_uses_tv = false;

        for (pp, cp) in proto_params.iter().zip(concrete_params.iter()) {
            let proto_ann = pp
                .annotation_span
                .and_then(|span| slice_span(source, span))
                .map_or("", str::trim);

            if !contains_typevar_reference(proto_ann, tv_name) {
                continue;
            }

            let concrete_ann = cp
                .annotation_span
                .and_then(|span| slice_span(source, span))
                .map_or("", str::trim);

            if concrete_ann.is_empty() {
                continue;
            }

            // If concrete also uses the TypeVar, method is generic — OK.
            if contains_typevar_reference(concrete_ann, tv_name) {
                concrete_uses_tv = true;
                continue;
            }

            // Concrete uses a specific type where protocol uses T.
            // Extract the "core" type used for T.
            // For direct `T` usage: `item: T` → concrete_ann is the bound type.
            // For nested usage: `Callable[[T], str]` → need to check consistency
            // of what T maps to.
            if proto_ann == tv_name {
                bound_types.push(concrete_ann);
            }
        }

        // If concrete uses T generically, it matches the protocol.
        if concrete_uses_tv {
            continue;
        }

        // Check consistency and concreteness of types bound to T.
        if let Some((&first, rest)) = bound_types.split_first() {
            // All concrete types bound to T must be the same.
            for &other in rest {
                if other != first {
                    return Some(format!(
                        "method `{}` uses inconsistent types for TypeVar `{tv_name}`: \
                         `{first}` and `{other}`",
                        proto_method.name
                    ));
                }
            }

            // A concrete type (not Self/T) cannot satisfy a protocol
            // where T is bound to Self via `self: T`.
            if first != "Self" && first != tv_name {
                return Some(format!(
                    "method `{}` uses concrete type `{first}` for TypeVar `{tv_name}` \
                     which is bound to `Self` via protocol's `self: {tv_name}` annotation",
                    proto_method.name
                ));
            }
        }
    }
    None
}
