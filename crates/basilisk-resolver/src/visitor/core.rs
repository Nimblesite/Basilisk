//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Core visitor functions.

use ruff_python_ast::{ElifElseClause, ExceptHandler, Expr, Stmt};
use ruff_text_size::TextRange;

use crate::canonical::BindingTable;
use crate::scope::{
    ClassInfo, FunctionInfo, ImportInfo, MatchStmtInfo, RhsKind, Span, TypedDictKeyViolation,
    TypedDictKeyViolationKind, VariableInfo,
};

use super::annotations::ann_assign_info_from;
use super::assigns::assign_infos_from;
use super::class_info_ext::{
    class_info_from, expr_simple_name, import_from_infos_from, import_infos_from,
    match_stmt_info_from,
};
use super::function_info::function_info_from;
use super::typeddict::{td_check_subscript_assign, td_var_type_from_stmts, TdFieldMap,
};
use super::typeddict_ext::{td_check_ann_assign, td_check_expr_reads, td_check_regular_assign};

pub(super) fn collect_from_body(
    bindings: &BindingTable,
    stmts: &[Stmt],
    functions: &mut Vec<FunctionInfo>,
    classes: &mut Vec<ClassInfo>,
    module_vars: &mut Vec<VariableInfo>,
    imports: &mut Vec<ImportInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    is_module_level: bool,
) {
    for stmt in stmts {
        collect_from_stmt(
            bindings,
            stmt,
            functions,
            classes,
            module_vars,
            imports,
            match_stmts,
            is_module_level,
        );
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "AST visitor covers many statement types"
)]
pub(super) fn collect_from_stmt(
    bindings: &BindingTable,
    stmt: &Stmt,
    functions: &mut Vec<FunctionInfo>,
    classes: &mut Vec<ClassInfo>,
    module_vars: &mut Vec<VariableInfo>,
    imports: &mut Vec<ImportInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    is_module_level: bool,
) {
    match stmt {
        Stmt::FunctionDef(func) => {
            functions.push(function_info_from(func, None));
            collect_from_body(
                bindings,
                &func.body,
                functions,
                &mut Vec::new(),
                &mut Vec::new(),
                &mut Vec::new(),
                match_stmts,
                false,
            );
        }
        Stmt::ClassDef(class) => {
            let class_info = class_info_from(bindings, class, functions, match_stmts);
            classes.push(class_info);
        }
        Stmt::If(node) => {
            collect_from_body(
                bindings,
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
            collect_from_elif_else(
                bindings,
                &node.elif_else_clauses,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
        }
        Stmt::For(node) => {
            collect_from_body(
                bindings,
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
            collect_from_body(
                bindings,
                &node.orelse,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
        }
        Stmt::While(node) => {
            collect_from_body(
                bindings,
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
            collect_from_body(
                bindings,
                &node.orelse,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
        }
        Stmt::With(node) => {
            collect_from_body(
                bindings,
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
        }
        Stmt::Try(node) => {
            collect_from_body(
                bindings,
                &node.body,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
            collect_from_handlers(
                bindings,
                &node.handlers,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
            collect_from_body(
                bindings,
                &node.orelse,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
            collect_from_body(
                bindings,
                &node.finalbody,
                functions,
                classes,
                module_vars,
                imports,
                match_stmts,
                is_module_level,
            );
        }
        Stmt::Import(node) if is_module_level => {
            imports.extend(import_infos_from(node));
        }
        Stmt::ImportFrom(node) if is_module_level => {
            imports.extend(import_from_infos_from(node));
        }
        Stmt::Assign(node) if is_module_level => {
            module_vars.extend(assign_infos_from(node));
        }
        Stmt::AnnAssign(node) if is_module_level => {
            if let Some(var) = ann_assign_info_from(node) {
                module_vars.push(var);
            }
        }
        Stmt::Match(node) => {
            match_stmts.push(match_stmt_info_from(node));
            for case in &node.cases {
                collect_from_body(
                    bindings,
                    &case.body,
                    functions,
                    classes,
                    module_vars,
                    imports,
                    match_stmts,
                    is_module_level,
                );
            }
        }
        _ => {}
    }
}

pub(super) fn collect_from_elif_else(
    bindings: &BindingTable,
    clauses: &[ElifElseClause],
    functions: &mut Vec<FunctionInfo>,
    class_defs: &mut Vec<ClassInfo>,
    module_vars: &mut Vec<VariableInfo>,
    imports: &mut Vec<ImportInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    is_module_level: bool,
) {
    for clause in clauses {
        collect_from_body(
            bindings,
            &clause.body,
            functions,
            class_defs,
            module_vars,
            imports,
            match_stmts,
            is_module_level,
        );
    }
}

pub(super) fn collect_from_handlers(
    bindings: &BindingTable,
    handlers: &[ExceptHandler],
    functions: &mut Vec<FunctionInfo>,
    classes: &mut Vec<ClassInfo>,
    module_vars: &mut Vec<VariableInfo>,
    imports: &mut Vec<ImportInfo>,
    match_stmts: &mut Vec<MatchStmtInfo>,
    is_module_level: bool,
) {
    for handler in handlers {
        let ExceptHandler::ExceptHandler(h) = handler;
        collect_from_body(
            bindings,
            &h.body,
            functions,
            classes,
            module_vars,
            imports,
            match_stmts,
            is_module_level,
        );
    }
}

// ---------------------------------------------------------------------------
// Class info
// ---------------------------------------------------------------------------

pub(super) fn classify_rhs(expr: &Expr) -> RhsKind {
    match expr {
        Expr::BooleanLiteral(_) => RhsKind::BoolLiteral,
        Expr::NumberLiteral(n) => match n.value {
            ruff_python_ast::Number::Float(_) => RhsKind::FloatLiteral,
            ruff_python_ast::Number::Complex { .. } => RhsKind::Other,
            ruff_python_ast::Number::Int(_) => RhsKind::IntLiteral,
        },
        Expr::StringLiteral(_) | Expr::FString(_) => RhsKind::StrLiteral,
        Expr::BytesLiteral(_) => RhsKind::BytesLiteral,
        Expr::NoneLiteral(_) => RhsKind::NoneValue,
        Expr::List(list) if !list.elts.is_empty() => {
            RhsKind::List(list.elts.iter().map(classify_rhs).collect())
        }
        Expr::Dict(dict) if !dict.items.is_empty() => {
            let pairs = dict
                .items
                .iter()
                .filter_map(|item| {
                    let key = item.key.as_ref().map(classify_rhs)?;
                    let val = classify_rhs(&item.value);
                    Some((key, val))
                })
                .collect();
            RhsKind::Dict(pairs)
        }
        Expr::Set(set) => RhsKind::Set(set.elts.iter().map(classify_rhs).collect()),
        Expr::Tuple(tup) => RhsKind::Tuple(tup.elts.iter().map(classify_rhs).collect()),
        Expr::List(list) if list.elts.is_empty() => RhsKind::EmptyList,
        Expr::Dict(dict) if dict.items.is_empty() => RhsKind::EmptyDict,
        Expr::Call(call) => classify_call(call),
        Expr::Lambda(_) => RhsKind::Lambda,
        _ => RhsKind::Other,
    }
}

/// Classify a call expression. Most calls have unknown result kinds
/// ([`RhsKind::CallExpr`]), but `<dict-literal>.get(key, default)` resolves
/// to the common kind of the dict's values and the default when they all
/// agree (#253 — feeds inferred-type display in hover and inlay hints).
fn classify_call(call: &ruff_python_ast::ExprCall) -> RhsKind {
    dict_get_result_kind(call).map_or(RhsKind::CallExpr, |kind| RhsKind::KnownCall(Box::new(kind)))
}

/// Result kind of a two-argument `.get(key, default)` on a dict literal, when
/// every value kind and the default's kind agree. The one-argument form is
/// excluded: it returns `value | None`, which has no single kind.
fn dict_get_result_kind(call: &ruff_python_ast::ExprCall) -> Option<RhsKind> {
    let Expr::Attribute(attr) = call.func.as_ref() else {
        return None;
    };
    if attr.attr.as_str() != "get"
        || call.arguments.args.len() != 2
        || !call.arguments.keywords.is_empty()
    {
        return None;
    }
    let RhsKind::Dict(pairs) = classify_rhs(&attr.value) else {
        return None;
    };
    let default_kind = classify_rhs(call.arguments.args.get(1)?);
    let mut value_kinds = pairs
        .into_iter()
        .map(|(_, value)| value)
        .chain(std::iter::once(default_kind));
    let first = value_kinds.next()?;
    value_kinds.all(|kind| kind == first).then_some(first)
}

// ---------------------------------------------------------------------------
// Match statement info
// ---------------------------------------------------------------------------

pub(super) fn text_range_to_span(range: TextRange) -> Span {
    Span::from(range)
}

/// Slice `source` using a [`TextRange`] without `as` conversions.
///
/// Returns `None` if the range is out of bounds.
#[must_use]
pub(super) fn source_slice_range(source: &str, range: TextRange) -> Option<&str> {
    let start = usize::try_from(range.start().to_u32()).ok()?;
    let end = usize::try_from(range.end().to_u32()).ok()?;
    source.get(start..end)
}

/// Slice `source` using a [`Span`] without `as` conversions.
///
/// Returns `None` if the span is out of bounds.
#[must_use]
pub(super) fn source_slice_span(source: &str, span: Span) -> Option<&str> {
    let start = usize::try_from(span.start).ok()?;
    let end = usize::try_from(span.end).ok()?;
    source.get(start..end)
}

pub(super) fn check_td_stmts(
    fields: &TdFieldMap<'_>,
    var_type: &std::collections::HashMap<String, String>,
    stmts: &[Stmt],
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                td_check_subscript_assign(node, var_type, fields, out);
                td_check_regular_assign(node, var_type, fields, out);
                td_check_expr_reads(&node.value, var_type, fields, out);
            }
            Stmt::AnnAssign(node) => {
                td_check_ann_assign(node, fields, out);
                if let Some(val) = &node.value {
                    td_check_expr_reads(val, var_type, fields, out);
                }
            }
            Stmt::Expr(expr_stmt) => {
                // Detect disallowed method calls: movie.clear()
                if let Expr::Call(call) = expr_stmt.value.as_ref() {
                    if let Expr::Attribute(attr) = call.func.as_ref() {
                        if let Some(var_name) = expr_simple_name(&attr.value) {
                            if let Some(class_name) = var_type.get(&var_name) {
                                // A TypedDict with `extra_items` (PEP 728) behaves
                                // like `dict[str, VT]`, so the mutating dict methods
                                // become available and must not be flagged.
                                if let Some((_, _, _, has_extra_items)) =
                                    fields.get(class_name.as_str())
                                {
                                    const DISALLOWED: &[&str] =
                                        &["clear", "pop", "popitem", "setdefault", "update"];
                                    let method = attr.attr.as_str();
                                    if !has_extra_items && DISALLOWED.contains(&method) {
                                        out.push(TypedDictKeyViolation {
                                            span: text_range_to_span(expr_stmt.value.range()),
                                            class_name: class_name.clone(),
                                            kind: TypedDictKeyViolationKind::DisallowedMethodCall {
                                                method: method.to_owned(),
                                            },
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                td_check_expr_reads(&expr_stmt.value, var_type, fields, out);
            }
            Stmt::Delete(del) => {
                // `del td[k]` is only an error when `k` is a literal naming a
                // declared *required* field. Deleting a NotRequired field or an
                // `extra_items` key (PEP 728) is allowed, and a non-literal key
                // cannot be resolved to a field so is left alone.
                for target in &del.targets {
                    let Expr::Subscript(sub) = target else {
                        continue;
                    };
                    let Some(var_name) = expr_simple_name(&sub.value) else {
                        continue;
                    };
                    let Some(class_name) = var_type.get(&var_name) else {
                        continue;
                    };
                    let Some((all_fields, field_types, is_total, _)) =
                        fields.get(class_name.as_str())
                    else {
                        continue;
                    };
                    let Expr::StringLiteral(key_str) = sub.slice.as_ref() else {
                        continue;
                    };
                    let key = key_str.value.to_str();
                    if all_fields.contains(&key)
                        && super::typeddict_ext::is_field_required(key, field_types, *is_total)
                    {
                        out.push(TypedDictKeyViolation {
                            span: text_range_to_span(del.range()),
                            class_name: class_name.clone(),
                            kind: TypedDictKeyViolationKind::DeleteSubscript,
                        });
                    }
                }
            }
            Stmt::FunctionDef(func) => {
                // Recurse with a local var_type that includes function-level annotated vars
                // and function parameter types.
                let mut local_vars = var_type.clone();
                local_vars.extend(td_var_type_from_stmts(&func.body, fields));
                // Add parameter types that are TypedDict classes.
                for param in super::walks::iter_all_params(&func.parameters) {
                    if let Some(ann) = &param.parameter.annotation {
                        if let Some(type_name) = expr_simple_name(ann) {
                            if fields.contains_key(type_name.as_str()) {
                                let _ =
                                    local_vars.insert(param.parameter.name.to_string(), type_name);
                            }
                        }
                    }
                }
                check_td_stmts(fields, &local_vars, &func.body, out);
            }
            _ => {}
        }
    }
}
