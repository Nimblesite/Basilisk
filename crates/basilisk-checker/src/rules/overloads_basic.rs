//! Implements [`overloads_basic`] from [CHKARCH-DIAG-OPTIONAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OPTIONAL
//! `overloads_basic`: No matching overload for subscript indexing.
//!
//! When a class defines overloaded `__getitem__` methods and a module-level
//! subscript expression (e.g. `b[""]`) passes an argument whose type is
//! incompatible with all overload signatures, Basilisk reports the error.
//!
//! ```python
//! from typing import overload
//!
//! class Bytes:
//!     @overload
//!     def __getitem__(self, __i: int) -> int: ...
//!     @overload
//!     def __getitem__(self, __s: slice) -> bytes: ...
//!     def __getitem__(self, __i_or_s: int | slice) -> int | bytes: ...
//!
//! b = Bytes()
//! b[""]  # E0072 -- no overload of __getitem__ accepts str
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "overloads_basic",
    docs_url: "https://www.basilisk-python.dev/errors/overloads_basic",
};

/// Emits `overloads_basic` for subscript indexing where no overloaded `__getitem__`
/// signature matches the argument type.
pub(crate) struct NoMatchingOverload;

impl Rule for NoMatchingOverload {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        module: &ResolvedModule,
        types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Bail on parse errors — those are reported separately as BSK-0000.
        if types.annotations().is_none() {
            return;
        }
        let source = &module.source;
        let path = &module.path;

        // Build a map from variable name -> class name for module-level variables
        // assigned via constructor calls (e.g. `b = Bytes()` -> "b" -> "Bytes").
        let var_class_map: HashMap<&str, &str> = module
            .module_vars
            .iter()
            .filter(|v| v.rhs_kind == basilisk_resolver::RhsKind::CallExpr)
            .filter_map(|v| {
                let rhs_span = v.rhs_span?;
                let rhs_text = slice_span(source, rhs_span)?;
                // Extract class name from "ClassName()" or "ClassName(args)"
                let class_name = rhs_text.split('(').next()?;
                let class_name = class_name.trim();
                if class_name.is_empty() {
                    return None;
                }
                // Verify it starts with uppercase (heuristic for class constructors)
                if !class_name.starts_with(|c: char| c.is_ascii_uppercase()) {
                    return None;
                }
                Some((v.name.as_str(), class_name))
            })
            .collect();

        if var_class_map.is_empty() {
            return;
        }

        // Collect overloaded __getitem__ methods by class name.
        // Each entry maps class_name -> Vec<annotation_text> for the first non-self parameter.
        let mut overload_getitem: HashMap<&str, Vec<&str>> = HashMap::new();
        for func in &module.functions {
            if func.name != "__getitem__" {
                continue;
            }
            if !func.is_overload {
                continue;
            }
            let Some(class_name) = func.class_name.as_deref() else {
                continue;
            };
            // The first parameter is `self`; the type-bearing parameter is the second.
            if let Some(param) = func.parameters.get(1) {
                if let Some(ann_span) = param.annotation_span {
                    if let Some(ann_text) = slice_span(source, ann_span) {
                        overload_getitem
                            .entry(class_name)
                            .or_default()
                            .push(ann_text.trim());
                    }
                }
            }
        }

        if overload_getitem.is_empty() {
            return;
        }

        // Scan the source for module-level subscript expressions: `varname[literal]`
        // We parse the source with basilisk_parser to get the AST, then walk it.
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        for stmt in &parsed.ast.body {
            let ruff_python_ast::Stmt::Expr(expr_stmt) = stmt else {
                continue;
            };

            check_subscript_expr(
                &expr_stmt.value,
                path,
                &var_class_map,
                &overload_getitem,
                diagnostics,
            );
        }
    }
}

