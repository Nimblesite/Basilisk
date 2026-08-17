//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Typeddict Ext visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAnnAssign, StmtAssign};

use crate::scope::{ClassGraph, Span, TypedDictKeyViolation, TypedDictKeyViolationKind};

use super::class_info_ext::expr_simple_name;
use super::core::text_range_to_span;
use super::typeddict::{annotation_local_class, TdFieldMap};

/// Determine whether a `TypedDict` field is required.
///
/// - `is_total=true` (default):  all fields are required.
/// - `is_total=false`:  all fields are not-required.
pub(super) fn is_field_required(
    _field_name: &str,
    _field_types: &std::collections::HashMap<&str, String>,
    is_total: bool,
) -> bool {
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
/// Emits diagnostics for non-literal keys, invalid keys, and missing required
/// keys. `span_range` is the range used for emitted spans; callers typically
/// pass the enclosing statement's range so the diagnostic points at the whole
/// assignment.
fn check_dict_against_typeddict(
    dict: &ruff_python_ast::ExprDict,
    spec: &TdSpec<'_>,
    span_range: ruff_text_size::TextRange,
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
}

pub(super) fn td_check_regular_assign(
    node: &StmtAssign,
    var_type: &std::collections::HashMap<String, Span>,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    for target in &node.targets {
        let Some(var_name) = expr_simple_name(target) else {
            continue;
        };
        let Some(site) = var_type.get(&var_name) else {
            continue;
        };
        let Some(schema) = fields.get(site) else {
            continue;
        };
        let Expr::Dict(dict) = node.value.as_ref() else {
            continue;
        };
        check_dict_against_typeddict(
            dict,
            &TdSpec {
                class_name: schema.class_name,
                all_fields: &schema.all_fields,
                field_types: &schema.field_types,
                is_total: schema.is_total,
                has_extra_items: schema.has_extra_items,
            },
            node.range(),
            out,
        );
    }
}

/// Validate `var: Movie = {...}` against the schema of the class the
/// ANNOTATION denotes.
///
/// The predecessor of this function was deleted for finding the schema by
/// matching the annotation's characters against a class-name key, so an
/// aliased or dotted annotation was never validated and an ordinary class
/// sharing a `TypedDict`'s name had that schema's errors reported against it.
/// Here the annotation resolves through the module's bindings —
/// [`annotation_local_class`] follows assignment aliases and quoted forward
/// references — and the schema lookup is the definition-site key
/// ([RESOLV-CANONICAL-BINDING]). An annotation that resolves to no local
/// `TypedDict` is abstention: nothing is validated and nothing is invented.
pub(super) fn td_check_ann_assign(
    bindings: &basilisk_canonical::BindingTable,
    node: &StmtAnnAssign,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    let Some(value) = node.value.as_deref() else {
        return;
    };
    let Expr::Dict(dict) = value else {
        return;
    };
    let Some(site) = annotation_local_class(bindings, &node.annotation) else {
        return;
    };
    let Some(schema) = fields.get(&text_range_to_span(site)) else {
        return;
    };
    check_dict_against_typeddict(
        dict,
        &TdSpec {
            class_name: schema.class_name,
            all_fields: &schema.all_fields,
            field_types: &schema.field_types,
            is_total: schema.is_total,
            has_extra_items: schema.has_extra_items,
        },
        node.range(),
        out,
    );
}

/// Walk an expression and report subscript reads with invalid `TypedDict` keys.
pub(super) fn td_check_expr_reads(
    expr: &Expr,
    var_type: &std::collections::HashMap<String, Span>,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    match expr {
        Expr::Subscript(sub) => {
            if let Some(var_name) = expr_simple_name(&sub.value) {
                if let Some(schema) = var_type.get(&var_name).and_then(|site| fields.get(site)) {
                    // A string literal is a statically-known key; anything else is not.
                    if let Some(key) = subscript_key_literal(&sub.slice) {
                        if !schema.all_fields.contains(&key.as_str()) {
                            out.push(TypedDictKeyViolation {
                                span: text_range_to_span(sub.range()),
                                class_name: schema.class_name.to_owned(),
                                kind: TypedDictKeyViolationKind::SubscriptReadInvalidKey { key },
                            });
                        }
                    } else {
                        // Non-literal key access on a TypedDict
                        out.push(TypedDictKeyViolation {
                            span: text_range_to_span(sub.range()),
                            class_name: schema.class_name.to_owned(),
                            kind: TypedDictKeyViolationKind::NonLiteralDictKey,
                        });
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
        Expr::UnaryOp(unary) => {
            td_check_expr_reads(&unary.operand, var_type, fields, out);
        }
        _ => {}
    }
}

/// Resolve a subscript key expression to its statically-known string value.
///
/// Returns the value for a string literal; anything else yields `None`.
fn subscript_key_literal(slice: &Expr) -> Option<String> {
    match slice {
        Expr::StringLiteral(key_str) => Some(key_str.value.to_string()),
        _ => None,
    }
}

/// Collect `isinstance`/`issubclass`-on-`TypedDict` violations from
/// module-level statements and function bodies.
///
/// PEP 589: "TypedDict type objects cannot be used in `isinstance()` tests
/// such as `isinstance(d, Movie)`." Both sides of the call resolve through
/// the module's [`basilisk_canonical::BindingTable`] ([ASTREBUILD-LAW]): the
/// callee must resolve to the builtin `isinstance`/`issubclass`, and the
/// checked expression must resolve to the DEFINITION of a class this module
/// defines, which the [`ClassGraph`] then judges.
///
/// The second half was REBUILT. It used to test the argument's `Expr::Name.id`
/// against a `HashSet<&str>` of names of classes DIRECTLY declared with a
/// `TypedDict` base, which got the question wrong three ways at once: a
/// TypedDict reached under a second name was not in the set, a subclass of a
/// TypedDict was not in the set even though it is just as un-checkable, and a
/// name since rebound to an ordinary class matched the set and was reported
/// anyway.
pub(super) fn collect_isinstance_typeddict_violations(
    bindings: &basilisk_canonical::BindingTable,
    stmts: &[Stmt],
    graph: &ClassGraph<'_>,
) -> Vec<Span> {
    let mut out = Vec::new();
    collect_isinstance_typeddict_in_stmts(bindings, stmts, graph, &mut out);
    out
}

pub(super) fn collect_isinstance_typeddict_in_stmts(
    bindings: &basilisk_canonical::BindingTable,
    stmts: &[Stmt],
    graph: &ClassGraph<'_>,
    out: &mut Vec<Span>,
) {
    let mut check = |expr: &Expr| collect_isinstance_typeddict_in_expr(bindings, expr, graph, out);
    crate::walk_all_stmts(stmts, &mut |stmt| match stmt {
        Stmt::If(node) => {
            check(&node.test);
            for clause in &node.elif_else_clauses {
                if let Some(test) = &clause.test {
                    check(test);
                }
            }
        }
        Stmt::Expr(node) => check(&node.value),
        Stmt::Assign(node) => check(&node.value),
        Stmt::AnnAssign(node) => {
            if let Some(val) = &node.value {
                check(val);
            }
        }
        Stmt::While(node) => check(&node.test),
        _ => {}
    });
}

pub(super) fn collect_isinstance_typeddict_in_expr(
    bindings: &basilisk_canonical::BindingTable,
    expr: &Expr,
    graph: &ClassGraph<'_>,
    out: &mut Vec<Span>,
) {
    use basilisk_canonical::TypingForm;
    use ruff_text_size::Ranged as _;
    let Expr::Call(call) = expr else { return };
    let callee_is_runtime_check = matches!(
        bindings.form_of_with_builtins(&call.func),
        Some(TypingForm::IsinstanceFunction | TypingForm::IssubclassFunction)
    );
    if !callee_is_runtime_check {
        return;
    }
    let Some(second_arg) = call.arguments.args.get(1) else {
        return;
    };
    // Positional resolution: which class the argument denotes is decided by
    // the binding in force AT THE CALL, so a name later rebound to something
    // else does not retroactively make this call illegal, and a name rebound
    // to an ordinary class before it is not illegal at all.
    let Some(site) = bindings.local_class_definition(second_arg) else {
        return;
    };
    let Some(class) = graph.at(text_range_to_span(site)) else {
        return;
    };
    if !graph.is_typed_dict(class) {
        return;
    }
    let range = call.range();
    out.push(Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    });
}
