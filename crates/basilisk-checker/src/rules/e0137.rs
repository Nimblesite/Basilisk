//! BSK-E0137: Generic protocol violations.
//!
//! Detects violations related to generic protocol usage:
//!
//! 1. **Protocol[T] combined with Generic[T]**: The `Protocol[T, S, ...]` shorthand
//!    is already equivalent to `Protocol, Generic[T, S, ...]`. It is an error to
//!    combine the shorthand with an explicit `Generic[...]` base.
//!
//! 2. **Incompatible generic protocol assignment**: When a module-level variable
//!    is annotated with a concrete generic protocol specialisation like
//!    `Proto[int, str]` and the RHS is a concrete class, the concrete class's
//!    method signatures must be compatible with the substituted type arguments.
//!
//! 3. **Self-typed protocol method incompatibility**: When a protocol declares
//!    methods using a `self: T` annotation (making the return type depend on the
//!    concrete receiver), concrete classes that implement those methods with
//!    incompatible signatures are flagged.
//!
//! ```python
//! from typing import Generic, Protocol, TypeVar
//!
//! T_co = TypeVar("T_co", covariant=True)
//!
//! class Proto2(Protocol[T_co], Generic[T_co]):  # E — shorthand + Generic
//!     ...
//! ```
//!
//! PEP 544: <https://typing.readthedocs.io/en/latest/spec/protocol.html#generic-protocols>

use std::collections::HashMap;

use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0137",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0137",
};

/// Emits BSK-E0137 for generic protocol violations.
pub(crate) struct GenericProtocolViolation;

impl Rule for GenericProtocolViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Build a lookup map from class name to ClassInfo.
        let class_map: HashMap<&str, &ClassInfo> = module
            .classes
            .iter()
            .map(|cls| (cls.name.as_str(), cls))
            .collect();

        // Build a map from class name -> list of methods.
        let class_methods: HashMap<&str, Vec<&FunctionInfo>> = module
            .classes
            .iter()
            .map(|cls| {
                let methods: Vec<&FunctionInfo> = module
                    .functions
                    .iter()
                    .filter(|f| f.class_name.as_deref() == Some(cls.name.as_str()))
                    .collect();
                (cls.name.as_str(), methods)
            })
            .collect();

        // Build TypeVar variance info.
        let typevar_info: HashMap<&str, bool> = module
            .typevar_calls
            .iter()
            .map(|tv| (tv.name.as_str(), tv.is_covariant || tv.is_contravariant))
            .collect();

        // --- Check 1: Protocol[T] combined with Generic[T] ---
        check_protocol_generic_combined(module, path, diagnostics);

        // --- Check 2: Generic protocol assignment type arg mismatch ---
        check_generic_protocol_assignments(
            module,
            source,
            path,
            &class_map,
            &class_methods,
            &typevar_info,
            diagnostics,
        );

        // --- Check 3: Self-typed protocol method incompatibility ---
        check_self_typed_protocol_assignments(
            module,
            source,
            path,
            &class_map,
            &class_methods,
            diagnostics,
        );
    }
}

// ---------------------------------------------------------------------------
// Check 1: Protocol[T] shorthand combined with Generic[T]
// ---------------------------------------------------------------------------

