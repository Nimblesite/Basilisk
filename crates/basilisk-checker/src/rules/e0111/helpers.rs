//! Implements [BSK-E0111] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Helper functions for BSK-E0111: Constructor call errors.

use std::collections::HashMap;

use basilisk_resolver::ClassInfo;

use basilisk_resolver::Span;

use crate::rules::shared::{infer_expr_literal_type, is_type_compatible};
use crate::span_util::slice_span;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

/// Error code for BSK-E0111 diagnostics.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0111",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0111",
};

/// Collect all base class names (simple and subscripted) for a class.
pub(super) fn all_base_names(class_info: &ClassInfo) -> Vec<&str> {
    let mut names: Vec<&str> = class_info
        .bases
        .iter()
        .map(|b| b.split('[').next().unwrap_or(b.as_str()))
        .collect();
    for entry in &class_info.base_subscripts {
        let name = entry.base_name.as_str();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Recursively check if any base class defines `__init__` or `__new__`.
/// Builtin types that have custom `__init__` or `__new__` accepting
/// arguments.  Classes inheriting from these can always be constructed
/// with arguments even though they don't define `__init__` in user code.
const BUILTINS_WITH_INIT: &[&str] = &[
    "str",
    "int",
    "float",
    "bool",
    "bytes",
    "bytearray",
    "complex",
    "list",
    "dict",
    "set",
    "frozenset",
    "tuple",
    "type",
    "range",
    "slice",
    "memoryview",
    "super",
    "property",
    "staticmethod",
    "classmethod",
    "Exception",
    "BaseException",
    "ValueError",
    "TypeError",
    "KeyError",
    "IndexError",
    "AttributeError",
    "RuntimeError",
    "StopIteration",
    "OSError",
    "IOError",
    "UserDict",
    "UserList",
    "UserString",
    "Mapping",
    "MutableMapping",
    "ABC",
];

pub(super) fn has_custom_init_in_bases(
    class_info: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
) -> bool {
    for base_name in all_base_names(class_info) {
        if base_name == "object" || base_name == "Generic" || base_name == "Protocol" {
            continue;
        }

        // Builtin types always have custom __init__/__new__.
        if BUILTINS_WITH_INIT.contains(&base_name) {
            return true;
        }

        // Check if the base class itself defines __init__ or __new__.
        if method_map.contains_key(&(base_name, "__init__"))
            || method_map.contains_key(&(base_name, "__new__"))
        {
            return true;
        }

        // Recurse into the base's bases.
        if let Some(base_class) = class_map.get(base_name) {
            if has_custom_init_in_bases(base_class, class_map, method_map) {
                return true;
            }
        }
    }
    false
}

/// Returns `true` if the class has a base the checker cannot resolve to a known
/// definition — i.e. a base that is not `object`/`Generic`/`Protocol`, not a
/// known builtin, and not a class defined in this module.
///
/// Such a base is an external import (e.g. pydantic `BaseModel`, attrs, msgspec)
/// that may provide an argument-accepting constructor we cannot see. Callers
/// must therefore not conclude the class "inherits only from `object`".
pub(super) fn has_unresolved_base(
    class_info: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
) -> bool {
    all_base_names(class_info).into_iter().any(|base_name| {
        base_name != "object"
            && base_name != "Generic"
            && base_name != "Protocol"
            && !BUILTINS_WITH_INIT.contains(&base_name)
            && !class_map.contains_key(base_name)
    })
}

/// Find `__init__` methods for a class, searching up the MRO.
pub(super) fn find_init_in_hierarchy<'a>(
    class_name: &str,
    class_info: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    method_map: &'a HashMap<(&str, &str), Vec<&'a basilisk_resolver::FunctionInfo>>,
) -> Option<Vec<&'a basilisk_resolver::FunctionInfo>> {
    // Check the class itself first.
    if let Some(funcs) = method_map.get(&(class_name, "__init__")) {
        return Some(funcs.clone());
    }

    // Walk bases (both simple and subscripted).
    for base_name in all_base_names(class_info) {
        if base_name == "object" || base_name == "Generic" || base_name == "Protocol" {
            continue;
        }

        if let Some(funcs) = method_map.get(&(base_name, "__init__")) {
            return Some(funcs.clone());
        }

        if let Some(base_class) = class_map.get(base_name) {
            if let Some(funcs) =
                find_init_in_hierarchy(base_name, base_class, class_map, method_map)
            {
                return Some(funcs);
            }
        }
    }

    None
}

