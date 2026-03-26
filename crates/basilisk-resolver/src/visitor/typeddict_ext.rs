//! Typeddict Ext visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAnnAssign, StmtAssign};

use crate::scope::{Span, TypedDictKeyViolation, TypedDictKeyViolationKind};

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;
use super::typeddict::TdFieldMap;

/// Determine whether a `TypedDict` field is required, accounting for
/// `Required[...]`, `NotRequired[...]`, and `ReadOnly[...]` wrappers.
///
/// - `is_total=true` (default):  all fields are required unless wrapped in `NotRequired`.
/// - `is_total=false`:  all fields are not-required unless wrapped in `Required`.
fn is_field_required(
    field_name: &str,
    field_types: &std::collections::HashMap<&str, String>,
    is_total: bool,
) -> bool {
    let Some(ann) = field_types.get(field_name) else {
        return is_total;
    };
    let ann_lower = ann.to_ascii_lowercase();
    // Strip outer wrappers to find Required/NotRequired regardless of nesting.
    if ann_lower.contains("notrequired") {
        return false;
    }
    if ann_lower.contains("required") {
        return true;
    }
    is_total
}

pub(super) fn td_check_regular_assign(
    node: &StmtAssign,
    var_type: &std::collections::HashMap<String, String>,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    for target in &node.targets {
        let Some(var_name) = expr_simple_name(target) else {
            continue;
        };
        let Some(class_name) = var_type.get(&var_name) else {
            continue;
        };
        let Some((all_fields, field_types, is_total, extra_items_type)) =
            fields.get(class_name.as_str())
        else {
            continue;
        };
        let Expr::Dict(dict) = node.value.as_ref() else {
            continue;
        };

        // Flag any non-literal (variable) key in the dict
        let has_non_literal = dict.items.iter().any(|item| {
            item.key
                .as_ref()
                .is_some_and(|k| !matches!(k, Expr::StringLiteral(_)))
        });
        if has_non_literal {
            out.push(TypedDictKeyViolation {
                span: text_range_to_span(node.range()),
                class_name: class_name.clone(),
                kind: TypedDictKeyViolationKind::NonLiteralDictKey,
            });
            continue;
        }

        let literal_keys: Vec<String> = dict
            .items
            .iter()
            .filter_map(|item| {
                let Expr::StringLiteral(s) = item.key.as_ref()? else {
                    return None;
                };
                Some(s.value.to_string())
            })
            .collect();

        // When `extra_items` is set, unknown keys are allowed.
        let invalid_keys: Vec<String> = if extra_items_type.is_some() {
            Vec::new()
        } else {
            literal_keys
                .iter()
                .filter(|k| !all_fields.contains(&k.as_str()))
                .cloned()
                .collect()
        };
        let missing_keys: Vec<String> = all_fields
            .iter()
            .filter(|&&f| !literal_keys.iter().any(|k| k == f))
            .filter(|&&f| is_field_required(f, field_types, *is_total))
            .map(|s| (*s).to_owned())
            .collect();

        if !invalid_keys.is_empty() || !missing_keys.is_empty() {
            out.push(TypedDictKeyViolation {
                span: text_range_to_span(node.range()),
                class_name: class_name.clone(),
                kind: TypedDictKeyViolationKind::InvalidDictLiteral {
                    invalid_keys,
                    missing_keys,
                },
            });
        }

        // Check value types for each key-value pair.
        check_dict_item_types(
            dict,
            all_fields,
            field_types,
            extra_items_type.as_deref(),
            class_name,
            node.range(),
            out,
        );
    }
}

