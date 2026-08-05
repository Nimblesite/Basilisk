//! `tuples_type_compat`: Tuple starred-unpack type compatibility violation.
//!
//! Detects assignments where a tuple literal or a tuple-typed variable is
//! assigned to a target whose annotation contains a starred unpack expression
//! (`*tuple[T, ...]` or `*tuple[T]`) and the assignment is incompatible with
//! that annotation.
//!
//! Covers module-level bare reassignments of annotated tuple variables and
//! function-body variable assignments. Every verdict is structural over the
//! parsed `ruff` AST ([LINESCANPLAN-AST-MIGRATION], issue #408): declarations
//! are `AnnAssign` nodes, reassignments are `Assign` nodes, and tuple shapes
//! come from expression structure — never reconstructed source lines.
//!
//! ## Examples
//!
//! ```python
//! t1: tuple[int, *tuple[str]] = (1, "")  # OK
//! t1 = (1, "", "")  # E — too many elements for *tuple[str]
//!
//! t2: tuple[int, *tuple[str, ...]] = (1, "")  # OK
//! t2 = (1, 1, "")  # E — second element must be str
//!
//! def f(t1: tuple[int], t2: tuple[int, *tuple[int, ...]], t3: tuple[int, ...]):
//!     v2: tuple[int, *tuple[int, ...]]
//!     v2 = t3  # E — homogeneous tuple[int,...] not assignable to mixed starred form
//!     v3: tuple[int]
//!     v3 = t2  # E — t2 may have more elements than v3 allows
//!     v3 = t3  # E — t3 is unbounded, v3 is fixed length 1
//! ```
//!
//! # Specification
//!
//! <https://typing.readthedocs.io/en/latest/spec/tuples.html#type-compatibility-rules>

pub(super) mod annotation;

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Stmt, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

use annotation::{
    check_literal_against_shape, check_var_against_shape, has_starred_unpack, parse_tuple_shape,
    TupleShape,
};

const CODE: ErrorCode = ErrorCode {
    code: "tuples_type_compat",
    docs_url: "https://www.basilisk-python.dev/errors/tuples_type_compat",
};

/// Emits `tuples_type_compat` for incompatible starred-unpack tuple assignments.
pub(crate) struct TupleStarredUnpackCompatibility;

impl Rule for TupleStarredUnpackCompatibility {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let Some(resolver) = AnnotationResolver::for_module(module) else {
            return;
        };

        check_module_level(&parsed.ast.body, &resolver, &module.path, diagnostics);
        walk_functions(&parsed.ast.body, &resolver, &module.path, diagnostics);
    }
}

/// The span of an AST node's source range.
fn node_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

fn make_diag(message: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        message.to_owned(),
        span,
        path,
        Some("The assigned value must match the starred-unpack tuple structure".to_owned()),
        Some(
            "typing spec, tuples: a starred unpack constrains the minimum length and \
             per-position element types of the tuple"
                .to_owned(),
        ),
    )
}

// ---------------------------------------------------------------------------
// Module-level bare reassignment checking
// ---------------------------------------------------------------------------

/// Check module-level bare assignments like `t2 = (1, 1, "")` after a
/// preceding annotated declaration like `t2: tuple[int, *tuple[str, ...]] = ...`.
fn check_module_level(
    body: &[Stmt],
    resolver: &AnnotationResolver<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Annotated module variables whose annotation carries a starred unpack.
    let mut known_shapes: HashMap<&str, TupleShape> = HashMap::new();

    for stmt in body {
        match stmt {
            Stmt::AnnAssign(assign) => {
                let Expr::Name(target) = assign.target.as_ref() else {
                    continue;
                };
                if !has_starred_unpack(resolver, &assign.annotation) {
                    continue;
                }
                if let Some(shape) = parse_tuple_shape(resolver, &assign.annotation) {
                    let _ = known_shapes.insert(target.id.as_str(), shape);
                }
            }
            Stmt::Assign(assign) => {
                let [Expr::Name(target)] = assign.targets.as_slice() else {
                    continue;
                };
                let Some(shape) = known_shapes.get(target.id.as_str()) else {
                    continue;
                };
                let Expr::Tuple(literal) = assign.value.as_ref() else {
                    continue;
                };
                let elems: Vec<&Expr> = literal.elts.iter().collect();
                if let Some(msg) = check_literal_against_shape(&elems, shape) {
                    diagnostics.push(make_diag(msg, node_span(assign.range()), path));
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Function-body checking
// ---------------------------------------------------------------------------

/// Recursively visit every function definition, however nested.
fn walk_functions(
    body: &[Stmt],
    resolver: &AnnotationResolver<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                check_function_body(func_def, resolver, path, diagnostics);
                walk_functions(&func_def.body, resolver, path, diagnostics);
            }
            Stmt::ClassDef(class_def) => {
                walk_functions(&class_def.body, resolver, path, diagnostics);
            }
            _ => {}
        }
    }
}

/// Check one function's own body for incompatible assignments to
/// tuple-annotated local variables, using parameter shapes as source types.
fn check_function_body(
    func_def: &StmtFunctionDef,
    resolver: &AnnotationResolver<'_>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Parameter tuple shapes.
    let mut param_shapes: HashMap<&str, TupleShape> = HashMap::new();
    for param in func_def.parameters.iter_non_variadic_params() {
        let Some(ann) = param.annotation() else {
            continue;
        };
        if let Some(shape) = parse_tuple_shape(resolver, ann) {
            let _ = param_shapes.insert(param.name().as_str(), shape);
        }
    }

    // Local tuple annotations, recorded as declarations are seen in order.
    let mut local_shapes: HashMap<&str, TupleShape> = HashMap::new();

    for stmt in &func_def.body {
        match stmt {
            Stmt::AnnAssign(assign) => {
                let Expr::Name(target) = assign.target.as_ref() else {
                    continue;
                };
                if let Some(shape) = parse_tuple_shape(resolver, &assign.annotation) {
                    let _ = local_shapes.insert(target.id.as_str(), shape);
                }
            }
            Stmt::Assign(assign) => {
                let [Expr::Name(target)] = assign.targets.as_slice() else {
                    continue;
                };
                let Some(target_shape) = local_shapes.get(target.id.as_str()) else {
                    continue;
                };
                match assign.value.as_ref() {
                    // RHS is a parameter reference: shape-vs-shape check.
                    Expr::Name(rhs) => {
                        let Some(src_shape) = param_shapes.get(rhs.id.as_str()) else {
                            continue;
                        };
                        if let Some(msg) = check_var_against_shape(src_shape, target_shape) {
                            diagnostics.push(make_diag(msg, node_span(assign.range()), path));
                        }
                    }
                    // RHS is a tuple literal: element-wise check.
                    Expr::Tuple(literal) => {
                        let elems: Vec<&Expr> = literal.elts.iter().collect();
                        if let Some(msg) = check_literal_against_shape(&elems, target_shape) {
                            diagnostics.push(make_diag(msg, node_span(assign.range()), path));
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
