//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
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
pub(super) fn is_field_required(
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

/// Resolved `TypedDict` class metadata used to validate a dict literal.
struct TdSpec<'a> {
    class_name: &'a str,
    all_fields: &'a [&'a str],
    field_types: &'a std::collections::HashMap<&'a str, String>,
    is_total: bool,
    has_extra_items: bool,
}

/// Validate a dict literal against the `TypedDict` spec.
///
/// Emits diagnostics for non-literal keys, invalid keys, missing required keys,
/// and value-type mismatches. `span_range` is the range used for emitted spans;
/// callers typically pass the enclosing statement's range so the diagnostic
/// points at the whole assignment.
fn check_dict_against_typeddict(
    dict: &ruff_python_ast::ExprDict,
    spec: &TdSpec<'_>,
    span_range: ruff_text_size::TextRange,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    let TdSpec {
        class_name,
        all_fields,
        field_types,
        is_total,
        has_extra_items,
    } = *spec;
    // Flag any non-literal (variable) key in the dict — if found, return early
    // (the literal-key checks below assume every key is a string literal).
    let has_non_literal = dict.items.iter().any(|item| {
        item.key
            .as_ref()
            .is_some_and(|k| !matches!(k, Expr::StringLiteral(_)))
    });
    if has_non_literal {
        out.push(TypedDictKeyViolation {
            span: text_range_to_span(span_range),
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
    let invalid_keys: Vec<String> = if has_extra_items {
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
        .filter(|&&f| is_field_required(f, field_types, is_total))
        .map(|s| (*s).to_owned())
        .collect();

    if !invalid_keys.is_empty() || !missing_keys.is_empty() {
        out.push(TypedDictKeyViolation {
            span: text_range_to_span(span_range),
            class_name: class_name.to_owned(),
            kind: TypedDictKeyViolationKind::InvalidDictLiteral {
                invalid_keys,
                missing_keys,
            },
        });
    }

    check_dict_value_types(
        dict,
        field_types,
        all_fields,
        class_name,
        span_range,
        fields,
        out,
    );
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
        let Some((all_fields, field_types, is_total, has_extra_items)) =
            fields.get(class_name.as_str())
        else {
            continue;
        };
        let Expr::Dict(dict) = node.value.as_ref() else {
            continue;
        };
        check_dict_against_typeddict(
            dict,
            &TdSpec {
                class_name,
                all_fields,
                field_types,
                is_total: *is_total,
                has_extra_items: *has_extra_items,
            },
            node.range(),
            fields,
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
    let Some((all_fields, field_types, is_total, has_extra_items)) = fields.get(class_name) else {
        return;
    };
    let Expr::Dict(dict) = value.as_ref() else {
        return;
    };
    check_dict_against_typeddict(
        dict,
        &TdSpec {
            class_name,
            all_fields,
            field_types,
            is_total: *is_total,
            has_extra_items: *has_extra_items,
        },
        node.range(),
        fields,
        out,
    );
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
    let stripped = crate::scope::strip_typeddict_qualifiers(expected);
    // A union field accepts a value matching any member (`year: int | None`).
    if stripped.contains('|') {
        return stripped
            .split('|')
            .any(|member| typeddict_field_type_compatible(actual, member.trim()));
    }
    actual == stripped
        || (actual == "bool" && stripped == "int")
        || (actual == "int" && stripped == "float")
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
    crate::walk_all_stmts(stmts, &mut |stmt| match stmt {
        Stmt::If(node) => {
            collect_isinstance_typeddict_in_expr(&node.test, typeddict_names, out);
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    collect_isinstance_typeddict_in_expr(test, typeddict_names, out);
                }
            }
        }
        Stmt::Expr(node) => collect_isinstance_typeddict_in_expr(&node.value, typeddict_names, out),
        Stmt::Assign(node) => {
            collect_isinstance_typeddict_in_expr(&node.value, typeddict_names, out);
        }
        Stmt::AnnAssign(node) => {
            if let Some(val) = &node.value {
                collect_isinstance_typeddict_in_expr(val, typeddict_names, out);
            }
        }
        Stmt::While(node) => collect_isinstance_typeddict_in_expr(&node.test, typeddict_names, out),
        _ => {}
    });
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
