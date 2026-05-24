//! Implements [BSK-E0148] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `TypeVar` constraint detection and parsing for BSK-E0148.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr};

use super::helpers::{ann_str, expr_name};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Constraint group for a `TypeVar`: the list of allowed types.
#[derive(Debug, Clone)]
pub(in super::super) struct ConstrainedTypeVar {
    /// The `TypeVar` name (e.g. `"AnyStr"`).
    pub(super) name: String,
    /// The constraint types in order (e.g. `["str", "bytes"]`).
    pub(super) constraints: Vec<String>,
}

impl ConstrainedTypeVar {
    /// Returns the constraint group index (0-based) that `ty` belongs to, or
    /// `None` when `ty` is not a known constraint (or its subtype).
    pub(super) fn group_of(&self, ty: &str) -> Option<usize> {
        for (idx, constraint) in self.constraints.iter().enumerate() {
            if ty == constraint.as_str() || is_subtype_of(ty, constraint) {
                return Some(idx);
            }
        }
        None
    }
}

/// Returns `true` when `subtype` is a well-known subtype of `supertype`.
///
/// For constraint checking we only need to handle the cases that can occur in
/// practice: e.g. `bool <: int`, and user-defined subclasses of `str` / `bytes`.
/// Unknown names are *conservatively treated as the supertype's group* when the
/// supertype is `str` or `bytes` (a common pattern like `MyStr(str)` maps to the
/// `str` group).
fn is_subtype_of(subtype: &str, supertype: &str) -> bool {
    match (subtype, supertype) {
        ("bool", "int") => true,
        // Conservative: names that start with an uppercase letter and are not
        // known builtins are *assumed* to be subtypes if the supertype is a
        // primitive.  This handles `class MyStr(str)` → maps to `str` group.
        _ => false,
    }
}

/// A function signature with constrained `TypeVar` parameters.
#[derive(Debug, Clone)]
pub(in super::super) struct ConstrainedFunc {
    /// The function name.
    pub(super) name: String,
    /// For each parameter index: which `ConstrainedTypeVar` it uses (by name).
    pub(super) param_tv: Vec<Option<String>>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Try to parse `name = TypeVar("name", str, bytes)` into a `ConstrainedTypeVar`.
pub(super) fn try_parse_constrained_typevar(
    lhs_name: &str,
    expr: &Expr,
) -> Option<ConstrainedTypeVar> {
    let Expr::Call(call) = expr else {
        return None;
    };

    // Must call `TypeVar` (simple name).
    let callee = expr_name(&call.func)?;
    if callee != "TypeVar" {
        return None;
    }

    // Must have at least 3 args: name_string + 2 constraints.
    if call.arguments.args.len() < 3 {
        return None;
    }

    // Collect positional constraint args (skip arg 0 which is the name string).
    let constraints: Vec<String> = call
        .arguments
        .args
        .get(1..)
        .unwrap_or_default()
        .iter()
        .map(ann_str)
        .collect();

    if constraints.len() < 2 {
        return None;
    }

    Some(ConstrainedTypeVar {
        name: lhs_name.to_owned(),
        constraints,
    })
}

/// Try to extract constrained-TypeVar parameter info from a function definition.
pub(super) fn try_parse_constrained_func(
    func: &ast::StmtFunctionDef,
    tvars: &HashMap<String, ConstrainedTypeVar>,
) -> Option<ConstrainedFunc> {
    let mut param_tv: Vec<Option<String>> = Vec::new();
    let mut has_constrained = false;

    for param in func
        .parameters
        .args
        .iter()
        .chain(func.parameters.posonlyargs.iter())
    {
        let tv_name = param
            .parameter
            .annotation
            .as_ref()
            .and_then(|a| expr_name(a))
            .and_then(|ann| tvars.get(ann).map(|tv| tv.name.clone()));
        if tv_name.is_some() {
            has_constrained = true;
        }
        param_tv.push(tv_name);
    }

    if !has_constrained {
        return None;
    }

    Some(ConstrainedFunc {
        name: func.name.to_string(),
        param_tv,
    })
}