/// Check if `class_name` is a subclass of `base_name` by walking the class hierarchy.
pub(super) fn is_subclass(
    class_name: &str,
    base_name: &str,
    class_map: &HashMap<&str, &ClassInfo>,
) -> bool {
    let Some(class_info) = class_map.get(class_name) else {
        return false;
    };

    for base in all_base_names(class_info) {
        if base == base_name {
            return true;
        }
        if is_subclass(base, base_name, class_map) {
            return true;
        }
    }
    false
}

/// Check if a class is a `NamedTuple` subclass (directly or transitively).
pub(super) fn is_namedtuple_class(
    class_info: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
) -> bool {
    for base_name in all_base_names(class_info) {
        if base_name == "NamedTuple" {
            return true;
        }
        if let Some(base_class) = class_map.get(base_name) {
            if is_namedtuple_class(base_class, class_map) {
                return true;
            }
        }
    }
    false
}

/// Return the tuple fields of a `NamedTuple` class.
///
/// Per the typing spec (namedtuples.html): "A named tuple class can be
/// subclassed, but any fields added by the subclass are not considered part of
/// the named tuple type." So a class that inherits `NamedTuple` *directly*
/// contributes its own annotated attributes as fields; a subclass of another
/// `NamedTuple` class inherits exactly that base's fields and its own added
/// annotations are NOT tuple fields.
pub(super) fn namedtuple_fields<'a>(
    class_info: &'a ClassInfo,
    class_map: &HashMap<&str, &'a ClassInfo>,
) -> Vec<&'a basilisk_resolver::AttributeInfo> {
    let own_annotated = || {
        class_info
            .attributes
            .iter()
            .filter(|a| a.has_annotation)
            .collect::<Vec<_>>()
    };

    // Direct `NamedTuple` inheritance: own annotated attributes are the fields.
    if all_base_names(class_info)
        .into_iter()
        .any(|base| base == "NamedTuple")
    {
        return own_annotated();
    }

    // Subclass of another `NamedTuple` class: inherit that base's fields only.
    for base_name in all_base_names(class_info) {
        if let Some(base_class) = class_map.get(base_name) {
            if is_namedtuple_class(base_class, class_map) {
                return namedtuple_fields(base_class, class_map);
            }
        }
    }

    own_annotated()
}

/// Resolve a string annotation by stripping surrounding quotes.
pub(super) fn resolve_string_annotation(annotation: &str) -> String {
    if (annotation.starts_with('"') && annotation.ends_with('"'))
        || (annotation.starts_with('\'') && annotation.ends_with('\''))
    {
        annotation
            .get(1..annotation.len().saturating_sub(1))
            .unwrap_or(annotation)
            .to_owned()
    } else {
        annotation.to_owned()
    }
}

/// Substitute each `TypeVar` name in `annotation` with its concrete binding,
/// matching whole identifier tokens only (so `T` in `T | None` is replaced but
/// the `T` inside `Type` is not). Example: `T | None` with `{T: int}` → `int | None`.
fn substitute_typevars(annotation: &str, substitutions: &HashMap<&str, &str>) -> String {
    let mut result = String::with_capacity(annotation.len());
    let mut ident = String::new();
    let flush = |ident: &mut String, result: &mut String| {
        if !ident.is_empty() {
            result.push_str(substitutions.get(ident.as_str()).copied().unwrap_or(ident));
            ident.clear();
        }
    };
    for ch in annotation.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            ident.push(ch);
        } else {
            flush(&mut ident, &mut result);
            result.push(ch);
        }
    }
    flush(&mut ident, &mut result);
    result
}

