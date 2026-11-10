//! Implements [TYPEINF-TARGET-NARROWING]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-NARROWING
//! Rebinding collection for narrow-invalidation: names re-bound by tuple/
//! starred assignment targets, loop targets, `with ... as`, augmented
//! assignment, or anywhere inside a loop/`try` body must lose their stale
//! flow narrows ([TYPEINF-NARROWING-ASSIGN] — a rebound name never keeps a
//! narrow proven for its previous value).

use std::collections::HashSet;

use ruff_python_ast::{ExceptHandler, Expr, Stmt};

/// Collect every NAME bound by an assignment-target expression (attribute
/// and subscript targets do not bind a name).
pub(crate) fn target_names(target: &Expr, out: &mut Vec<String>) {
    match target {
        Expr::Name(name) => out.push(name.id.to_string()),
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                target_names(elt, out);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                target_names(elt, out);
            }
        }
        Expr::Starred(starred) => target_names(&starred.value, out),
        _ => {}
    }
}

/// Collect every name BOUND anywhere in a statement list — assignment and
/// loop targets, `with ... as` names, `except ... as` names — stopping at
/// nested function/class boundaries (their bindings are their own scope's).
///
/// Conservative direction: reporting MORE names than strictly rebound only
/// resets narrows early (sound); missing one would leave a stale narrow.
pub(crate) fn bound_names(stmts: &[Stmt], out: &mut HashSet<String>) {
    for stmt in stmts {
        bound_names_in_stmt(stmt, out);
    }
}

/// One statement's contribution to [`bound_names`].
fn bound_names_in_stmt(stmt: &Stmt, out: &mut HashSet<String>) {
    let mut names = Vec::new();
    match stmt {
        Stmt::Assign(node) => {
            for target in &node.targets {
                target_names(target, &mut names);
            }
        }
        Stmt::AnnAssign(node) => target_names(&node.target, &mut names),
        Stmt::AugAssign(node) => target_names(&node.target, &mut names),
        Stmt::For(node) => {
            target_names(&node.target, &mut names);
            bound_names(&node.body, out);
            bound_names(&node.orelse, out);
        }
        Stmt::While(node) => {
            bound_names(&node.body, out);
            bound_names(&node.orelse, out);
        }
        Stmt::If(node) => {
            bound_names(&node.body, out);
            for clause in &node.elif_else_clauses {
                bound_names(&clause.body, out);
            }
        }
        Stmt::With(node) => {
            for item in &node.items {
                if let Some(vars) = item.optional_vars.as_deref() {
                    target_names(vars, &mut names);
                }
            }
            bound_names(&node.body, out);
        }
        Stmt::Try(node) => {
            bound_names(&node.body, out);
            for handler in &node.handlers {
                let ExceptHandler::ExceptHandler(h) = handler;
                if let Some(name) = &h.name {
                    names.push(name.to_string());
                }
                bound_names(&h.body, out);
            }
            bound_names(&node.orelse, out);
            bound_names(&node.finalbody, out);
        }
        Stmt::Match(node) => {
            for case in &node.cases {
                bound_names(&case.body, out);
            }
        }
        // Nested functions/classes bind only their own NAME in this scope.
        Stmt::FunctionDef(node) => names.push(node.name.to_string()),
        Stmt::ClassDef(node) => names.push(node.name.to_string()),
        _ => {}
    }
    out.extend(names);
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::expect_used,
        reason = "test-only parsing of fixed, known-valid fixtures"
    )]

    use super::*;

    fn bound_in(source: &str) -> HashSet<String> {
        let parsed = ruff_python_parser::parse_module(source).expect("fixture parses");
        let mut out = HashSet::new();
        bound_names(&parsed.syntax().body, &mut out);
        out
    }

    /// Tuple/starred targets, loop targets, `with as`, and `except as` all
    /// count as bindings; nested function BODIES do not.
    #[test]
    fn collects_every_binding_form_and_respects_boundaries() {
        let names = bound_in(
            "a, (b, *c) = v\nfor i, j in xs:\n    k = 1\nwith open(p) as fh:\n    pass\ntry:\n    pass\nexcept E as err:\n    pass\ndef inner():\n    hidden = 1\n",
        );
        for expected in ["a", "b", "c", "i", "j", "k", "fh", "err", "inner"] {
            assert!(names.contains(expected), "{expected} missing: {names:?}");
        }
        assert!(
            !names.contains("hidden"),
            "nested function bodies are a boundary: {names:?}"
        );
    }

    /// Attribute/subscript targets bind no name.
    #[test]
    fn attribute_and_subscript_targets_bind_nothing() {
        assert!(bound_in("obj.attr = 1\nxs[0] = 2\n").is_empty());
    }
}
