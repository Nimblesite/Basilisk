//! BSK-E0148: Generic type argument violations.
//!
//! Detects several generic-type errors:
//!
//! 1. **Constrained `TypeVar` constraint mismatch**: When a function parameter is typed
//!    with a constrained `TypeVar` (e.g. `AnyStr = TypeVar("AnyStr", str, bytes)`),
//!    all arguments bound to the same type variable must belong to the same constraint.
//!    Passing `(str_val, bytes_val)` for `(x: AnyStr, y: AnyStr)` is an error.
//!
//! 2. **Mapping subscript key type mismatch**: When a `Mapping`-derived type has a
//!    known key type (e.g. `MyMap[str, int]`), indexing with a literal of the wrong
//!    type (e.g. `my_map[0]`) is an error.
//!
//! 3. **Generic metaclass usage**: Using a parameterized generic class as a metaclass
//!    (`metaclass=SomeGeneric[T]`) is not supported by the Python type system.
//!
//! ```python
//! from typing import TypeVar
//!
//! AnyStr = TypeVar("AnyStr", str, bytes)
//!
//! def concat(x: AnyStr, y: AnyStr) -> AnyStr:
//!     return x + y
//!
//! def bad(s: str, b: bytes) -> None:
//!     concat(s, b)  # E — constraint groups do not match
//! ```

mod checkers;
mod helpers;
mod typevar;

use std::collections::HashMap;

use ruff_python_ast::Stmt;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

use checkers::check_stmts;
use helpers::{ann_str, expr_name, split_top_level};
use typevar::{
    try_parse_constrained_func, try_parse_constrained_typevar, ConstrainedFunc, ConstrainedTypeVar,
};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0148",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0148",
};

/// Emits BSK-E0148 for generic type argument violations.
pub(crate) struct GenericTypeArgViolation;

impl Rule for GenericTypeArgViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        let ctx = ModuleContext::from_ast(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// Module-level knowledge needed to check calls.
pub(super) struct ModuleContext {
    /// All constrained `TypeVars` defined at module level.
    pub(super) constrained_tvars: HashMap<String, ConstrainedTypeVar>,
    /// Functions that have at least one constrained-TypeVar parameter.
    pub(super) constrained_funcs: Vec<ConstrainedFunc>,
    /// Variables with known types: name -> type annotation text.
    pub(super) var_types: HashMap<String, String>,
    /// Classes that represent Mapping types with known key types.
    /// Maps class name -> (`key_type_text`, `value_type_text`).
    pub(super) mapping_vars: HashMap<String, (String, String)>,
}

impl ModuleContext {
    /// Build module context by scanning top-level statements.
    pub(super) fn from_ast(stmts: &[Stmt]) -> Self {
        let mut constrained_tvars: HashMap<String, ConstrainedTypeVar> = HashMap::new();
        let mut constrained_funcs: Vec<ConstrainedFunc> = Vec::new();
        let mut var_types: HashMap<String, String> = HashMap::new();
        let mut mapping_vars: HashMap<String, (String, String)> = HashMap::new();

        // Pass 1: collect TypeVar definitions.
        for stmt in stmts {
            if let Stmt::Assign(assign) = stmt {
                if assign.targets.len() == 1 {
                    if let Some(lhs_name) = assign.targets.first().and_then(expr_name) {
                        if let Some(ctv) = try_parse_constrained_typevar(lhs_name, &assign.value) {
                            let _ = constrained_tvars.insert(lhs_name.to_owned(), ctv);
                        }
                    }
                }
            }
        }

        // Pass 2: collect function signatures and variable annotations.
        for stmt in stmts {
            match stmt {
                Stmt::FunctionDef(func) => {
                    if let Some(cfunc) = try_parse_constrained_func(func, &constrained_tvars) {
                        constrained_funcs.push(cfunc);
                    }
                }
                Stmt::AnnAssign(ann) => {
                    if let Some(var_name) = expr_name(&ann.target) {
                        let ann_text = ann_str(&ann.annotation);
                        // Record type annotation for all variables.
                        let _ = var_types.insert(var_name.to_owned(), ann_text.clone());
                        // Also check for Mapping subscripts.
                        if let Some((key_ty, val_ty)) = parse_mapping_annotation(&ann_text) {
                            let _ = mapping_vars.insert(var_name.to_owned(), (key_ty, val_ty));
                        }
                    }
                }
                _ => {}
            }
        }

        Self {
            constrained_tvars,
            constrained_funcs,
            var_types,
            mapping_vars,
        }
    }
}

// ---------------------------------------------------------------------------
// Mapping annotation parsing
// ---------------------------------------------------------------------------

/// Detect Mapping-like annotations with explicit key/value types.
///
/// Recognises:
/// - `MyMap1[str, int]`, `MyMap2[int, str]`
/// - `Mapping[K, V]`, `Dict[K, V]`, `dict[K, V]`
///
/// Returns `(key_type, value_type)` or `None`.
pub(super) fn parse_mapping_annotation(ann: &str) -> Option<(String, String)> {
    let ann = ann.trim();
    // Look for `Name[k, v]` pattern.
    let bracket_pos = ann.find('[')?;
    let inner = ann.get(bracket_pos + 1..ann.rfind(']')?)?;
    let args = split_top_level(inner);
    if args.len() < 2 {
        return None;
    }
    let key_ty = args.first()?.trim().to_owned();
    let val_ty = args.get(1)?.trim().to_owned();
    // Only return for types that are clearly mapping-like (have exactly 2 args
    // and look like type names, not bare TypeVar names by convention).
    if key_ty.is_empty() || val_ty.is_empty() {
        return None;
    }
    Some((key_ty, val_ty))
}