/// Detect classes that inherit from both `Protocol[T]` (subscript) and `Generic[T]`.
///
/// Per PEP 544, `Protocol[T, S, ...]` is a shorthand for `Protocol, Generic[T, S, ...]`.
/// Combining both is a redundant and invalid form.
fn check_protocol_generic_combined(
    module: &ResolvedModule,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for class in &module.classes {
        // Only check classes that have Protocol as a base (either plain or subscripted).
        let has_protocol_in_bases = class.bases.iter().any(|b| b == "Protocol")
            || class
                .base_subscripts
                .iter()
                .any(|bs| bs.base_name == "Protocol");
        if !has_protocol_in_bases {
            continue;
        }

        // Check if any base subscript uses Protocol[T] (subscript form).
        let protocol_is_subscripted = class
            .base_subscripts
            .iter()
            .any(|bs| bs.base_name == "Protocol");

        if !protocol_is_subscripted {
            continue;
        }

        // Check if Generic also appears as a base (subscript or plain).
        let has_generic_base = class.bases.iter().any(|b| b == "Generic")
            || class
                .base_subscripts
                .iter()
                .any(|bs| bs.base_name == "Generic");

        if has_generic_base {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Protocol class `{}` combines `Protocol[T]` shorthand with explicit `Generic[T]` base",
                    class.name
                ),
                span: class.name_span,
                path: path.to_owned(),
                help: Some(
                    "Remove the explicit `Generic[T]` base; `Protocol[T]` already implies \
                     `Generic[T]`"
                        .to_owned(),
                ),
                note: Some(
                    "PEP 544: `Protocol[T, S, ...]` is shorthand for `Protocol, Generic[T, S, ...]`. \
                     Combining the two is redundant and invalid."
                        .to_owned(),
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Check 2: Generic protocol assignment with wrong type arguments
// ---------------------------------------------------------------------------

/// Check module-level annotated assignments where:
/// - The annotation is a subscripted generic protocol (`Proto[A, B]`)
/// - The RHS is a concrete class constructor call
/// - The concrete class's methods are incompatible with the substituted type args.
fn check_generic_protocol_assignments(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &ClassInfo>,
    class_methods: &HashMap<&str, Vec<&FunctionInfo>>,
    typevar_info: &HashMap<&str, bool>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        if !var.has_annotation {
            continue;
        }

        let Some(ann_span) = var.annotation_span else {
            continue;
        };
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };

        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_text = ann_text.trim();

        // Only process subscripted annotations like `Proto[T1, T2]`.
        let Some((proto_name, type_args)) = parse_subscript_annotation(ann_text) else {
            continue;
        };

        // Look up the protocol class.
        let Some(proto_class) = class_map.get(proto_name) else {
            continue;
        };

        // Only process Protocol classes.
        let is_protocol = proto_class.bases.iter().any(|b| b == "Protocol")
            || proto_class
                .base_subscripts
                .iter()
                .any(|bs| bs.base_name == "Protocol");
        if !is_protocol {
            continue;
        }

        // Get the protocol's type parameters in order.
        let proto_type_params: Vec<&str> = proto_class
            .generic_params
            .iter()
            .map(|p| p.name.as_str())
            .collect();

        if proto_type_params.is_empty() || type_args.len() != proto_type_params.len() {
            continue;
        }

        // Build the substitution map: TypeVar name -> concrete type.
        let substitution: HashMap<&str, &str> = proto_type_params
            .iter()
            .zip(type_args.iter())
            .map(|(&tv, arg)| (tv, arg.as_str()))
            .collect();

        // Extract RHS class name.
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let rhs_text = rhs_text.trim();
        let Some(rhs_class_name) = extract_constructor_name(rhs_text) else {
            continue;
        };

        // If same class, skip.
        if rhs_class_name == proto_name {
            continue;
        }

        // Check that the concrete class's methods are compatible with the substituted types.
        let Some(rhs_methods) = class_methods.get(rhs_class_name) else {
            continue;
        };

        // For each method in the protocol, check the concrete class's signature.
        let Some(proto_methods) = class_methods.get(proto_name) else {
            continue;
        };

        if let Some(mismatch_details) = find_method_mismatch(
            proto_methods,
            rhs_methods,
            source,
            &substitution,
            typevar_info,
        ) {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Class `{rhs_class_name}` is incompatible with `{ann_text}`: {mismatch_details}"
                ),
                span: var.name_span,
                path: path.to_owned(),
                help: Some(format!(
                    "The concrete class `{rhs_class_name}` does not satisfy the \
                     type constraints of `{ann_text}`"
                )),
                note: Some(
                    "Generic protocols require that the implementing class's method signatures \
                     are compatible with the substituted type arguments"
                        .to_owned(),
                ),
            });
        }
    }
}

