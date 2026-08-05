//! Implements [LINESCANPLAN-AST-MIGRATION]. See
//! docs/plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-AST-MIGRATION
//!
//! The ONE structural judge of type-expression validity for checker rules
//! (issues #379, #408). Verdicts come from the parsed `ruff` expression tree,
//! never from source text: renaming an identifier or reformatting whitespace
//! cannot change an answer, and no branch of this module may compare against
//! the spelling of a conformance-suite fixture.
//!
//! Type-expression grammar accepted here: names, dotted-name attribute
//! chains, `None`, `X | Y` unions of type expressions, subscriptions whose
//! BASE is a type expression, and string forward references whose content
//! parses to a type expression. Subscript *arguments* are never descended
//! into — special forms (`Literal[...]`, `Callable[[...], X]`,
//! `Annotated[X, ...]`) legitimately hold non-type expressions there.

use std::collections::HashMap;

use basilisk_resolver::Span;
use ruff_python_ast::visitor::{walk_expr, Visitor};
use ruff_python_ast::{Expr, ModModule, Operator};
use ruff_text_size::Ranged;

use crate::annotation::AnnotationResolver;
use crate::types::InferredType;

/// How string literals inside the judged expression are treated.
///
/// The evaluation regime of the surrounding construct decides this — it is a
/// property of Python semantics, not of any particular test corpus.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StringPolicy {
    /// The construct is lazily evaluated (PEP 695 `type` statements): a
    /// string anywhere is a forward reference, judged by parsing its content.
    LazyForwardRef,
    /// The construct is eagerly evaluated (annotations, PEP 613 alias
    /// values): a TOP-LEVEL string is a forward reference, but a string as a
    /// union operand (`"A" | int`) is a runtime `str | type` error.
    EagerForwardRef,
    /// The construct is a plain runtime value position (implicit-alias
    /// candidate RHS): a string is a `str` value, never a type.
    RejectValue,
}

/// The judging context: which names are known non-types, and the string
/// regime of the surrounding construct.
pub(crate) struct TypeExprJudge<'a> {
    /// Answers "is this bare name bound to a non-type runtime value?".
    /// Attribute heads are exempt — module references (`types.ModuleType`)
    /// are legitimate there even when the bare module name is not a type.
    pub(crate) non_type: &'a dyn Fn(&str) -> bool,
    pub(crate) strings: StringPolicy,
}

/// Whether `expr` has the structural shape of a type expression.
pub(crate) fn is_type_expression(expr: &Expr, judge: &TypeExprJudge<'_>) -> bool {
    valid(expr, judge, true)
}

fn valid(expr: &Expr, judge: &TypeExprJudge<'_>, top: bool) -> bool {
    match expr {
        Expr::Name(name) => !(judge.non_type)(name.id.as_str()),
        Expr::Attribute(attr) => is_dotted_name(&attr.value),
        Expr::NoneLiteral(_) => true,
        Expr::StringLiteral(lit) => string_valid(lit.value.to_str(), judge, top),
        Expr::Subscript(subscript) => valid(&subscript.value, judge, false),
        Expr::BinOp(binop) if binop.op == Operator::BitOr => {
            valid(&binop.left, judge, false) && valid(&binop.right, judge, false)
        }
        // PEP 646: `*Ts` / `*tuple[int, ...]` unpacks a TypeVarTuple or
        // tuple type — a type-expression form in variadic positions.
        Expr::Starred(starred) => valid(&starred.value, judge, false),
        _ => false,
    }
}

/// A dotted-name chain (`a`, `a.b`, `a.b.c`) — the only shape a type
/// expression's attribute access may take. Attribute access on anything else
/// (`list[int].attr`) is a value operation, not a type.
fn is_dotted_name(expr: &Expr) -> bool {
    match expr {
        Expr::Name(_) => true,
        Expr::Attribute(attr) => is_dotted_name(&attr.value),
        _ => false,
    }
}

fn string_valid(content: &str, judge: &TypeExprJudge<'_>, top: bool) -> bool {
    let allowed = match judge.strings {
        StringPolicy::RejectValue => false,
        StringPolicy::LazyForwardRef => true,
        StringPolicy::EagerForwardRef => top,
    };
    allowed && forward_ref_is_type_expression(content, judge)
}

/// Whether a forward-reference string's content is a type expression: it must
/// parse as a Python expression (the spec treats triple-quoted references as
/// implicitly parenthesized, so the content is wrapped before parsing) and
/// that expression must itself pass the judge as a fresh top level.
pub(crate) fn forward_ref_is_type_expression(content: &str, judge: &TypeExprJudge<'_>) -> bool {
    let Ok(parsed) = ruff_python_parser::parse_expression(&format!("({content})")) else {
        return false;
    };
    valid(parsed.expr(), judge, true)
}

/// Whether the annotation at `span` denotes `typing.TypeAlias`, resolved
/// through the shared cascade ([TYPEINF-ANNOTATION-RESOLUTION]) so every
/// spelling — `TypeAlias`, `typing.TypeAlias`, `t.TypeAlias`,
/// `from typing import TypeAlias as TA` — collapses to the same answer.
pub(crate) fn annotation_is_type_alias(
    resolver: &AnnotationResolver<'_>,
    span: Option<Span>,
) -> bool {
    span.and_then(|span| resolver.resolve_span(span))
        .is_some_and(|ty| matches!(&ty, InferredType::Named(name) if name.eq_ignore_ascii_case("typealias")))
}

/// Every expression in a parsed module, keyed by its exact source range —
/// the same range the resolver records for annotation and RHS spans. Rules
/// use this to move from a resolver span to the structural node instead of
/// slicing and re-interpreting source text.
pub(crate) struct ExprIndex<'ast> {
    nodes: HashMap<(u32, u32), &'ast Expr>,
}

impl<'ast> ExprIndex<'ast> {
    pub(crate) fn build(ast: &'ast ModModule) -> Self {
        let mut collector = ExprCollector {
            nodes: HashMap::new(),
        };
        for stmt in &ast.body {
            collector.visit_stmt(stmt);
        }
        Self {
            nodes: collector.nodes,
        }
    }

    /// The expression node occupying exactly `span`, if any.
    pub(crate) fn expr(&self, span: Span) -> Option<&'ast Expr> {
        self.nodes.get(&(span.start, span.end)).copied()
    }
}

struct ExprCollector<'ast> {
    nodes: HashMap<(u32, u32), &'ast Expr>,
}

impl<'ast> Visitor<'ast> for ExprCollector<'ast> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        let range = expr.range();
        let _ = self
            .nodes
            .insert((range.start().to_u32(), range.end().to_u32()), expr);
        walk_expr(self, expr);
    }
}
