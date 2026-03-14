//! Assigns visitor functions.

use ruff_python_ast::{ExceptHandler, Expr, Stmt, StmtAssign};
use ruff_text_size::Ranged;

use crate::scope::VariableInfo;

use super::class_info_ext::expr_simple_name;
use super::core::{classify_rhs, text_range_to_span};

pub(super) fn extract_target_names(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Name(name) => vec![name.id.to_string()],
        Expr::Tuple(tuple) => tuple.elts.iter().flat_map(extract_target_names).collect(),
        Expr::List(list) => list.elts.iter().flat_map(extract_target_names).collect(),
        _ => Vec::new(),
    }
}

/// Collect all names assigned anywhere in the function body (not in nested functions).
pub(super) fn collect_all_assigns(stmts: &[Stmt]) -> Vec<String> {
    let mut out = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                out.extend(node.targets.iter().flat_map(extract_target_names));
            }
            Stmt::AnnAssign(node) => {
                if let Some(name) = expr_simple_name(&node.target) {
                    out.push(name);
                }
            }
            Stmt::For(node) => {
                out.extend(extract_target_names(&node.target));
                out.extend(collect_all_assigns(&node.body));
                out.extend(collect_all_assigns(&node.orelse));
            }
            Stmt::FunctionDef(func) => {
                // Nested function name is defined in enclosing scope.
                out.push(func.name.to_string());
                // Do NOT recurse into nested function body.
            }
            Stmt::If(node) => {
                out.extend(collect_all_assigns(&node.body));
                for clause in &node.elif_else_clauses {
                    out.extend(collect_all_assigns(&clause.body));
                }
            }
            Stmt::While(node) => {
                out.extend(collect_all_assigns(&node.body));
                out.extend(collect_all_assigns(&node.orelse));
            }
            Stmt::With(node) => {
                for item in &node.items {
                    if let Some(var) = item.optional_vars.as_deref() {
                        out.extend(extract_target_names(var));
                    }
                }
                out.extend(collect_all_assigns(&node.body));
            }
            Stmt::Try(node) => {
                out.extend(collect_all_assigns(&node.body));
                for handler in &node.handlers {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    if let Some(exc_name) = &h.name {
                        out.push(exc_name.to_string());
                    }
                    out.extend(collect_all_assigns(&h.body));
                }
                out.extend(collect_all_assigns(&node.orelse));
                out.extend(collect_all_assigns(&node.finalbody));
            }
            _ => {}
        }
    }
    out
}

/// Collect names assigned at the top level of a function body (unconditionally).
pub(super) fn collect_unconditional_assigns(stmts: &[Stmt]) -> Vec<String> {
    let mut assignments = Vec::new();

    for stmt in stmts {
        match stmt {
            Stmt::Assign(node) => {
                assignments.extend(node.targets.iter().flat_map(extract_target_names));
            }
            Stmt::AnnAssign(node) => {
                if let Some(name) = expr_simple_name(&node.target) {
                    assignments.push(name);
                }
            }
            Stmt::For(node) => {
                // The for-loop variable(s) are bound whenever the loop body runs.
                assignments.extend(extract_target_names(&node.target));
            }
            Stmt::FunctionDef(func) => {
                assignments.push(func.name.to_string());
            }
            Stmt::If(node) => {
                // Check if this is an if-else statement that guarantees assignment
                if let Some(if_else_assignments) = collect_if_else_assignments(node) {
                    assignments.extend(if_else_assignments);
                }
            }
            _ => {}
        }
    }

    assignments
}

/// Check if an if statement has both if and else branches that assign the same variables.
/// Returns the intersection of assignments from all branches if they cover all paths.
pub(super) fn collect_if_else_assignments(
    if_stmt: &ruff_python_ast::StmtIf,
) -> Option<Vec<String>> {
    let if_branch_assigns = collect_unconditional_assigns(&if_stmt.body);

    // Check if there's an else clause
    let has_else = if_stmt
        .elif_else_clauses
        .iter()
        .any(|clause| clause.test.is_none());
    if !has_else {
        return None;
    }

    // Collect assignments from all elif/else branches
    let mut all_else_assigns = Vec::new();
    for clause in &if_stmt.elif_else_clauses {
        if clause.test.is_none() {
            // This is the else branch
            all_else_assigns.extend(collect_unconditional_assigns(&clause.body));
        } else {
            // This is an elif branch - for now, we'll be conservative and only handle
            // simple if-else without elif chains
            return None;
        }
    }

    // If there are elif branches, we can't guarantee coverage
    let has_elif = if_stmt
        .elif_else_clauses
        .iter()
        .any(|clause| clause.test.is_some());
    if has_elif {
        return None;
    }

    // Find the intersection of assignments from if and else branches
    let intersection: Vec<String> = if_branch_assigns
        .iter()
        .filter(|name| all_else_assigns.contains(name))
        .cloned()
        .collect();

    if intersection.is_empty() {
        None
    } else {
        Some(intersection)
    }
}