/// Compare protocol methods against concrete methods, returning the first mismatch detail.
fn find_method_mismatch(
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
fn check_return_type_mismatch(
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
fn check_param_type_mismatch(
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

// ---------------------------------------------------------------------------
// Check 3: Self-typed protocol method incompatibility
// ---------------------------------------------------------------------------

/// Check module-level annotated assignments for protocols that use `self: T`
/// typed parameters, where the concrete class doesn't correctly implement them.
///
/// When a protocol declares `def f(self: T) -> T` and a concrete class is
/// assigned to a variable of that protocol type, the concrete class must
/// implement `f` with a compatible self-typed signature.
fn check_self_typed_protocol_assignments(
    module: &ResolvedModule,
    source: &str,
    path: &str,
    class_map: &HashMap<&str, &ClassInfo>,
    class_methods: &HashMap<&str, Vec<&FunctionInfo>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        if !var.has_annotation {
            continue;
        }

        let Some(ann_span) = var.annotation_span else {
            continue;
        };
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };

        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_name = ann_text.trim();

        // Skip subscripted annotations (handled by Check 2).
        if ann_name.contains('[') {
            continue;
        }

        // Look up the protocol class.
        let Some(proto_class) = class_map.get(ann_name) else {
            continue;
        };

        // Only process Protocol classes.
        let is_protocol = proto_class.bases.iter().any(|b| b == "Protocol")
            || proto_class
                .base_subscripts
                .iter()
                .any(|bs| bs.base_name == "Protocol");
        if !is_protocol {
            continue;
        }

        // Extract RHS class name.
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let rhs_text = rhs_text.trim();
        let Some(rhs_class_name) = extract_constructor_name(rhs_text) else {
            continue;
        };

        if rhs_class_name == ann_name {
            continue;
        }

        // Check if the protocol has self-typed methods (methods with `self: T`).
        let Some(proto_methods) = class_methods.get(ann_name) else {
            continue;
        };

        let self_typed_methods: Vec<&FunctionInfo> = proto_methods
            .iter()
            .filter(|m| method_has_typed_self(m))
            .copied()
            .collect();

        if self_typed_methods.is_empty() {
            continue;
        }

        // Get the concrete class's methods.
        let Some(concrete_methods) = class_methods.get(rhs_class_name) else {
            continue;
        };

        // Check each self-typed protocol method.
        for proto_method in &self_typed_methods {
            let Some(concrete_method) = concrete_methods
                .iter()
                .find(|m| m.name == proto_method.name)
            else {
                continue;
            };

            // Get the TypeVar name used as the self annotation in the protocol.
            let proto_self_typevar = get_self_typevar_name(proto_method, source);

            // Check if concrete method has a compatible self-typed or Self-typed signature.
            if let Some(tv_name) = &proto_self_typevar {
                let mismatch = check_self_typed_method_incompatibility(
                    proto_method,
                    concrete_method,
                    tv_name,
                    source,
                );

                if let Some(detail) = mismatch {
                    diagnostics.push(Diagnostic {
                        code: CODE.clone(),
                        severity: Severity::Error,
                        message: format!(
                            "Class `{rhs_class_name}` is incompatible with protocol `{ann_name}`: \
                             {detail}"
                        ),
                        span: var.name_span,
                        path: path.to_owned(),
                        help: Some(format!(
                            "Method `{}` in `{rhs_class_name}` must match the self-typed \
                             signature declared in protocol `{ann_name}`",
                            proto_method.name
                        )),
                        note: Some(
                            "PEP 544: methods with `self: T` annotations in protocols require \
                             that implementing classes use compatible self-typed or `Self` signatures"
                                .to_owned(),
                        ),
                    });
                    break;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a subscript annotation like `Proto[A, B]` into `(proto_name, [A, B])`.
fn parse_subscript_annotation(text: &str) -> Option<(&str, Vec<String>)> {
    let bracket_pos = text.find('[')?;
    let proto_name = text[..bracket_pos].trim();
    if proto_name.is_empty() {
        return None;
    }

    let inner_start = bracket_pos + 1;
    let inner_end = text.rfind(']')?;
    if inner_end <= inner_start {
        return None;
    }
    let inner = &text[inner_start..inner_end];

    let args = split_top_level_args(inner)
        .into_iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    if args.is_empty() {
        return None;
    }

    Some((proto_name, args))
}

/// Split comma-separated type arguments at the top level (respecting bracket nesting).
fn split_top_level_args(inner: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;

    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                args.push(&inner[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    if start < inner.len() {
        args.push(&inner[start..]);
    }
    args
}

/// Extract the constructor class name from an expression like `ClassName(...)`.
fn extract_constructor_name(expr: &str) -> Option<&str> {
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
fn substitute_typevars(
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
fn types_compatible(expected: &str, actual: &str) -> bool {
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
const fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Skip the `self` or `cls` parameter from a parameter list.
fn skip_self_param(
    params: &[basilisk_resolver::ParameterInfo],
) -> &[basilisk_resolver::ParameterInfo] {
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
fn method_has_typed_self(method: &FunctionInfo) -> bool {
    if let Some(first_param) = method.parameters.first() {
        if first_param.name == "self" && first_param.has_annotation {
            return true;
        }
    }
    false
}

/// Get the `TypeVar` name used in a `self: T` annotation, if present.
fn get_self_typevar_name(method: &FunctionInfo, source: &str) -> Option<String> {
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

/// Check if a concrete method is incompatible with a self-typed protocol method.
///
/// Returns a human-readable description of the mismatch, or `None` if the
/// concrete method is compatible.
///
/// The protocol method uses `self: T` where T is a `TypeVar`, so in the concrete
/// class, every use of T in parameters and return type must use the same concrete
/// type consistently.
fn check_self_typed_method_incompatibility(
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
            // Protocol: `def f(self: T) -> T`
            // Concrete must return `Self` or a compatible self-referential type.
            // A bare `self` without annotation returning `int` is incompatible.
            let concrete_ret_text = concrete_ret.unwrap_or("").trim();
            let concrete_self_text = concrete_self_ann.unwrap_or("self").trim();

            // If concrete has typed self (either Self or a TypeVar), check compatibility.
            if concrete_self_text == "self" {
                // Bare self annotation — check if return type looks like a self-type.
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

    // Check non-self parameters for TypeVar consistency.
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
            // This parameter doesn't use the self TypeVar — skip.
            continue;
        }

        // Protocol uses TypeVar T for this parameter.
        // The concrete method must use the same type consistently.
        let concrete_ann = concrete_param
            .annotation_span
            .and_then(|span| slice_span(source, span))
            .map_or("", str::trim);

        // The concrete parameter type must match what is inferred from the self type.
        // If the concrete self is untyped (bare `self`), any concrete type is valid
        // only if it matches what the return type implies.
        // We flag cases where the concrete parameter type is different from what
        // the protocol's return type (== tv_name) would imply.
        let concrete_ret_text = concrete_ret.unwrap_or("").trim();

        if !concrete_ann.is_empty()
            && concrete_ret_text != "Self"
            && concrete_ret_text != tv_name
            && concrete_ret_text != concrete_ann
            && !concrete_ann.is_empty()
        {
            // The return type and the TypeVar parameter don't match in the concrete class.
            return Some(format!(
                "method `{}` uses `{concrete_ann}` for the `{tv_name}` parameter but \
                 returns `{concrete_ret_text}`; these must match for a self-typed protocol",
                proto_method.name
            ));
        }
    }

    None
}

/// Collect all methods belonging to a class from the module's function list.
fn _collect_class_methods<'a>(
    class_name: &str,
    functions: &'a [FunctionInfo],
) -> Vec<&'a FunctionInfo> {
    functions
        .iter()
        .filter(|f| f.class_name.as_deref() == Some(class_name))
        .collect()
}

/// Compute the byte span of a line in the source for diagnostic anchoring.
fn _line_span(source: &str, line_number: usize) -> Option<Span> {
    let mut current_line = 1;
    let mut start = 0;
    for (i, ch) in source.char_indices() {
        if current_line == line_number {
            let end = source.get(i..).and_then(|s| s.find('\n')).map_or(source.len(), |j| i + j);
            return Some(Span {
                start: u32::try_from(start).ok()?,
                end: u32::try_from(end).ok()?,
            });
        }
        if ch == '\n' {
            current_line += 1;
            start = i + 1;
        }
    }
    None
}
