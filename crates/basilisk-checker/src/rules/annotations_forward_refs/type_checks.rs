//! Implements [`annotations_forward_refs`] from [CHKARCH-DIAG-IMMUTABILITY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMMUTABILITY
//! Structural annotation validity checks for `annotations_forward_refs`.
//!
//! Every verdict is made over the parsed `ruff` expression tree through the
//! shared type-expression judge ([LINESCANPLAN-AST-MIGRATION], issue #408) —
//! never by scanning annotation source text. Annotations are eagerly
//! evaluated, so a top-level string is a forward reference whose content must
//! itself parse to a type expression, while a string as a union operand
//! (`"A" | int`) is a runtime `str | type` error.

use std::collections::HashSet;

use basilisk_resolver::{ImportKind, ResolvedModule};
use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::Expr;

use crate::rules::shared::{is_type_expression, StringPolicy, TypeExprJudge};

// ---------------------------------------------------------------------------
// Structural checks on annotation expressions
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation expression is not a valid type
/// expression: literals, collection displays, comprehensions, lambdas,
/// conditionals, boolean operators, calls, f-strings, unparseable forward
/// references, and names bound to non-types. `Generic` is rejected because
/// PEP 484 allows it only in a class's base list, never as an annotation.
pub(super) fn is_invalid_type_annotation(expr: &Expr, non_type_names: &HashSet<String>) -> bool {
    if subscript_base_is(expr, "Annotated") {
        return false; // `Annotated[...]` is validated by `qualifiers_annotated`.
    }
    let judge = TypeExprJudge {
        non_type: &|name| name == "Generic" || non_type_names.contains(name),
        strings: StringPolicy::EagerForwardRef,
    };
    !is_type_expression(expr, &judge)
}

/// Whether `expr` is a subscript whose base denotes `name` (bare or dotted).
fn subscript_base_is(expr: &Expr, name: &str) -> bool {
    let Expr::Subscript(subscript) = expr else {
        return false;
    };
    base_name(&subscript.value) == Some(name)
}

/// The rightmost identifier of a subscript base: `Callable` in both
/// `Callable[...]` and `typing.Callable[...]`.
fn base_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(name) => Some(name.id.as_str()),
        Expr::Attribute(attr) => Some(attr.attr.as_str()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Non-type name detection
// ---------------------------------------------------------------------------

/// Build a set of names that are definitely not valid type expressions:
/// - Names bound to modules via plain `import X` statements (`import a.b`
///   binds `a` at runtime).
/// - Names bound to unannotated simple literal values.
pub(super) fn collect_non_type_names(module: &ResolvedModule) -> HashSet<String> {
    let mut names = HashSet::new();

    for import in &module.imports {
        if import.kind == ImportKind::Plain {
            let local_name = import
                .module
                .split('.')
                .next()
                .unwrap_or(import.module.as_str());
            let _ = names.insert(local_name.to_owned());
        }
    }

    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let is_simple_literal = matches!(
            var.rhs_kind,
            basilisk_resolver::RhsKind::IntLiteral
                | basilisk_resolver::RhsKind::FloatLiteral
                | basilisk_resolver::RhsKind::StrLiteral
                | basilisk_resolver::RhsKind::BoolLiteral
                | basilisk_resolver::RhsKind::BytesLiteral
                | basilisk_resolver::RhsKind::EmptyList
                | basilisk_resolver::RhsKind::EmptyDict
                | basilisk_resolver::RhsKind::NoneValue
        );
        if is_simple_literal {
            let _ = names.insert(var.name.clone());
        }
    }

    names
}

// ---------------------------------------------------------------------------
// ParamSpec invalid annotation detection
// ---------------------------------------------------------------------------

/// Returns `true` when the annotation uses a `ParamSpec` in an invalid
/// position (PEP 612).
///
/// Valid positions for `P` (a `ParamSpec`):
/// - As the parameters argument of `Callable`: `Callable[P, ReturnType]`
/// - Subscripting a class or alias that is itself generic over a
///   `ParamSpec`: `Base[P]`
///
/// Invalid positions (detected here):
/// - Bare `P` as a direct annotation
/// - `Concatenate[...]` used directly as an annotation (only valid inside
///   `Callable`)
/// - `P` inside a non-`Callable` subscript: `list[P]`, `dict[str, P]`
/// - `P` as the return type of `Callable`: `Callable[[int, str], P]`
pub(super) fn is_paramspec_invalid_annotation(
    expr: &Expr,
    paramspec_names: &HashSet<&str>,
    paramspec_generic_bases: &HashSet<&str>,
) -> bool {
    if paramspec_names.is_empty() {
        return false;
    }
    match expr {
        Expr::Name(name) => paramspec_names.contains(name.id.as_str()),
        Expr::Subscript(subscript) => {
            let Some(base) = base_name(&subscript.value) else {
                return false;
            };
            if base == "Concatenate" {
                return true;
            }
            if paramspec_generic_bases.contains(base) {
                return false;
            }
            if base == "Callable" {
                return callable_return_is_paramspec(&subscript.slice, paramspec_names);
            }
            references_paramspec(&subscript.slice, paramspec_names)
        }
        _ => false,
    }
}

/// `Callable[..., P]` — the last subscript argument is the return type.
fn callable_return_is_paramspec(slice: &Expr, paramspec_names: &HashSet<&str>) -> bool {
    let Expr::Tuple(tuple) = slice else {
        return false;
    };
    matches!(
        tuple.elts.last(),
        Some(Expr::Name(name)) if paramspec_names.contains(name.id.as_str())
    )
}

/// Whether any `Name` in the expression tree is a declared `ParamSpec`.
fn references_paramspec(expr: &Expr, paramspec_names: &HashSet<&str>) -> bool {
    struct Finder<'a> {
        names: &'a HashSet<&'a str>,
        found: bool,
    }
    impl<'ast> Visitor<'ast> for Finder<'_> {
        fn visit_expr(&mut self, expr: &'ast Expr) {
            if self.found {
                return;
            }
            if let Expr::Name(name) = expr {
                if self.names.contains(name.id.as_str()) {
                    self.found = true;
                    return;
                }
            }
            walk_expr(self, expr);
        }
    }
    let mut finder = Finder {
        names: paramspec_names,
        found: false,
    };
    finder.visit_expr(expr);
    finder.found
}