/// Check annotated assignments `var: TypedDict = {...}`.
pub(super) fn td_check_ann_assign(
    node: &StmtAnnAssign,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    let Some(value) = &node.value else { return };
    let Expr::Name(ann_name) = node.annotation.as_ref() else {
        return;
    };
    let class_name = ann_name.id.as_str();
    let Some((all_fields, field_types, is_total, extra_items_type)) = fields.get(class_name) else {
        return;
    };
    let Expr::Dict(dict) = value.as_ref() else {
        return;
    };

    // Flag any non-literal (variable) key
    let has_non_literal = dict.items.iter().any(|item| {
        item.key
            .as_ref()
            .is_some_and(|k| !matches!(k, Expr::StringLiteral(_)))
    });
    if has_non_literal {
        out.push(TypedDictKeyViolation {
            span: text_range_to_span(node.range()),
            class_name: class_name.to_owned(),
            kind: TypedDictKeyViolationKind::NonLiteralDictKey,
        });
        return;
    }

    let literal_keys: Vec<String> = dict
        .items
        .iter()
        .filter_map(|item| {
            let Expr::StringLiteral(s) = item.key.as_ref()? else {
                return None;
            };
            Some(s.value.to_string())
        })
        .collect();

    // When `extra_items` is set, unknown keys are allowed.
    let invalid_keys: Vec<String> = if extra_items_type.is_some() {
        Vec::new()
    } else {
        literal_keys
            .iter()
            .filter(|k| !all_fields.contains(&k.as_str()))
            .cloned()
            .collect()
    };
    let missing_keys: Vec<String> = all_fields
        .iter()
        .filter(|&&f| !literal_keys.iter().any(|k| k == f))
        .filter(|&&f| is_field_required(f, field_types, *is_total))
        .map(|s| (*s).to_owned())
        .collect();

    if !invalid_keys.is_empty() || !missing_keys.is_empty() {
        out.push(TypedDictKeyViolation {
            span: text_range_to_span(node.range()),
            class_name: class_name.to_owned(),
            kind: TypedDictKeyViolationKind::InvalidDictLiteral {
                invalid_keys,
                missing_keys,
            },
        });
    }

    // Check value types in dict literal against field types.
    let Some((_, field_types, _, extra_type)) = fields.get(class_name) else {
        return;
    };
    check_dict_value_types(
        dict,
        field_types,
        all_fields,
        class_name,
        node.range(),
        fields,
        out,
    );
    check_extra_items_values(
        dict,
        all_fields,
        extra_type.as_deref(),
        class_name,
        node.range(),
        out,
    );
}

/// Check value types for extra keys against `extra_items` type.
fn check_extra_items_values(
    dict: &ruff_python_ast::ExprDict,
    all_fields: &[&str],
    extra_items_type: Option<&str>,
    class_name: &str,
    span_range: ruff_text_size::TextRange,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    let Some(expected) = extra_items_type else {
        return;
    };
    let expected = strip_td_wrappers(expected);
    for item in &dict.items {
        let Some(Expr::StringLiteral(s)) = &item.key else {
            continue;
        };
        let key = s.value.to_string();
        if all_fields.contains(&key.as_str()) {
            continue;
        }
        let Some(actual) = expr_literal_type_name(&item.value) else {
            continue;
        };
        if !typeddict_field_type_compatible(actual, expected) {
            out.push(TypedDictKeyViolation {
                span: text_range_to_span(span_range),
                class_name: class_name.to_owned(),
                kind: TypedDictKeyViolationKind::WrongSubscriptValueType {
                    key,
                    expected: expected.to_owned(),
                },
            });
        }
    }
}

/// Check value types for items in a regular assign dict.
fn check_dict_item_types(
    dict: &ruff_python_ast::ExprDict,
    all_fields: &[&str],
    field_types: &std::collections::HashMap<&str, String>,
    extra_items_type: Option<&str>,
    class_name: &str,
    span_range: ruff_text_size::TextRange,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    for item in &dict.items {
        let Some(Expr::StringLiteral(s)) = &item.key else {
            continue;
        };
        let key = s.value.to_string();
        let Some(actual) = expr_literal_type_name(&item.value) else {
            continue;
        };
        if all_fields.contains(&key.as_str()) {
            if let Some(expected) = field_types.get(key.as_str()) {
                if !typeddict_field_type_compatible(actual, expected) {
                    out.push(TypedDictKeyViolation {
                        span: text_range_to_span(span_range),
                        class_name: class_name.to_owned(),
                        kind: TypedDictKeyViolationKind::WrongSubscriptValueType {
                            key,
                            expected: expected.clone(),
                        },
                    });
                }
            }
        } else if let Some(expected) = extra_items_type {
            let stripped = strip_td_wrappers(expected);
            if !typeddict_field_type_compatible(actual, stripped) {
                out.push(TypedDictKeyViolation {
                    span: text_range_to_span(span_range),
                    class_name: class_name.to_owned(),
                    kind: TypedDictKeyViolationKind::WrongSubscriptValueType {
                        key,
                        expected: stripped.to_owned(),
                    },
                });
            }
        }
    }
}

