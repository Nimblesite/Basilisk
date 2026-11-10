//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Assigns visitor functions.

use ruff_python_ast::{ExceptHandler, Expr, Stmt, StmtAssign, StmtImport, StmtImportFrom};
use ruff_text_size::Ranged;

use crate::scope::VariableInfo;

use super::class_info_ext::{alias_name, expr_simple_name};
use super::core::{classify_rhs, text_range_to_span};
use super::walrus::{collect_walrus_targets, Reach};

/// Names a plain `import` statement binds into the enclosing scope.
/// `import a.b.c` binds the top-level package `a`; `import a.b as d` binds `d`.
fn plain_import_bound_names(node: &StmtImport) -> Vec<String> {
    node.names
        .iter()
        .map(|alias| {
            alias.asname.as_ref().map_or_else(
                || top_level_module(alias.name.as_str()).to_owned(),
                ToString::to_string,
            )
        })
        .collect()
}

/// Names a `from ... import` statement binds into the enclosing scope.
/// `from m import X, Y as z` binds `X` and `z`; the `as` alias takes priority.
fn from_import_bound_names(node: &StmtImportFrom) -> Vec<String> {
    node.names.iter().map(alias_name).collect()
}

/// The top-level package component of a dotted module path (`a` in `a.b.c`).
fn top_level_module(dotted: &str) -> &str {
    dotted.split('.').next().unwrap_or(dotted)
}

pub(super) fn extract_target_names(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Name(name) => vec![name.id.to_string()],
        Expr::Tuple(tuple) => tuple.elts.iter().flat_map(extract_target_names).collect(),
        Expr::List(list) => list.elts.iter().flat_map(extract_target_names).collect(),
        // `a, *rest = ...` — the starred element binds `rest` like any other target.
        Expr::Starred(starred) => extract_target_names(&starred.value),
        _ => Vec::new(),
    }
}

/// Names bound by a `match` case pattern: `case [x, *rest]`, `case {**extra}`,
/// `case Point(x=px) as pt` bind `x`, `rest`, `extra`, `px`, and `pt`.
fn extract_pattern_names(pattern: &ruff_python_ast::Pattern) -> Vec<String> {
    use ruff_python_ast::Pattern;
    match pattern {
        Pattern::MatchValue(_) | Pattern::MatchSingleton(_) => Vec::new(),
        Pattern::MatchSequence(seq) => seq
            .patterns
            .iter()
            .flat_map(extract_pattern_names)
            .collect(),
        Pattern::MatchMapping(map) => map
            .patterns
            .iter()
            .flat_map(extract_pattern_names)
            .chain(map.rest.iter().map(ToString::to_string))
            .collect(),
        Pattern::MatchClass(class) => class
            .arguments
            .patterns
            .iter()
            .chain(class.arguments.keywords.iter().map(|kw| &kw.pattern))
            .flat_map(extract_pattern_names)
            .collect(),
        Pattern::MatchStar(star) => star.name.iter().map(ToString::to_string).collect(),
        Pattern::MatchAs(as_pattern) => as_pattern
            .pattern
            .iter()
            .flat_map(|inner| extract_pattern_names(inner))
            .chain(as_pattern.name.iter().map(ToString::to_string))
            .collect(),
        Pattern::MatchOr(or_pattern) => or_pattern
            .patterns
            .iter()
            .flat_map(extract_pattern_names)
            .collect(),
    }
}

/// Collect all names assigned anywhere in the function body (not in nested functions).
pub(super) fn collect_all_assigns(stmts: &[Stmt]) -> Vec<String> {
    let mut out = collect_walrus_targets(stmts, Reach::Any);
    out.extend(collect_statement_assigns(stmts));
    out
}

/// The statement-shaped half of [`collect_all_assigns`].
fn collect_statement_assigns(stmts: &[Stmt]) -> Vec<String> {
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
                out.extend(collect_statement_assigns(&node.body));
                out.extend(collect_statement_assigns(&node.orelse));
            }
            Stmt::FunctionDef(func) => {
                // Nested function name is defined in enclosing scope.
                out.push(func.name.to_string());
                // Do NOT recurse into nested function body.
            }
            Stmt::ClassDef(class) => {
                // A nested class binds its name in the enclosing scope, exactly
                // like a nested function. Do NOT recurse into the class body.
                out.push(class.name.to_string());
            }
            Stmt::TypeAlias(node) => {
                // A PEP 695 `type` statement binds its alias name in the
                // enclosing scope, exactly like a `def`.
                if let Some(name) = expr_simple_name(&node.name) {
                    out.push(name);
                }
            }
            Stmt::Import(node) => {
                // A function-local import binds names in the enclosing scope and
                // is reachable by nested scopes (incl. methods of nested classes).
                out.extend(plain_import_bound_names(node));
            }
            Stmt::ImportFrom(node) => {
                out.extend(from_import_bound_names(node));
            }
            Stmt::If(node) => {
                out.extend(collect_statement_assigns(&node.body));
                for clause in &node.elif_else_clauses {
                    out.extend(collect_statement_assigns(&clause.body));
                }
            }
            Stmt::While(node) => {
                out.extend(collect_statement_assigns(&node.body));
                out.extend(collect_statement_assigns(&node.orelse));
            }
            Stmt::With(node) => {
                for item in &node.items {
                    if let Some(var) = item.optional_vars.as_deref() {
                        out.extend(extract_target_names(var));
                    }
                }
                out.extend(collect_statement_assigns(&node.body));
            }
            Stmt::Try(node) => {
                out.extend(collect_statement_assigns(&node.body));
                for handler in &node.handlers {
                    let ExceptHandler::ExceptHandler(h) = handler;
                    if let Some(exc_name) = &h.name {
                        out.push(exc_name.to_string());
                    }
                    out.extend(collect_statement_assigns(&h.body));
                }
                out.extend(collect_statement_assigns(&node.orelse));
                out.extend(collect_statement_assigns(&node.finalbody));
            }
            Stmt::Match(node) => {
                for case in &node.cases {
                    out.extend(extract_pattern_names(&case.pattern));
                    out.extend(collect_statement_assigns(&case.body));
                }
            }
            _ => {}
        }
    }
    out
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
