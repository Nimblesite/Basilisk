//! Typeddict visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAssign};
use ruff_text_size::Ranged;

use crate::scope::{
    ClassInfo, TypedDictCallInfo, TypedDictKeyViolation, TypedDictKeyViolationKind,
    TypedDictSecondArgKind,
};

use super::annotations::strip_annotated_wrapper;
use super::class_info_ext::expr_simple_name;
use super::core::{check_td_stmts, source_slice_span, text_range_to_span};
use super::final_readonly::TYPING_FORMS;
use super::typeddict_ext::{expr_literal_type_name, typeddict_field_type_compatible};

pub(super) fn collect_typeddict_calls(stmts: &[Stmt]) -> Vec<TypedDictCallInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Expr::Call(call) = node.value.as_ref() else {
            continue;
        };
        // Callee must be `TypedDict` or `typing.TypedDict`.
        let is_typeddict = if let Some(name) = expr_simple_name(&call.func) {
            name == "TypedDict"
        } else if let Expr::Attribute(attr) = call.func.as_ref() {
            attr.attr.as_str() == "TypedDict"
        } else {
            false
        };
        if !is_typeddict {
            continue;
        }
        // Determine the LHS name.
        let Some(lhs_name) = node.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        // First positional arg: the declared name (must be a string literal).
        let declared_name = call.arguments.args.first().and_then(|arg| {
            if let Expr::StringLiteral(s) = arg {
                Some(s.value.to_string())
            } else {
                None
            }
        });
        // Second positional arg: expected to be a dict literal.
        let has_positional_dict;
        let (second_arg_kind, has_non_string_key) =
            if let Some(second_arg) = call.arguments.args.get(1) {
                has_positional_dict = true;
                if let Expr::Dict(dict) = second_arg {
                    // Check if every key is a string literal.
                    let non_string = dict.items.iter().any(|item| {
                        item.key
                            .as_ref()
                            .is_some_and(|k| !matches!(k, Expr::StringLiteral(_)))
                    });
                    (TypedDictSecondArgKind::DictLiteral, non_string)
                } else {
                    (TypedDictSecondArgKind::NotDictLiteral, false)
                }
            } else {
                // No second arg — keyword syntax or zero args; treat as dict literal
                // variant since we don't flag keyword-only syntax here.
                has_positional_dict = false;
                (TypedDictSecondArgKind::DictLiteral, false)
            };
        let keyword_names: Vec<String> = call
            .arguments
            .keywords
            .iter()
            .filter_map(|kw| kw.arg.as_ref().map(std::string::ToString::to_string))
            .collect();
        out.push(TypedDictCallInfo {
            lhs_name,
            declared_name,
            second_arg_kind,
            has_non_string_key,
            has_positional_dict,
            keyword_names,
            span: text_range_to_span(call.range()),
        });
    }
    out
}

/// Collect module-level `NewType(...)` call sites.
///
/// Matches assignments of the form `Name = NewType("Name", BaseType)`.
pub(super) fn expr_is_parameterized(expr: &Expr) -> bool {
    match expr {
        Expr::Subscript(sub) => {
            // Skip well-known typing forms: Literal["x"], Optional[T], etc.
            let base_name = expr_simple_name(&sub.value);
            if base_name
                .as_deref()
                .is_some_and(|n| TYPING_FORMS.contains(&n))
            {
                return false;
            }
            true
        }
        Expr::BinOp(bin) => expr_is_parameterized(&bin.left) || expr_is_parameterized(&bin.right),
        Expr::Tuple(tup) => tup.elts.iter().any(expr_is_parameterized),
        _ => false,
    }
}

/// Extracts the name from a PEP 695 `TypeParam` (`TypeVar`, `TypeVarTuple`, or `ParamSpec`).
pub(super) fn resolve_actual_type(
    expr: &Expr,
    params: &[(&str, &str)],
    _source: &str,
) -> Option<String> {
    match expr {
        Expr::Name(name) => {
            let param_name = name.id.as_str();
            params
                .iter()
                .find(|(n, _)| *n == param_name)
                .and_then(|(_, ann)| {
                    let normalized = normalize_type_str(ann);
                    // If the normalized annotation still contains quotes, it has forward
                    // references inside subscripts (e.g. `list["ClassA"]`) that we cannot
                    // resolve textually — skip the check to avoid false positives.
                    if normalized.contains('"') || normalized.contains('\'') {
                        None
                    } else {
                        Some(normalized)
                    }
                })
        }
        Expr::StringLiteral(_) => Some("str".to_owned()),
        Expr::NumberLiteral(n) => {
            if matches!(n.value, ruff_python_ast::Number::Float(_)) {
                Some("float".to_owned())
            } else {
                Some("int".to_owned())
            }
        }
        Expr::BooleanLiteral(_) => Some("bool".to_owned()),
        Expr::BytesLiteral(_) => Some("bytes".to_owned()),
        Expr::NoneLiteral(_) => Some("None".to_owned()),
        // Complex expressions (attribute access, subscripts, calls, binary ops, etc.)
        // cannot be typed without full type inference — returning source text produces
        // false positives when compared against expected types textually.
        _ => None,
    }
}