// ---------------------------------------------------------------------------
// Return name ref collection
// ---------------------------------------------------------------------------

/// Collect `(name, span)` pairs from `return <name>` stmts in a function body.
/// Does not recurse into nested function definitions.
pub(super) fn assign_infos_from(node: &StmtAssign) -> Vec<VariableInfo> {
    let rhs_kind = classify_rhs(&node.value);
    let rhs_span = Some(text_range_to_span(node.value.range()));
    node.targets
        .iter()
        .filter_map(|target| {
            expr_simple_name(target).map(|name| VariableInfo {
                name,
                name_span: text_range_to_span(target.range()),
                has_annotation: false,
                rhs_kind: rhs_kind.clone(),
                annotation_span: None,
                rhs_span,
            })
        })
        .collect()
}

pub(super) fn collect_module_bare_assignments(
    stmts: &[Stmt],
) -> Vec<crate::scope::ModuleBareAssignment> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        for target in &node.targets {
            if let Some(name) = expr_simple_name(target) {
                out.push(crate::scope::ModuleBareAssignment {
                    name,
                    name_span: text_range_to_span(target.range()),
                });
            }
        }
    }
    out
}

/// Collect module-level attribute assignments (`Class.attr = expr`).
///
/// Used by the checker to detect re-assignments to `Final` class attributes.
pub(super) fn collect_module_attr_assignments(
    stmts: &[Stmt],
) -> Vec<crate::scope::ModuleAttrAssignment> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        for target in &node.targets {
            if let Expr::Attribute(attr) = target {
                if let Some(object_name) = expr_simple_name(&attr.value) {
                    out.push(crate::scope::ModuleAttrAssignment {
                        object_name,
                        attr_name: attr.attr.to_string(),
                        target_span: text_range_to_span(target.range()),
                        rhs_span: Some(text_range_to_span(node.value.range())),
                    });
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Final violation collection stub
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation text refers to `Final`.
pub(super) fn collect_unconditional_self_assigns(
    stmts: &[Stmt],
) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    let Expr::Attribute(attr) = target else {
                        continue;
                    };
                    let Expr::Name(n) = attr.value.as_ref() else {
                        continue;
                    };
                    if n.id == "self" {
                        let _ = names.insert(attr.attr.to_string());
                    }
                }
            }
            // An if/else where both branches assign self.X counts as unconditional.
            Stmt::If(if_stmt) if !if_stmt.elif_else_clauses.is_empty() => {
                let has_else = if_stmt
                    .elif_else_clauses
                    .last()
                    .is_some_and(|clause| clause.test.is_none());
                if has_else {
                    let if_assigns = collect_self_assigns_from_stmts(&if_stmt.body);
                    // Intersect with all elif/else branch assigns.
                    let mut common = if_assigns;
                    for clause in &if_stmt.elif_else_clauses {
                        let branch_assigns = collect_self_assigns_from_stmts(&clause.body);
                        common.retain(|name| branch_assigns.contains(name));
                    }
                    names.extend(common);
                }
            }
            _ => {}
        }
    }
    names
}

/// Collect `self.X` assignment targets from a list of statements (non-recursive,
/// only top-level assigns).
pub(super) fn collect_self_assigns_from_stmts(stmts: &[Stmt]) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    for stmt in stmts {
        let Stmt::Assign(assign) = stmt else { continue };
        for target in &assign.targets {
            let Expr::Attribute(attr) = target else {
                continue;
            };
            let Expr::Name(n) = attr.value.as_ref() else {
                continue;
            };
            if n.id == "self" {
                let _ = names.insert(attr.attr.to_string());
            }
        }
    }
    names
}
