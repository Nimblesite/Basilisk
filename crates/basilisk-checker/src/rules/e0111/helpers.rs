//! Helper functions for BSK-E0111: Constructor call errors.

use std::collections::HashMap;

use basilisk_resolver::ClassInfo;

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