/// Check an expression for subscript indexing with no matching overload.
fn check_subscript_expr(
    expr: &ruff_python_ast::Expr,
    path: &str,
    var_class_map: &HashMap<&str, &str>,
    overload_getitem: &HashMap<&str, Vec<&str>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;
    use ruff_text_size::Ranged as _;

    let Expr::Subscript(sub) = expr else {
        return;
    };

    // The subscripted value must be a simple name referencing a known class instance.
    let Expr::Name(name) = sub.value.as_ref() else {
        return;
    };

    let var_name = name.id.as_str();
    let Some(class_name) = var_class_map.get(var_name) else {
        return;
    };

    let Some(overload_annotations) = overload_getitem.get(class_name) else {
        return;
    };

    // Classify the subscript slice argument.
    let Some(slice_type) = classify_expr_type(&sub.slice) else {
        return; // Cannot determine the type -- skip.
    };

    // Check if any overload annotation accepts this type.
    let matches_any = overload_annotations
        .iter()
        .any(|ann| type_matches_annotation(slice_type, ann));

    if !matches_any {
        let range = sub.range();
        let span = Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        };
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "No matching overload for `{var_name}[...]`: argument type `{slice_type}` \
                 does not match any `@overload` signature of `{class_name}.__getitem__`"
            ),
            span,
            path,
            Some(format!(
                "Expected one of: {}",
                overload_annotations.join(", ")
            )),
            Some(
                "Each `@overload` variant specifies which argument types are accepted; \
                 no variant matches the provided argument"
                    .to_owned(),
            ),
        ));
    }
}

/// Classify the Python type of a literal expression.
/// Returns `None` for non-literal or unrecognizable expressions.
fn classify_expr_type(expr: &ruff_python_ast::Expr) -> Option<&'static str> {
    use ruff_python_ast::Expr;
    match expr {
        Expr::StringLiteral(_) => Some("str"),
        Expr::NumberLiteral(num) => {
            if num.value.is_int() {
                Some("int")
            } else {
                Some("float")
            }
        }
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::NoneLiteral(_) => Some("None"),
        Expr::Tuple(_) => Some("tuple"),
        Expr::List(_) => Some("list"),
        Expr::Dict(_) => Some("dict"),
        Expr::Set(_) => Some("set"),
        Expr::Slice(_) => Some("slice"),
        _ => None,
    }
}

// ##########################################################################
// # DELETED BODY — `type_matches_annotation`. DO NOT RESTORE IT. DO NOT    #
// # SUBSTITUTE A PLACEHOLDER THAT RETURNS `true` OR `false`.               #
// #                                                                        #
// # The ENTIRE function was string comparison. Every line:                 #
// #                                                                        #
// #   if ann == type_name           — type identity by rendered equality   #
// #   if ann == "object"            — the top type by builtin spelling     #
// #   if ann.contains('|') { ann.split('|') … }  — union decomposition by  #
// #                                  splitting source text on a character  #
// #   if ann == "int" && type_name == "bool"     — the numeric tower as    #
// #   if ann == "float" && …                       two literal spellings   #
// #                                                                        #
// # Its own doc comment called it "a simplified check that handles common  #
// # cases". It was not a simplified subtype check; it was no subtype check #
// # at all. `A | B` written across two lines, `int` imported under an      #
// # alias, or a user class named `object` each broke it, and a class       #
// # merely spelled the same as another satisfied it.                       #
// #                                                                        #
// # Union decomposition, the numeric tower, and the top type are all       #
// # properties of RESOLVED types. Ask the binding table.                   #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// DELETED — panics. The signature survives only so its callers stay visible
/// as the rebuild map; see the banner above.
fn type_matches_annotation(_type_name: &str, _annotation: &str) -> bool {
    panic!(
        "basilisk-checker: `type_matches_annotation` was DELETED because it compared \
         TYPE SPELLINGS end to end — `ann == type_name`, `ann == \"object\"`, \
         `ann.split('|')` for unions, and `\"int\"`/`\"float\"`/`\"bool\"` literals for \
         the numeric tower. It panics because the real implementation — relating two \
         resolved types — DOES NOT EXIST YET. Do not restore the comparisons and do \
         not answer `true`/`false` in its place."
    )
}
