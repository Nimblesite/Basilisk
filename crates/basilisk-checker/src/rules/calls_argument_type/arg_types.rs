//! Implements [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Type-directed argument resolution for bound built-in method calls.
//!
//! The resolver classifies a call argument by the *syntactic shape* of its
//! expression (`RhsKind`): a name is `Other` whatever it was declared to be,
//! and a display element is `Other` even when its declared type is known. A
//! rule that matches on those shapes cannot tell a valid `[*p]` (`p: list[str]`)
//! from an invalid `[1]`, so it must either reject both or accept both
//! (GitHub #356).
//!
//! This module answers the question the shape cannot: the *type* of the
//! argument expression, resolved through the declared types visible at that
//! point in the module. Anything it cannot resolve is [`InferredType::Unknown`],
//! which every compatibility predicate here accepts — an unresolved expression
//! never manufactures a diagnostic ([CHKARCH-CONFORMANCE-MODE]).

use std::collections::HashMap;

use basilisk_resolver::{iter_all_params, ResolvedModule, Span};
use ruff_python_ast::visitor::{walk_body, walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{Expr, Stmt, StmtAnnAssign, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::rules::shared::{ann_str, parse_module};
use crate::types::{InferredType, LiteralValue};

/// Declared types of names, grouped by the source range of the scope that
/// declares them.
///
/// A lookup is scope-aware: the *innermost* enclosing scope that declares the
/// name wins, so a parameter of one function never supplies the type for a
/// same-named parameter of another.
pub(crate) struct ScopedTypes<'a> {
    scopes: Vec<Scope>,
    /// Every expression in the module, keyed by its exact source range — the
    /// same range the resolver records for a call argument.
    expressions: HashMap<(u32, u32), &'a Expr>,
}

/// One lexical scope: the range it spans and the types it declares.
struct Scope {
    range: Span,
    names: HashMap<String, InferredType>,
}

impl<'a> ScopedTypes<'a> {
    /// Collect every declared type and expression in `module`.
    ///
    /// Yields an empty table when the module does not parse; the parse error is
    /// reported separately, and every lookup then answers `Unknown`.
    pub(crate) fn from_module(module: &'a ResolvedModule) -> Self {
        let module_range = Span::new(0, u32::try_from(module.source.len()).unwrap_or(u32::MAX));
        let mut collector = Collector {
            scopes: vec![Scope {
                range: module_range,
                names: HashMap::new(),
            }],
            expressions: HashMap::new(),
            current: 0,
        };
        if let Some(parsed) = parse_module(module) {
            walk_body(&mut collector, &parsed.ast.body);
        }
        Self {
            scopes: collector.scopes,
            expressions: collector.expressions,
        }
    }

    /// The type of the argument expression occupying `span`.
    ///
    /// A span that names no expression means the module did not parse, so the
    /// index is empty. That answers `Unknown`, which every compatibility
    /// predicate here accepts — the parse error is reported separately and an
    /// unparsed module never manufactures a type diagnostic
    /// ([CHKARCH-CONFORMANCE-MODE]). Previously this fell back to the
    /// resolver's `RhsKind` shape inference, a second inference path condemned
    /// under [TYPEINF-LEGACY]; the shape could only ever be *less* informed
    /// than the expression it was derived from.
    pub(crate) fn argument_type(&self, span: Span) -> InferredType {
        self.expressions
            .get(&(span.start, span.end))
            .map_or(InferredType::Unknown, |expr| self.expr_type(expr))
    }

    /// The declared type of `name` as seen from `offset`, innermost scope first.
    fn lookup(&self, name: &str, offset: u32) -> Option<&InferredType> {
        self.scopes
            .iter()
            .filter(|scope| scope.range.contains_offset(offset))
            .filter_map(|scope| {
                let width = scope.range.end.saturating_sub(scope.range.start);
                scope.names.get(name).map(|ty| (width, ty))
            })
            .min_by_key(|(width, _)| *width)
            .map(|(_, ty)| ty)
    }

    /// The type of an arbitrary expression; `Unknown` when unresolvable.
    fn expr_type(&self, expr: &Expr) -> InferredType {
        match expr {
            Expr::StringLiteral(_) => InferredType::LiteralString,
            // An f-string is a `str` but never a `LiteralString` (PEP 675:
            // interpolations may carry runtime data). A t-string is neither —
            // it builds a `Template`, so it stays unresolved below.
            Expr::FString(_) => InferredType::Str,
            Expr::BytesLiteral(_) => InferredType::Bytes,
            Expr::BooleanLiteral(_) => InferredType::Bool,
            Expr::NoneLiteral(_) => InferredType::None_,
            Expr::NumberLiteral(number) => number_type(&number.value),
            Expr::Name(name) => self
                .lookup(name.id.as_str(), name.range.start().to_u32())
                .cloned()
                .unwrap_or(InferredType::Unknown),
            Expr::List(list) => InferredType::List(Box::new(self.element_type(&list.elts))),
            Expr::Set(set) => InferredType::Set(Box::new(self.element_type(&set.elts))),
            Expr::Tuple(tuple) => InferredType::Tuple(
                tuple
                    .elts
                    .iter()
                    .map(|element| self.unpacked_type(element))
                    .collect(),
            ),
            _ => InferredType::Unknown,
        }
    }

    /// The union of the element types of a list/set display.
    fn element_type(&self, elements: &[Expr]) -> InferredType {
        elements
            .iter()
            .map(|element| self.unpacked_type(element))
            .fold(InferredType::Never, InferredType::union)
    }

    /// The type an element contributes to its display: its own type, or the
    /// type it yields when it is unpacked (`*values`).
    fn unpacked_type(&self, element: &Expr) -> InferredType {
        match element {
            Expr::Starred(starred) => iterated_type(&self.expr_type(&starred.value)),
            other => self.expr_type(other),
        }
    }
}

/// The type produced by iterating `container`; `Unknown` when unknowable.
fn iterated_type(container: &InferredType) -> InferredType {
    match container {
        InferredType::List(element) | InferredType::Set(element) => element.as_ref().clone(),
        InferredType::Dict(key, _) => key.as_ref().clone(),
        InferredType::Tuple(elements) => elements
            .iter()
            .cloned()
            .fold(InferredType::Never, InferredType::union),
        InferredType::Str | InferredType::LiteralString => InferredType::Str,
        InferredType::Generator(yielded, _, _) => yielded.as_ref().clone(),
        _ => InferredType::Unknown,
    }
}

/// The type of a numeric literal.
fn number_type(number: &ruff_python_ast::Number) -> InferredType {
    match number {
        ruff_python_ast::Number::Int(_) => InferredType::Int,
        ruff_python_ast::Number::Float(_) => InferredType::Float,
        ruff_python_ast::Number::Complex { .. } => InferredType::Named("complex".to_owned()),
    }
}

/// Does `argument` satisfy an `Iterable[str]` / `Iterable[LiteralString]`
/// parameter such as `str.join`'s?
///
/// `str` itself qualifies — iterating a string yields strings. Everything this
/// module could not resolve qualifies too; only a positively-known mismatch is
/// rejected.
pub(crate) fn satisfies_str_iterable(argument: &InferredType) -> bool {
    match argument {
        InferredType::Literal(value) => matches!(value, LiteralValue::Str(_)),
        InferredType::Int
        | InferredType::Float
        | InferredType::Bool
        | InferredType::Bytes
        | InferredType::None_ => false,
        InferredType::List(element) | InferredType::Set(element) => may_be_str(element),
        InferredType::Dict(key, _) => may_be_str(key),
        InferredType::Tuple(elements) => elements.iter().all(may_be_str),
        InferredType::Generator(yielded, _, _) => may_be_str(yielded),
        // A union member that qualifies is enough: this check sees declared
        // types, not narrowed ones, so `list[str] | None` inside an
        // `if values is not None:` guard must not be rejected.
        InferredType::Union(members) => members.iter().any(satisfies_str_iterable),
        InferredType::Optional(inner) => satisfies_str_iterable(inner),
        // `str`/`LiteralString` iterate as `Iterable[str]`; everything else
        // reaching here is unresolved (`Unknown`, `Any`, a named class) and is
        // accepted rather than guessed at.
        _ => true,
    }
}

/// Could a value of this type be a `str`? `true` unless it is positively known
/// to be something else.
fn may_be_str(element: &InferredType) -> bool {
    match element {
        InferredType::Int
        | InferredType::Float
        | InferredType::Bool
        | InferredType::Bytes
        | InferredType::None_
        | InferredType::List(_)
        | InferredType::Set(_)
        | InferredType::Dict(_, _)
        | InferredType::Tuple(_) => false,
        InferredType::Literal(value) => matches!(value, LiteralValue::Str(_)),
        InferredType::Union(members) => members.iter().all(may_be_str),
        InferredType::Optional(inner) => may_be_str(inner),
        _ => true,
    }
}

/// Walks the module AST once, recording declared types per scope and indexing
/// every expression by its source range.
struct Collector<'a> {
    scopes: Vec<Scope>,
    expressions: HashMap<(u32, u32), &'a Expr>,
    /// Index into `scopes` of the scope currently being filled.
    current: usize,
}