/// Extract type argument texts from a subscript slice expression.
pub(super) fn extract_type_args_text(slice: &ruff_python_ast::Expr, source: &str) -> Vec<String> {
    use ruff_python_ast::Expr;
    use ruff_text_size::Ranged as _;

    match slice {
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .map(|e| {
                let range = e.range();
                source
                    .get(range.start().to_usize()..range.end().to_usize())
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            })
            .collect(),
        other => {
            let range = other.range();
            vec![source
                .get(range.start().to_usize()..range.end().to_usize())
                .unwrap_or("")
                .trim()
                .to_owned()]
        }
    }
}

/// Check Self type incompatibility through inheritance in `__init__`.
#[expect(
    clippy::too_many_arguments,
    reason = "self type incompatibility check needs class map, method map, and call context"
)]
pub(super) fn check_self_type_incompatibility(
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    class_info: &basilisk_resolver::ClassInfo,
    source: &str,
    class_map: &HashMap<&str, &basilisk_resolver::ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // Find __init__ on the class or inherited.
    let init_funcs = find_init_in_hierarchy(class_name, class_info, class_map, method_map);
    let Some(init_funcs) = init_funcs else {
        return;
    };

    for init_func in &init_funcs {
        // Check if any non-self parameter has annotation containing "Self".
        let has_self_annotation = init_func.parameters.iter().skip(1).any(|param| {
            param
                .annotation_span
                .and_then(|span| slice_span(source, span))
                .is_some_and(|ann_text| {
                    let resolved = resolve_string_annotation(ann_text.trim());
                    resolved.contains("Self")
                })
        });

        if !has_self_annotation {
            continue;
        }

        // Check each argument: if it's a call to a parent class, that's an error.
        for arg_expr in &call.arguments.args {
            let ruff_python_ast::Expr::Call(arg_call) = arg_expr else {
                continue;
            };
            let ruff_python_ast::Expr::Name(arg_callee) = arg_call.func.as_ref() else {
                continue;
            };
            let arg_class_name = arg_callee.id.as_str();

            // Skip if the argument is the same class (that's OK).
            if arg_class_name == class_name {
                continue;
            }

            // Check if class_name is a subclass of arg_class_name.
            if is_subclass(class_name, arg_class_name, class_map) {
                let range = call.range();
                let span = Span {
                    start: range.start().to_u32(),
                    end: range.end().to_u32(),
                };

                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "`{arg_class_name}` instance is not compatible with `Self` type \
                         of `{class_name}` in `__init__`"
                    ),
                    span,
                    path,
                    Some(format!(
                        "Pass an instance of `{class_name}` (or a subclass) instead of `{arg_class_name}`"
                    )),
                    Some(format!(
                        "`Self` in `__init__` of `{class_name}` refers to `{class_name}`, \
                         not the base class `{arg_class_name}`"
                    )),
                ));
            }
        }
    }
}

