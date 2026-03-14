//! Helper functions for BSK-E0111: Constructor call errors.

use std::collections::HashMap;

use basilisk_resolver::ClassInfo;

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

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
pub(super) fn has_custom_init_in_bases(
    class_info: &ClassInfo,
    class_map: &HashMap<&str, &ClassInfo>,
    method_map: &HashMap<(&str, &str), Vec<&basilisk_resolver::FunctionInfo>>,
) -> bool {
    for base_name in all_base_names(class_info) {
        if base_name == "object" || base_name == "Generic" || base_name == "Protocol" {
            continue;
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

/// Classify the Python type of a literal expression.
pub(super) fn classify_literal_type(expr: &ruff_python_ast::Expr) -> Option<&'static str> {
    use ruff_python_ast::Expr;
    match expr {
        Expr::StringLiteral(_) => Some("str"),
        Expr::NumberLiteral(num) => {
            if num.value.is_int() {
                Some("int")
            } else {
                Some("float")
            }
        }
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Check if an argument type is compatible with a parameter type.
pub(super) fn is_type_compatible(arg_type: &str, param_type: &str) -> bool {
    if arg_type == param_type {
        return true;
    }
    if param_type == "Any" || param_type == "object" {
        return true;
    }
    if param_type == "int" && arg_type == "bool" {
        return true;
    }
    if param_type == "float" && (arg_type == "int" || arg_type == "bool") {
        return true;
    }
    if param_type == "complex" && (arg_type == "int" || arg_type == "float" || arg_type == "bool") {
        return true;
    }
    if param_type.contains('|') {
        return param_type
            .split('|')
            .any(|part| is_type_compatible(arg_type, part.trim()));
    }
    false
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


fn check_self_type_incompatibility(
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
                .and_then(|span| source.get(span.start as usize..span.end as usize))
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

                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "`{arg_class_name}` instance is not compatible with `Self` type \
                         of `{class_name}` in `__init__`"
                    ),
                    span,
                    path: path.to_owned(),
                    help: Some(format!(
                        "Pass an instance of `{class_name}` (or a subclass) instead of `{arg_class_name}`"
                    )),
                    note: Some(format!(
                        "`Self` in `__init__` of `{class_name}` refers to `{class_name}`, \
                         not the base class `{arg_class_name}`"
                    )),
                });
            }
        }
    }
}

/// Find `__init__` methods for a class, searching up the MRO.

fn check_init_method_args(
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
            if let Some(ann_text) = source.get(ann_span.start as usize..ann_span.end as usize) {
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
        let Some(ann_text) = source.get(ann_span.start as usize..ann_span.end as usize) else {
            continue;
        };

        let ann_trimmed = ann_text.trim();

        // Substitute type parameters in the annotation.
        let resolved_type = if let Some(replacement) = substitutions.get(ann_trimmed) {
            (*replacement).to_owned()
        } else {
            ann_trimmed.to_owned()
        };

        // If the resolved type is still a TypeVar name (function-scoped, not
        // class-scoped), it can accept any type — skip the check.
        if typevar_names.contains(&resolved_type.as_str()) {
            continue;
        }

        // Classify the argument expression type.
        let Some(arg_type) = classify_literal_type(arg_expr) else {
            continue;
        };

        // Check compatibility.
        if !is_type_compatible(arg_type, &resolved_type) {
            let range = arg_expr.range();
            let span = Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            };
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Argument type `{arg_type}` is incompatible with parameter `{}` \
                     of type `{resolved_type}` in `{class_name}.__init__`",
                    param.name
                ),
                span,
                path: path.to_owned(),
                help: Some(format!(
                    "Pass a value of type `{resolved_type}` for parameter `{}`",
                    param.name
                )),
                note: Some(format!(
                    "`{class_name}` is specialized with type arguments `[{}]`, \
                     binding `{}` to `{resolved_type}`",
                    type_args.join(", "),
                    ann_trimmed
                )),
            });
        }
    }
}

/// Check if the `self` parameter annotation in `__init__` is incompatible with
/// the provided type arguments.
#[allow(clippy::too_many_arguments)]

fn check_self_param_init_mismatch(
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
    let generic_param_names: Vec<&str> = class_info
        .generic_params
        .iter()
        .map(|p| p.name.as_str())
        .collect();

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
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "`{class_name}[{}]()` is incompatible: `__init__` expects \
                 `self: {self_annotation}` but received `{class_name}[{}]`",
                type_args.join(", "),
                type_args.join(", ")
            ),
            span,
            path: path.to_owned(),
            help: Some(format!(
                "Use `{class_name}[{}]()` to match the expected `self` parameter type",
                ann_type_args.join(", ")
            )),
            note: Some(format!(
                "The `__init__` method constrains `self` to `{self_annotation}`"
            )),
        });
    }
}
