//! Typeddict Ext visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAnnAssign, StmtAssign};

use crate::scope::{Span, TypedDictKeyViolation, TypedDictKeyViolationKind};

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;
use super::typeddict::TdFieldMap;

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
        let Some((all_fields, field_types, is_total, has_extra_items)) =
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
        let invalid_keys: Vec<String> = if *has_extra_items {
            Vec::new()
        } else {
            literal_keys
                .iter()
                .filter(|k| !all_fields.contains(&k.as_str()))
                .cloned()
                .collect()
        };
        let missing_keys: Vec<String> = if *is_total {
            all_fields
                .iter()
                .filter(|&&f| !literal_keys.iter().any(|k| k == f))
                .map(|s| (*s).to_owned())
                .collect()
        } else {
            Vec::new()
        };

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

        // Check value types for each key-value pair
        for item in &dict.items {
            let Some(key_expr) = &item.key else { continue };
            let Expr::StringLiteral(s) = key_expr else {
                continue;
            };
            let key = s.value.to_string();
            if !all_fields.contains(&key.as_str()) {
                continue; // Already flagged as invalid key
            }
            if let Some(expected) = field_types.get(key.as_str()) {
                if let Some(actual) = expr_literal_type_name(&item.value) {
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
    let Some((all_fields, _, is_total, has_extra_items)) = fields.get(class_name) else {
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
    let invalid_keys: Vec<String> = if *has_extra_items {
        Vec::new()
    } else {
        literal_keys
            .iter()
            .filter(|k| !all_fields.contains(&k.as_str()))
            .cloned()
            .collect()
    };
    let missing_keys: Vec<String> = if *is_total {
        all_fields
            .iter()
            .filter(|&&f| !literal_keys.iter().any(|k| k == f))
            .map(|s| (*s).to_owned())
            .collect()
    } else {
        Vec::new()
    };

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
    let Some((_, field_types, _, _)) = fields.get(class_name) else {
        return;
    };
    for item in &dict.items {
        let Some(Expr::StringLiteral(s)) = &item.key else {
            continue;
        };
        let key = s.value.to_string();
        if !all_fields.contains(&key.as_str()) {
            continue;
        }
        if let Some(expected) = field_types.get(key.as_str()) {
            if let Some(actual) = expr_literal_type_name(&item.value) {
                if !typeddict_field_type_compatible(actual, expected) {
                    out.push(TypedDictKeyViolation {
                        span: text_range_to_span(node.range()),
                        class_name: class_name.to_owned(),
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
                    if let Some((all_fields, _, _, _)) = fields.get(class_name.as_str()) {
                        if let Expr::StringLiteral(key_str) = sub.slice.as_ref() {
                            let key = key_str.value.to_string();
                            if !all_fields.contains(&key.as_str()) {
                                out.push(TypedDictKeyViolation {
                                    span: text_range_to_span(sub.range()),
                                    class_name: class_name.clone(),
                                    kind: TypedDictKeyViolationKind::SubscriptReadInvalidKey {
                                        key,
                                    },
                                });
                            }
                        } else {
                            // Non-literal key access on a TypedDict
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
    actual == expected
        || (actual == "bool" && expected == "int")
        || (actual == "int" && expected == "float")
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