/// Recursively check value types in a dict literal against `TypedDict` field types.
fn check_dict_value_types(
    dict: &ruff_python_ast::ExprDict,
    field_types: &std::collections::HashMap<&str, String>,
    all_fields: &[&str],
    class_name: &str,
    span_range: ruff_text_size::TextRange,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    for item in &dict.items {
        let Some(Expr::StringLiteral(s)) = &item.key else {
            continue;
        };
        let key = s.value.to_string();
        if !all_fields.contains(&key.as_str()) {
            continue;
        }
        let Some(expected) = field_types.get(key.as_str()) else {
            continue;
        };

        // Primitive literal value — check type directly.
        if let Some(actual) = expr_literal_type_name(&item.value) {
            if !typeddict_field_type_compatible(actual, expected) {
                out.push(TypedDictKeyViolation {
                    span: text_range_to_span(span_range),
                    class_name: class_name.to_owned(),
                    kind: TypedDictKeyViolationKind::WrongSubscriptValueType {
                        key,
                        expected: expected.clone(),
                    },
                });
            }
            continue;
        }

        // Nested dict literal — if the expected type is a TypedDict, recurse.
        if let Expr::Dict(nested_dict) = &item.value {
            if let Some((nested_fields, nested_types, _, _)) = fields.get(expected.as_str()) {
                check_dict_value_types(
                    nested_dict,
                    nested_types,
                    nested_fields,
                    expected,
                    nested_dict.range(),
                    fields,
                    out,
                );
            }
        }
    }
}

/// Walk an expression and report subscript reads with invalid `TypedDict` keys.
pub(super) fn td_check_expr_reads(
    expr: &Expr,
    var_type: &std::collections::HashMap<String, String>,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    match expr {
        Expr::Subscript(sub) => {
            if let Some(var_name) = expr_simple_name(&sub.value) {
                if let Some(class_name) = var_type.get(&var_name) {
                    if let Some((all_fields, _, _, extra_items_type)) =
                        fields.get(class_name.as_str())
                    {
                        if let Expr::StringLiteral(key_str) = sub.slice.as_ref() {
                            let key = key_str.value.to_string();
                            if !all_fields.contains(&key.as_str()) && extra_items_type.is_none() {
                                out.push(TypedDictKeyViolation {
                                    span: text_range_to_span(sub.range()),
                                    class_name: class_name.clone(),
                                    kind: TypedDictKeyViolationKind::SubscriptReadInvalidKey {
                                        key,
                                    },
                                });
                            }
                        } else if extra_items_type.is_none() {
                            // Non-literal key access on a TypedDict without extra_items
                            out.push(TypedDictKeyViolation {
                                span: text_range_to_span(sub.range()),
                                class_name: class_name.clone(),
                                kind: TypedDictKeyViolationKind::NonLiteralDictKey,
                            });
                        }
                    }
                }
            }
            td_check_expr_reads(&sub.value, var_type, fields, out);
            td_check_expr_reads(&sub.slice, var_type, fields, out);
        }
        Expr::Call(call) => {
            td_check_expr_reads(&call.func, var_type, fields, out);
            for arg in &call.arguments.args {
                td_check_expr_reads(arg, var_type, fields, out);
            }
        }
        Expr::BinOp(binop) => {
            td_check_expr_reads(&binop.left, var_type, fields, out);
            td_check_expr_reads(&binop.right, var_type, fields, out);
        }
        Expr::UnaryOp(unary) => td_check_expr_reads(&unary.operand, var_type, fields, out),
        _ => {}
    }
}