impl<'a> Collector<'a> {
    /// Open a scope for `function`, seeded with its annotated parameters, and
    /// walk the whole definition inside it.
    fn enter_function(&mut self, stmt: &'a Stmt, function: &'a StmtFunctionDef) {
        self.scopes.push(Scope {
            range: Span::from(function.range),
            names: parameter_types(function),
        });
        let outer = std::mem::replace(&mut self.current, self.scopes.len() - 1);
        walk_stmt(self, stmt);
        self.current = outer;
    }

    /// Index a class body: its nested definitions are walked, but a class-level
    /// `x: T` binds an attribute, not a name its methods can read, so the
    /// annotation is only indexed — never recorded as a scope type.
    fn enter_class_body(&mut self, body: &'a [Stmt]) {
        for nested in body {
            match nested {
                Stmt::AnnAssign(assign) => self.index_annotation(assign),
                other => self.visit_stmt(other),
            }
        }
    }

    /// Index every expression of `x: T = value` without recording the type.
    fn index_annotation(&mut self, assign: &'a StmtAnnAssign) {
        self.visit_expr(&assign.target);
        self.visit_expr(&assign.annotation);
        if let Some(value) = assign.value.as_ref() {
            self.visit_expr(value);
        }
    }

    /// Record `x: T` in the current scope when the target is a plain name.
    fn record_annotation(&mut self, assign: &'a StmtAnnAssign) {
        self.index_annotation(assign);
        let Expr::Name(target) = assign.target.as_ref() else {
            return;
        };
        let declared = InferredType::from_annotation(&ann_str(&assign.annotation));
        if let Some(scope) = self.scopes.get_mut(self.current) {
            let _ = scope.names.insert(target.id.to_string(), declared);
        }
    }
}

impl<'a> Visitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(function) => self.enter_function(stmt, function),
            Stmt::ClassDef(class) => self.enter_class_body(&class.body),
            Stmt::AnnAssign(assign) => self.record_annotation(assign),
            other => walk_stmt(self, other),
        }
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        let range = expr.range();
        let _ = self
            .expressions
            .insert((range.start().to_u32(), range.end().to_u32()), expr);
        walk_expr(self, expr);
    }
}

/// Parameter name → declared type for one function's annotated parameters.
fn parameter_types(function: &StmtFunctionDef) -> HashMap<String, InferredType> {
    iter_all_params(&function.parameters)
        .filter_map(|param| {
            let annotation = param.parameter.annotation.as_ref()?;
            Some((
                param.parameter.name.to_string(),
                InferredType::from_annotation(&ann_str(annotation)),
            ))
        })
        .collect()
}