/// Extract the text of a type expression (the second argument to `assert_type`).
pub(super) fn normalize_type_str(ann: &str) -> String {
    let trimmed = ann.trim();
    // Strip Annotated[T, ...] → take first argument only.
    if let Some(inner) = strip_annotated_wrapper(trimmed) {
        return normalize_type_str(inner);
    }
    // Strip outer string quotes (forward references like `"list[int]"` or `'MyClass'`).
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return normalize_type_str(&trimmed[1..trimmed.len() - 1]);
    }
    trimmed.to_owned()
}

/// If `ann` starts with `Annotated[`, return the first type argument (the actual type).
pub(super) fn build_var_type_map<'a>(
    stmts: &[Stmt],
    td_readonly_fields: &'a std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> std::collections::HashMap<String, &'a str> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Some(var_name) = expr_simple_name(&ann.target) else {
            continue;
        };
        let Expr::Name(type_name) = ann.annotation.as_ref() else {
            continue;
        };
        if let Some((key, _)) = td_readonly_fields.get_key_value(type_name.id.as_str()) {
            let _ = map.insert(var_name, key.as_str());
        }
    }
    map
}

/// Collect `TypedDict` key/value violations from module-level statements and function bodies.
///
/// Detects:
/// - Subscript assignments with invalid keys: `movie["director"] = "Ridley Scott"`
/// - Subscript assignments with wrong value type: `movie["year"] = "1982"`
/// - Annotated dict literal assignments with invalid or missing keys
/// - Regular dict assignments to `TypedDict` variables with wrong keys/types
/// - Subscript read access with invalid keys: `print(movie["unknown"])`
/// - Disallowed method calls: `movie.clear()`
/// - Delete operations on required `TypedDict` keys: `del movie["name"]`
pub(super) fn collect_typeddict_key_violations<'a>(
    stmts: &[Stmt],
    classes: &'a [ClassInfo],
    source: &'a str,
) -> Vec<TypedDictKeyViolation> {
    use std::collections::HashMap;
    // (all_fields, field_types, is_total, extra_items_type)
    type FieldMap<'x> =
        HashMap<&'x str, (Vec<&'x str>, HashMap<&'x str, String>, bool, Option<String>)>;

    let typeddict_fields: FieldMap<'a> = classes
        .iter()
        .filter(|c| c.is_typed_dict)
        .map(|c| {
            let all_fields: Vec<&str> = c.attributes.iter().map(|a| a.name.as_str()).collect();
            let field_types: HashMap<&str, String> = c
                .attributes
                .iter()
                .filter_map(|a| {
                    let span = a.annotation_span?;
                    let type_text = source_slice_span(source, span)?.trim().to_owned();
                    Some((a.name.as_str(), type_text))
                })
                .collect();
            let extra_items_type = c.typeddict_extra_items_type.clone();
            (
                c.name.as_str(),
                (
                    all_fields,
                    field_types,
                    c.is_typeddict_total,
                    extra_items_type,
                ),
            )
        })
        .collect();

    if typeddict_fields.is_empty() {
        return Vec::new();
    }

    let var_type = td_var_type_from_stmts(stmts, &typeddict_fields);
    let mut out = Vec::new();
    check_td_stmts(&typeddict_fields, &var_type, stmts, &mut out);
    out
}

/// `(all_fields, field_types, is_total, extra_items_type)` map keyed by class name.
pub(super) type TdFieldMap<'a> = std::collections::HashMap<
    &'a str,
    (
        Vec<&'a str>,
        std::collections::HashMap<&'a str, String>,
        bool,
        Option<String>,
    ),
>;

/// Build a variable-name → TypedDict-class-name map from annotated assignments in `stmts`.
pub(super) fn td_var_type_from_stmts(
    stmts: &[Stmt],
    fields: &TdFieldMap<'_>,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Some(var_name) = expr_simple_name(&ann.target) else {
            continue;
        };
        let Expr::Name(type_name) = ann.annotation.as_ref() else {
            continue;
        };
        let class_name = type_name.id.as_str();
        if fields.contains_key(class_name) {
            let _ = map.insert(var_name, class_name.to_owned());
        }
    }
    map
}

/// Recursively check statements for `TypedDict` violations.
pub(super) fn td_check_subscript_assign(
    node: &StmtAssign,
    var_type: &std::collections::HashMap<String, String>,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    for target in &node.targets {
        let Expr::Subscript(sub) = target else {
            continue;
        };
        let Some(var_name) = expr_simple_name(&sub.value) else {
            continue;
        };
        let Some(class_name) = var_type.get(&var_name) else {
            continue;
        };
        let Some((all_fields, field_types, _, extra_items_type)) = fields.get(class_name.as_str())
        else {
            continue;
        };
        let Expr::StringLiteral(key_str) = sub.slice.as_ref() else {
            continue;
        };
        let key = key_str.value.to_string();
        if !all_fields.contains(&key.as_str()) && extra_items_type.is_none() {
            out.push(TypedDictKeyViolation {
                span: text_range_to_span(node.range()),
                class_name: class_name.clone(),
                kind: TypedDictKeyViolationKind::InvalidSubscriptKey { key },
            });
        } else if let Some(expected) = field_types.get(key.as_str()) {
            if let Some(actual) = expr_literal_type_name(&node.value) {
                if !typeddict_field_type_compatible(actual, expected) {
                    out.push(TypedDictKeyViolation {
                        span: text_range_to_span(node.range()),
                        class_name: class_name.clone(),
                        kind: TypedDictKeyViolationKind::WrongSubscriptValueType {
                            key,
                            expected: expected.clone(),
                        },
                    });
                }
            }
        }
    }
}