/// Return the inferred type name for a literal expression, or `None` if not a literal.
pub(super) fn expr_literal_type_name(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::StringLiteral(_) | Expr::FString(_) => Some("str"),
        Expr::NumberLiteral(n) => Some(match n.value {
            ruff_python_ast::Number::Float(_) => "float",
            ruff_python_ast::Number::Complex { .. } => "complex",
            ruff_python_ast::Number::Int(_) => "int",
        }),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

/// Return `true` if an actual literal type is compatible with an expected `TypedDict` field type.
pub(super) fn typeddict_field_type_compatible(actual: &str, expected: &str) -> bool {
    let stripped = strip_td_wrappers(expected);
    if actual == stripped
        || (actual == "bool" && stripped == "int")
        || (actual == "int" && stripped == "float")
    {
        return true;
    }
    // Handle union types: `int` is compatible with `int | None`, etc.
    if stripped.contains(" | ") {
        return stripped
            .split(" | ")
            .map(str::trim)
            .any(|variant| typeddict_field_type_compatible(actual, variant));
    }
    false
}

/// Strip `Required[...]`, `NotRequired[...]`, `ReadOnly[...]`, and
/// `Annotated[..., meta]` wrappers from a `TypedDict` field annotation to
/// extract the underlying type.
fn strip_td_wrappers(annotation: &str) -> &str {
    let mut result = annotation.trim();
    loop {
        let lower = result.to_ascii_lowercase();
        if let Some(inner) = try_strip_wrapper(&lower, result, "required[")
            .or_else(|| try_strip_wrapper(&lower, result, "notrequired["))
            .or_else(|| try_strip_wrapper(&lower, result, "readonly["))
        {
            result = inner.trim();
            continue;
        }
        // Annotated[T, ...] — keep only the first type arg.
        if let Some(inner) = try_strip_wrapper(&lower, result, "annotated[") {
            if let Some(comma) = inner.find(',') {
                result = inner[..comma].trim();
                continue;
            }
            result = inner.trim();
            continue;
        }
        break;
    }
    result
}

/// Try to strip a wrapper prefix (case-insensitive) and its matching `]`.
fn try_strip_wrapper<'a>(lower: &str, original: &'a str, prefix: &str) -> Option<&'a str> {
    if !lower.starts_with(prefix) || !original.ends_with(']') {
        return None;
    }
    Some(&original[prefix.len()..original.len() - 1])
}

/// Collect `ReadOnly` violations from module-level statements and function bodies.
pub(super) fn collect_isinstance_typeddict_violations(
    stmts: &[Stmt],
    typeddict_names: &std::collections::HashSet<&str>,
) -> Vec<Span> {
    let mut out = Vec::new();
    collect_isinstance_typeddict_in_stmts(stmts, typeddict_names, &mut out);
    out
}

pub(super) fn collect_isinstance_typeddict_in_stmts(
    stmts: &[Stmt],
    typeddict_names: &std::collections::HashSet<&str>,
    out: &mut Vec<Span>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::If(node) => {
                collect_isinstance_typeddict_in_expr(&node.test, typeddict_names, out);
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
                for clause in &node.elif_else_clauses {
                    if let Some(test) = &clause.test {
                        collect_isinstance_typeddict_in_expr(test, typeddict_names, out);
                    }
                    collect_isinstance_typeddict_in_stmts(&clause.body, typeddict_names, out);
                }
            }
            Stmt::Expr(node) => {
                collect_isinstance_typeddict_in_expr(&node.value, typeddict_names, out);
            }
            Stmt::Assign(node) => {
                collect_isinstance_typeddict_in_expr(&node.value, typeddict_names, out);
            }
            Stmt::AnnAssign(node) => {
                if let Some(val) = &node.value {
                    collect_isinstance_typeddict_in_expr(val, typeddict_names, out);
                }
            }
            Stmt::While(node) => {
                collect_isinstance_typeddict_in_expr(&node.test, typeddict_names, out);
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
            }
            Stmt::For(node) => {
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
            }
            Stmt::FunctionDef(node) => {
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
            }
            Stmt::ClassDef(node) => {
                collect_isinstance_typeddict_in_stmts(&node.body, typeddict_names, out);
            }
            _ => {}
        }
    }
}

pub(super) fn collect_isinstance_typeddict_in_expr(
    expr: &Expr,
    typeddict_names: &std::collections::HashSet<&str>,
    out: &mut Vec<Span>,
) {
    use ruff_text_size::Ranged as _;
    let Expr::Call(call) = expr else { return };
    let callee_is_isinstance = matches!(
        call.func.as_ref(),
        Expr::Name(n) if n.id == "isinstance" || n.id == "issubclass"
    );
    if !callee_is_isinstance {
        return;
    }
    let Some(second_arg) = call.arguments.args.get(1) else {
        return;
    };
    if let Expr::Name(name) = second_arg {
        if typeddict_names.contains(name.id.as_str()) {
            let range = call.range();
            out.push(Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// TypeVar bound=TypedDict detection
// ---------------------------------------------------------------------------
