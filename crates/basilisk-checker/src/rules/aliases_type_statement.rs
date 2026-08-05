//! Implements [`aliases_type_statement`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `aliases_type_statement`: Invalid RHS in a PEP 695 `type X = rhs` statement.
//!
//! PEP 695 requires the RHS of a `type` statement to be a valid type
//! expression. The RHS is validated **structurally** on the parsed `ruff`
//! expression tree via the shared judge ([LINESCANPLAN-AST-MIGRATION], issue
//! #379 — substring matching both missed invalid forms and misfired on
//! identifiers containing matched text): names, dotted names, `X | Y`
//! unions, `None`, string forward references, and subscriptions of those are
//! type expressions; every other expression form (literals, calls, lambdas,
//! conditionals, comparisons, comprehensions, boolean operators, attribute
//! access on a subscript) is not. The `type` statement is lazily evaluated,
//! so string forward references are valid anywhere in the RHS, including as
//! union operands — recursive aliases spell themselves that way.
//!
//! ```python
//! type BadAlias1 = [int, str]   # E — list literal
//! type BadAlias2 = True         # E — bool literal
//! type BadAlias3 = 1            # E — int literal
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, RhsKind, Span};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::{is_type_expression, StringPolicy, TypeExprJudge};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "aliases_type_statement",
    docs_url: "https://www.basilisk-python.dev/errors/aliases_type_statement",
};

fn make_diag(name: &str, span: Span, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Invalid type expression in `type {name}` alias"),
        span,
        path,
        Some("The RHS of a `type` statement must be a valid type expression".to_owned()),
        Some(
            "PEP 695: `type X = T` requires T to be a type, not a literal or expression".to_owned(),
        ),
    )
}

/// Index every `type X = rhs` value expression in the module's ALREADY-PARSED
/// AST, keyed by the span the resolver recorded for it.
///
/// The RHS is a node in that tree, not text to be parsed again: re-parsing it
/// per statement cost a full `ruff` expression parse for every alias in the
/// file, which is most of the work on an alias-dense module.
fn index_rhs_nodes<'ast>(stmts: &'ast [Stmt], out: &mut HashMap<(u32, u32), &'ast Expr>) {
    for stmt in stmts {
        match stmt {
            Stmt::TypeAlias(alias) => {
                let range = alias.value.range();
                let _ = out.insert((range.start().to_u32(), range.end().to_u32()), &alias.value);
            }
            Stmt::ClassDef(class) => index_rhs_nodes(&class.body, out),
            Stmt::FunctionDef(function) => index_rhs_nodes(&function.body, out),
            _ => {}
        }
    }
}

/// Collect names of module-level variables that are not valid types.
fn collect_non_type_names(module: &ResolvedModule) -> HashSet<&str> {
    module
        .module_vars
        .iter()
        .filter(|v| !v.has_annotation)
        .filter(|v| {
            matches!(
                v.rhs_kind,
                RhsKind::IntLiteral
                    | RhsKind::FloatLiteral
                    | RhsKind::StrLiteral
                    | RhsKind::BoolLiteral
                    | RhsKind::BytesLiteral
                    | RhsKind::EmptyList
                    | RhsKind::EmptyDict
                    | RhsKind::NoneValue
            )
        })
        .map(|v| v.name.as_str())
        .collect()
}

/// Emits `aliases_type_statement` when a `type X = rhs` statement has an invalid type expression.
pub(crate) struct TypeStatementInvalidRhs;

impl Rule for TypeStatementInvalidRhs {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let path = &module.path;
        // The module's own AST, parsed once and shared with every other rule
        // that needs it. A module that does not parse has no type statements to
        // judge — the parser reports that itself.
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let mut rhs_nodes = HashMap::new();
        index_rhs_nodes(&parsed.ast.body, &mut rhs_nodes);
        let module_non_types = collect_non_type_names(module);

        for stmt in &module.type_statements {
            let Some(rhs) = rhs_nodes.get(&(stmt.rhs_span.start, stmt.rhs_span.end)) else {
                continue;
            };
            // PEP 695 binds the statement's own type parameters in its
            // annotation scope, so `T = 1` followed by `type Wrapper[T] = T`
            // is valid — the parameter shadows the module binding.
            let judge = TypeExprJudge {
                non_type: &|name| {
                    module_non_types.contains(name)
                        && !stmt.param_names.iter().any(|param| param == name)
                },
                strings: StringPolicy::LazyForwardRef,
            };
            if !is_type_expression(rhs, &judge) {
                diagnostics.push(make_diag(&stmt.name, stmt.name_span, path));
            }
        }
    }
}
