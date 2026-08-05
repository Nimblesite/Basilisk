//! Implements [LINESCANPLAN-AST-MIGRATION]. See
//! docs/plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-AST-MIGRATION
//!
//! Structural accessors over annotation expression nodes.
//!
//! Everything here reads the shape of a parsed `ruff` node. Nothing here
//! decides what a name *means*: the name-taking recognition surface that once
//! lived in this module is deleted under the symbol-naming ban, and no
//! replacement may be introduced here.

use ruff_python_ast::Expr;

/// The dotted spelling of a name/attribute chain, or `None` when the
/// expression is not a dotted name.
pub(crate) fn dotted_spelling(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => Some(format!("{}.{}", dotted_spelling(&attr.value)?, attr.attr)),
        _ => None,
    }
}

/// The comma-separated arguments of a subscript slice: `X[a, b]` yields
/// `[a, b]`, `X[a]` yields `[a]`.
pub(crate) fn subscript_args(slice: &Expr) -> Vec<&Expr> {
    match slice {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        single => vec![single],
    }
}
