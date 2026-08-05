//! Implements [`tuples_index_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `tuples_index_2`: Tuple index out of range.
//!
//! Detects subscript access on a fixed-length builtin `tuple[T1, T2, ...]`
//! parameter where the index is an inline integer literal that falls outside
//! the valid range `[-len, len-1]`.
//!
//! Every verdict is structural over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION], issue #408): subscripts are `Expr::Subscript`
//! nodes scoped to their own function body — never substring hits on raw lines
//! that also match comments and strings.
//!
//! Indices supplied through a parameter's declared type are no longer resolved:
//! that path recognised the annotation by the name written at the use site,
//! which the symbol-naming ban in `CLAUDE.md` forbids permanently, so it has
//! been deleted rather than re-expressed behind another spelling comparison.
//!
//! ```python
//! def f(v: tuple[int, str, list[bool]]):
//!     v[4]   # E — index 4 out of range
//!     v[-4]  # E — index -4 out of range (valid: -3..-1)
//! ```

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::visitor::{walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{Expr, Number, Stmt, UnaryOp};
use ruff_text_size::Ranged;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::typing_form::subscript_args;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "tuples_index_2",
    docs_url: "https://www.basilisk-python.dev/errors/tuples_index_2",
};

/// Emits `tuples_index_2` when a tuple is subscripted with an out-of-range literal index.
pub(crate) struct TupleIndexOutOfRange;

impl Rule for TupleIndexOutOfRange {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        walk_functions(&parsed.ast.body, &module.path, diagnostics);
    }
}

/// Recursively visit every function definition, however nested.
fn walk_functions(body: &[Stmt], path: &str, diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                check_function(func_def, path, diagnostics);
                walk_functions(&func_def.body, path, diagnostics);
            }
            Stmt::ClassDef(class_def) => {
                walk_functions(&class_def.body, path, diagnostics);
            }
            _ => {}
        }
    }
}

/// Check one function: collect its fixed-tuple parameters, then walk its own
/// body (not nested defs — GitHub #284) for subscripts.
fn check_function(
    func_def: &ruff_python_ast::StmtFunctionDef,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut tuple_params: HashMap<&str, usize> = HashMap::new();

    for param in func_def.parameters.iter_non_variadic_params() {
        let Some(annotation) = param.annotation() else {
            continue;
        };
        if let Some(len) = fixed_tuple_length(annotation) {
            let _ = tuple_params.insert(param.name().as_str(), len);
        }
    }
    if tuple_params.is_empty() {
        return;
    }

    let mut visitor = SubscriptVisitor {
        tuple_params: &tuple_params,
        path,
        diagnostics,
    };
    for stmt in &func_def.body {
        visitor.visit_stmt(stmt);
    }
}

/// Walks one function body; does not descend into nested function or class
/// definitions, whose identically-named bindings are different variables.
struct SubscriptVisitor<'a> {
    tuple_params: &'a HashMap<&'a str, usize>,
    path: &'a str,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl<'ast> Visitor<'ast> for SubscriptVisitor<'_> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            other => walk_stmt(self, other),
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if let Expr::Subscript(subscript) = expr {
            self.check_subscript(subscript);
        }
        walk_expr(self, expr);
    }
}

impl SubscriptVisitor<'_> {
    /// Report when a fixed-length tuple parameter is indexed out of range.
    fn check_subscript(&mut self, subscript: &ruff_python_ast::ExprSubscript) {
        let Expr::Name(base) = subscript.value.as_ref() else {
            return;
        };
        let Some(&tuple_len) = self.tuple_params.get(base.id.as_str()) else {
            return;
        };
        let Some(index) = int_literal_value(&subscript.slice) else {
            return;
        };

        let Ok(tuple_len_i64) = i64::try_from(tuple_len) else {
            return;
        };
        let out_of_range = if index >= 0 {
            index >= tuple_len_i64
        } else {
            index < -tuple_len_i64
        };
        if !out_of_range {
            return;
        }

        let range = subscript.range();
        let max_pos = tuple_len_i64 - 1;
        self.diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!("Tuple index {index} is out of range for `tuple` of length {tuple_len}"),
            Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            self.path,
            Some(format!(
                "Valid indices for a {tuple_len}-element tuple are \
                 -{tuple_len}..{max_pos} (inclusive)"
            )),
            Some(
                "PEP 484: indexing a fixed-length tuple with an out-of-range \
                 literal integer is a type error."
                    .to_owned(),
            ),
        ));
    }
}

/// The length of a fixed-length builtin `tuple[T1, T2, ...]` annotation. `None`
/// for variadic tuples (`tuple[int, ...]`, unpacks) and non-tuple annotations.
fn fixed_tuple_length(annotation: &Expr) -> Option<usize> {
    let Expr::Subscript(subscript) = annotation else {
        return None;
    };
    let is_tuple_head =
        matches!(subscript.value.as_ref(), Expr::Name(name) if name.id.as_str() == "tuple");
    if !is_tuple_head {
        return None;
    }
    let args = subscript_args(&subscript.slice);
    // `tuple[()]` is the empty-tuple type.
    if let [Expr::Tuple(inner)] = args.as_slice() {
        if inner.elts.is_empty() {
            return Some(0);
        }
    }
    // Variadic or unpacked forms have no fixed length.
    let variadic = args
        .iter()
        .any(|arg| matches!(arg, Expr::EllipsisLiteral(_) | Expr::Starred(_)));
    if variadic {
        return None;
    }
    Some(args.len())
}

/// The value of an integer literal expression, including a negated one.
fn int_literal_value(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::NumberLiteral(lit) => match &lit.value {
            Number::Int(int) => int.as_i64(),
            _ => None,
        },
        Expr::UnaryOp(unary) if unary.op == UnaryOp::USub => {
            int_literal_value(&unary.operand).map(i64::checked_neg)?
        }
        _ => None,
    }
}
