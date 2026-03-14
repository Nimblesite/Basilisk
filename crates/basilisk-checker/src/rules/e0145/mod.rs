//! BSK-E0145: Invalid `type[X]` usage violations.
//!
//! Detects several categories of invalid use of `type[X]` (or `Type[X]`):
//!
//! 1. **Callable passed as `type[T]` argument** — `Callable` and other special
//!    forms are not valid class objects and cannot be passed where `type[T]` is
//!    expected.
//!
//! 2. **Incompatible class passed to `type[A | B]`** — when a function expects
//!    `type[A | B]`, passing a class that is neither `A` nor `B` is an error.
//!
//! 3. **Unknown attribute access on `type[object]`** — unlike `type[Any]`,
//!    `type[object]` only exposes `object`'s own attributes; accessing any other
//!    member is an error.
//!
//! 4. **Unknown attribute access on a `TypeAlias` bound to `type` / `Type`** —
//!    a bare alias such as `TA1: TypeAlias = Type` resolves to `type[Any]`, but
//!    the alias *name itself* (used at module scope like `TA1.unknown`) does not
//!    expose arbitrary attributes.
//!
//! ```python
//! from typing import Callable, TypeVar
//!
//! T = TypeVar("T")
//!
//! def func5(x: type[T]) -> None: pass
//!
//! func5(Callable)   # E — Callable is not a class
//!
//! class A: ...
//! class B: ...
//! class C: ...
//!
//! def func4(x: type[A | B]) -> None: pass
//! func4(C)          # E — C is not A or B
//!
//! def func8(a: type[object]) -> None:
//!     a.unknown     # E — type[object] does not expose arbitrary attributes
//! ```

mod helpers;
mod walkers;

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

use helpers::{
    expr_simple_name, expr_to_str, is_type_annotation, is_typevar_call, strip_type_bracket,
};
use walkers::check_stmts;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0145",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0145",
};

/// Special-form names that are not valid class objects for `type[T]`.
const SPECIAL_FORMS: &[&str] = &[
    "Callable",
    "Union",
    "Optional",
    "ClassVar",
    "Final",
    "Literal",
    "Annotated",
    "TypeGuard",
    "TypeIs",
    "Never",
    "NoReturn",
    "LiteralString",
    "Self",
    "Unpack",
    "TypeVarTuple",
    "ParamSpec",
    "Concatenate",
    "Required",
    "NotRequired",
    "ReadOnly",
    "TypeAlias",
];

/// Emits BSK-E0145 for invalid `type[X]` usages.
pub(crate) struct TypeBracketViolation;

impl Rule for TypeBracketViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        let ctx = ModuleCtx::build(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Module-level context: collect class names, TypeAlias bindings, function
// parameter annotations.
// ---------------------------------------------------------------------------

/// Collected per-module context needed by the checker.
pub(super) struct ModuleCtx {
    /// Class names defined at module scope.
    pub(super) class_names: Vec<String>,
    /// `TypeVar` names (i.e., assigned via `TypeVar(...)`).
    pub(super) typevar_names: Vec<String>,
    /// `TypeAlias` bindings: alias name → annotation text of the RHS.
    /// e.g. `TA1: TypeAlias = Type` → `("TA1", "Type")`.
    pub(super) type_aliases: HashMap<String, String>,
    /// Module-level function signatures: name → list of (`param_name`, `annotation_text`).
    pub(super) func_params: HashMap<String, Vec<(String, String)>>,
}

impl ModuleCtx {
    /// Build context by scanning the top-level statements of the parsed AST.
    pub(super) fn build(stmts: &[Stmt]) -> Self {
        let mut class_names = Vec::new();
        let mut typevar_names = Vec::new();
        let mut type_aliases: HashMap<String, String> = HashMap::new();
        let mut func_params: HashMap<String, Vec<(String, String)>> = HashMap::new();

        for stmt in stmts {
            match stmt {
                Stmt::ClassDef(cls) => {
                    class_names.push(cls.name.to_string());
                }
                Stmt::Assign(assign) => {
                    // Detect `X = TypeVar("X")` or `X = TypeVar("X", bound=...)`.
                    if assign.targets.len() == 1 {
                        if let Some(name) = assign.targets.first().and_then(expr_simple_name) {
                            if is_typevar_call(&assign.value) {
                                typevar_names.push(name.to_owned());
                            }
                        }
                    }
                }
                Stmt::AnnAssign(ann) => {
                    // Detect `TA: TypeAlias = Type` / `TA: TypeAlias = type[Any]` etc.
                    let is_alias = match ann.annotation.as_ref() {
                        Expr::Name(n) => n.id.as_str() == "TypeAlias",
                        _ => false,
                    };
                    if is_alias {
                        if let Some(name) = expr_simple_name(&ann.target) {
                            if let Some(value) = &ann.value {
                                let rhs = expr_to_str(value);
                                let _ = type_aliases.insert(name.to_owned(), rhs);
                            }
                        }
                    }
                }
                Stmt::FunctionDef(func) => {
                    let mut params: Vec<(String, String)> = Vec::new();
                    for pwd in &func.parameters.args {
                        let pname = pwd.parameter.name.to_string();
                        let ann = pwd
                            .parameter
                            .annotation
                            .as_ref()
                            .map(|a| expr_to_str(a))
                            .unwrap_or_default();
                        if !ann.is_empty() {
                            params.push((pname, ann));
                        }
                    }
                    for pwd in &func.parameters.posonlyargs {
                        let pname = pwd.parameter.name.to_string();
                        let ann = pwd
                            .parameter
                            .annotation
                            .as_ref()
                            .map(|a| expr_to_str(a))
                            .unwrap_or_default();
                        if !ann.is_empty() {
                            params.push((pname, ann));
                        }
                    }
                    if !params.is_empty() {
                        let _ = func_params.insert(func.name.to_string(), params);
                    }
                }
                _ => {}
            }
        }

        Self {
            class_names,
            typevar_names,
            type_aliases,
            func_params,
        }
    }

    /// Returns true if `name` is a known module-level class.
    pub(super) fn is_class(&self, name: &str) -> bool {
        self.class_names.iter().any(|c| c == name)
    }

    /// Returns true if `name` is a `TypeVar`.
    pub(super) fn is_typevar(&self, name: &str) -> bool {
        self.typevar_names.iter().any(|t| t == name)
    }

    /// Returns true if `name` is a `TypeAlias` binding that resolves to a
    /// `type` / `Type` variant (with or without a parameter).
    pub(super) fn is_type_alias(&self, name: &str) -> bool {
        self.type_aliases
            .get(name)
            .is_some_and(|rhs| is_type_annotation(rhs))
    }

    /// Return the union members if the annotation is `type[A | B | ...]`.
    /// Returns `None` if it is not a union-parameterised `type[…]`.
    pub(super) fn type_union_members(ann: &str) -> Option<Vec<&str>> {
        // Accept both `type[...]` and `Type[...]`.
        let inner = strip_type_bracket(ann)?;
        if inner.contains(" | ") {
            Some(inner.split(" | ").map(str::trim).collect())
        } else {
            None
        }
    }
}