/// Check arguments to `__init__` after type parameter substitution.
#[expect(
    clippy::too_many_arguments,
    reason = "init method validation requires substitutions, call, class info and context"
)]
pub(super) fn check_init_method_args(
    init_func: &basilisk_resolver::FunctionInfo,
    substitutions: &HashMap<&str, &str>,
    call: &ruff_python_ast::ExprCall,
    class_name: &str,
    type_args: &[String],
    source: &str,
    path: &str,
    class_info: &basilisk_resolver::ClassInfo,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // Check 3: Explicit self annotation mismatch.
    if let Some(self_param) = init_func.parameters.first() {
        if let Some(ann_span) = self_param.annotation_span {
            if let Some(ann_text) = slice_span(source, ann_span) {
                let resolved = resolve_string_annotation(ann_text.trim());
                check_self_param_init_mismatch(
                    &resolved,
                    class_name,
                    type_args,
                    call,
                    path,
                    class_info,
                    typevar_names,
                    diagnostics,
                );
            }
        }
    }

    // Check 1: Non-self parameters for type mismatch after substitution.
    let non_self_params: Vec<&basilisk_resolver::ParameterInfo> =
        init_func.parameters.iter().skip(1).collect();

    // If __init__ accepts *args/**kwargs (passthrough), skip argument checking.
    if init_func.vararg.is_some() {
        return;
    }

    for (arg_idx, arg_expr) in call.arguments.args.iter().enumerate() {
        let Some(param) = non_self_params.get(arg_idx) else {
            break;
        };

        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };

        let ann_trimmed = ann_text.trim();

        // Substitute type parameters in the annotation (handles composite forms
        // such as `T | None`, where the whole string is not a substitution key).
        let resolved_type = substitute_typevars(ann_trimmed, substitutions);

        // If the resolved type is still a TypeVar name (function-scoped, not
        // class-scoped), it can accept any type — skip the check.
        if typevar_names.contains(&resolved_type.as_str()) {
            continue;
        }

        // Classify the argument expression type.
        let Some(arg_type) = infer_expr_literal_type(arg_expr) else {
            continue;
        };

        // Check compatibility.
        if !is_type_compatible(arg_type, &resolved_type) {
            let range = arg_expr.range();
            let span = Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            };
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument type `{arg_type}` is incompatible with parameter `{}` \
                     of type `{resolved_type}` in `{class_name}.__init__`",
                    param.name
                ),
                span,
                path,
                Some(format!(
                    "Pass a value of type `{resolved_type}` for parameter `{}`",
                    param.name
                )),
                Some(format!(
                    "`{class_name}` is specialized with type arguments `[{}]`, \
                     binding `{}` to `{resolved_type}`",
                    type_args.join(", "),
                    ann_trimmed
                )),
            ));
        }
    }
}

/// Check if the `self` parameter annotation in `__init__` is incompatible with
/// the provided type arguments.
#[expect(
    clippy::too_many_arguments,
    reason = "all args needed for mismatch check"
)]
pub(super) fn check_self_param_init_mismatch(
    self_annotation: &str,
    class_name: &str,
    type_args: &[String],
    call: &ruff_python_ast::ExprCall,
    path: &str,
    class_info: &basilisk_resolver::ClassInfo,
    typevar_names: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_text_size::Ranged as _;

    // Looking for annotations like "Class4[int]"
    let Some(bracket_start) = self_annotation.find('[') else {
        return;
    };
    let Some(bracket_end) = self_annotation.rfind(']') else {
        return;
    };

    let ann_class_name = self_annotation[..bracket_start].trim();
    if ann_class_name != class_name {
        return;
    }

    let args_str = &self_annotation[bracket_start + 1..bracket_end];
    let ann_type_args: Vec<&str> = args_str.split(',').map(str::trim).collect();

    // Check if annotation args contain class-scoped or function-scoped type vars.
    let generic_param_names: Vec<&str> =
        basilisk_resolver::collect_names(&class_info.generic_params);

    // If all annotation args are fixed (not type variables), check for mismatch.
    let all_fixed = ann_type_args
        .iter()
        .all(|arg| !generic_param_names.contains(arg) && !typevar_names.contains(arg));

    if !all_fixed {
        return;
    }

    // The annotation has fixed type args (e.g. `Class4[int]`).
    if type_args.len() != ann_type_args.len() {
        return;
    }

    let all_match = type_args
        .iter()
        .zip(ann_type_args.iter())
        .all(|(provided, expected)| provided.as_str() == *expected);

    if !all_match {
        let range = call.range();
        let span = Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        };
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "`{class_name}[{}]()` is incompatible: `__init__` expects \
                 `self: {self_annotation}` but received `{class_name}[{}]`",
                type_args.join(", "),
                type_args.join(", ")
            ),
            span,
            path,
            Some(format!(
                "Use `{class_name}[{}]()` to match the expected `self` parameter type",
                ann_type_args.join(", ")
            )),
            Some(format!(
                "The `__init__` method constrains `self` to `{self_annotation}`"
            )),
        ));
    }
}
