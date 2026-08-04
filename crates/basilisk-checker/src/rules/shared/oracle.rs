//! Implements [NARROWPLAN-INTEGRATION] / [TYPEINF-TARGET-BIDIRECTIONAL].
//! See docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-INTEGRATION
//!
//! The per-module TYPE ORACLE: one [`BidirEngine`] seeded from the module's
//! own definitions, answering "what type is the expression at this span?"
//! (synthesis) and "does that expression check against this expected type?"
//! (bidirectional checking) for every rule that used to shape-match `RhsKind`
//! or re-parse annotation text.
//!
//! Seeding is deliberately enforcement-grade, not display-grade
//! ([TYPEINF-TARGET-GRADUAL]): a function contributes a `Callable` only when
//! its return is DECLARED (a synthesized return may be displayed in hover, but
//! enforcing one would let removing an annotation add errors, breaking the
//! gradual guarantee), an `async def` or decorated function contributes
//! nothing (the call-result transform is not modelled), and every unresolved
//! annotation is `Unknown`, which no judgment turns into a diagnostic.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::visitor::{walk_body, walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{Expr, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::annotation::AnnotationResolver;
use crate::bidir::{BidirEngine, Ty};
use crate::types::InferredType;

use super::parse_module;

/// One module's bidirectional-inference oracle: an engine whose outermost
/// scope holds the module's functions and classes, an index of every
/// expression by source range, and the lexical function scopes whose
/// parameter bindings overlay the globals for spans inside them.
pub(crate) struct ModuleOracle<'m> {
    engine: RefCell<BidirEngine>,
    /// Every expression in the module, keyed by its exact source range — the
    /// same range the resolver records for an assignment RHS or call argument.
    expressions: HashMap<(u32, u32), &'m Expr>,
    /// Function scopes in outer-before-inner walk order.
    scopes: Vec<FunctionScope>,
    /// Module-level class names: a bare-name reference to one is a CLASS
    /// OBJECT, not an instance, and the engine's Stage-2 class/instance
    /// conflation must not let it masquerade as one.
    class_names: HashSet<String>,
}

/// One function's lexical scope: the range it spans and the parameter
/// bindings visible inside it.
struct FunctionScope {
    range: Span,
    bindings: HashMap<String, Ty>,
}

impl<'m> ModuleOracle<'m> {
    /// Build the oracle for `module`, resolving every annotation through the
    /// shared cascade. `None` when the module does not parse — the parse error
    /// is reported separately and every query then abstains.
    pub(crate) fn build(
        module: &'m ResolvedModule,
        resolver: &AnnotationResolver<'m>,
    ) -> Option<Self> {
        let parsed = parse_module(module)?;
        let mut collector = Collector {
            resolver,
            expressions: HashMap::new(),
            scopes: Vec::new(),
            globals: HashMap::new(),
            class_attributes: HashMap::new(),
            class_names: HashSet::new(),
            class_stack: Vec::new(),
        };
        collector.collect_globals(&parsed.ast.body);
        walk_body(&mut collector, &parsed.ast.body);
        let mut engine = BidirEngine::new(collector.globals);
        engine.set_class_attributes(collector.class_attributes);
        Some(Self {
            engine: RefCell::new(engine),
            expressions: collector.expressions,
            scopes: collector.scopes,
            class_names: collector.class_names,
        })
    }

    /// The expression node occupying exactly `span`, if any.
    pub(crate) fn expr(&self, span: Span) -> Option<&'m Expr> {
        self.expressions.get(&(span.start, span.end)).copied()
    }

    /// Synthesize the type of the expression at `span`, seen from its own
    /// lexical scope. `None` when no expression occupies the span; a bare
    /// name that denotes a module class answers `None` too — the value is the
    /// class OBJECT, which the engine's instance-conflating `Named` cannot
    /// represent without inventing errors on `x: type[C] = C`.
    pub(crate) fn synth_span(&self, span: Span) -> Option<InferredType> {
        let expr = self.expr(span)?;
        if let Expr::Name(name) = expr {
            if self.class_names.contains(name.id.as_str()) {
                return None;
            }
        }
        let mut engine = self.engine.borrow_mut();
        let depth = self.push_overlays(&mut engine, span.start);
        let ty = engine.synth(expr);
        let solution = engine.solve_expression();
        pop_overlays(&mut engine, depth);
        Some(ty.to_inferred(&solution.vars))
    }

    /// Check the expression at `span` against `expected` in check mode —
    /// expected types thread INTO displays, so `d: dict[str, str] = {"k": x}`
    /// judges `x` against `str` instead of rejecting under dict invariance.
    /// `Some(true)` means every recorded obligation held; `None` abstains.
    pub(crate) fn checks_span(&self, span: Span, expected: &InferredType) -> Option<bool> {
        let expr = self.expr(span)?;
        let mut engine = self.engine.borrow_mut();
        let depth = self.push_overlays(&mut engine, span.start);
        engine.check(expr, &Ty::from_inferred(expected));
        let solution = engine.solve_expression();
        pop_overlays(&mut engine, depth);
        Some(solution.errors.is_empty())
    }

    /// Push every function scope containing `offset`, outermost first, and
    /// return how many were pushed.
    fn push_overlays(&self, engine: &mut BidirEngine, offset: u32) -> usize {
        let containing = self
            .scopes
            .iter()
            .filter(|scope| scope.range.contains_offset(offset));
        let mut depth = 0;
        for scope in containing {
            engine.push_scope_with(scope.bindings.clone());
            depth += 1;
        }
        depth
    }
}

/// Pop `depth` overlay scopes pushed by [`ModuleOracle::push_overlays`].
fn pop_overlays(engine: &mut BidirEngine, depth: usize) {
    for _ in 0..depth {
        engine.pop_scope();
    }
}

/// Walks the module once: indexes every expression, records function scopes
/// with their parameter bindings, and gathers module-level globals.
struct Collector<'m, 'r> {
    resolver: &'r AnnotationResolver<'m>,
    expressions: HashMap<(u32, u32), &'m Expr>,
    scopes: Vec<FunctionScope>,
    globals: HashMap<String, Ty>,
    class_attributes: HashMap<String, HashMap<String, InferredType>>,
    class_names: HashSet<String>,
    class_stack: Vec<String>,
}

impl<'m> Collector<'m, '_> {
    /// Bind module-level `def`s and `class`es into the engine's global scope.
    fn collect_globals(&mut self, body: &'m [Stmt]) {
        for stmt in body {
            match stmt {
                Stmt::FunctionDef(def) => self.bind_function(def),
                Stmt::ClassDef(def) => self.bind_class(def),
                _ => {}
            }
        }
    }

    /// A module function becomes a `Callable` global — but only an
    /// undecorated, non-async one with a DECLARED return. A decorator may
    /// transform the callable and `async def` wraps its result in a
    /// coroutine; neither transform is modelled, and an inferred (undeclared)
    /// return must never be enforced ([TYPEINF-TARGET-GRADUAL]).
    fn bind_function(&mut self, def: &'m StmtFunctionDef) {
        if def.is_async || !def.decorator_list.is_empty() {
            return;
        }
        let Some(returns) = def.returns.as_deref() else {
            return;
        };
        let ret = Ty::from_inferred(&self.resolver.resolve(returns));
        let params = self.positional_param_tys(def);
        let _ = self
            .globals
            .insert(def.name.to_string(), Ty::Callable(params, Box::new(ret)));
    }

    /// The declared types of the function's positional parameters, in call
    /// order — exactly the positions `synth_call` zips arguments against.
    fn positional_param_tys(&self, def: &StmtFunctionDef) -> Vec<Ty> {
        def.parameters
            .posonlyargs
            .iter()
            .chain(def.parameters.args.iter())
            .map(|param| {
                param
                    .parameter
                    .annotation
                    .as_deref()
                    .map_or_else(Ty::unknown, |annotation| {
                        Ty::from_inferred(&self.resolver.resolve(annotation))
                    })
            })
            .collect()
    }

    /// A module class becomes a `Named` global (its constructor yields an
    /// instance through the engine's `Named`-callee rule) plus an attribute
    /// schema from its body's annotated assignments. Names keep their real
    /// case — the annotation cascade preserves class case, and the two sides
    /// must agree for `x: C = C()` to hold.
    fn bind_class(&mut self, def: &'m StmtClassDef) {
        let name = def.name.to_string();
        let _ = self
            .globals
            .insert(name.clone(), Ty::Ground(InferredType::Named(name.clone())));
        let _ = self.class_names.insert(name.clone());
        let attributes = self.class_attribute_schema(&def.body);
        if !attributes.is_empty() {
            let _ = self.class_attributes.insert(name, attributes);
        }
    }

    /// Attribute name → declared type for a class body's `x: T` declarations.
    fn class_attribute_schema(&self, body: &'m [Stmt]) -> HashMap<String, InferredType> {
        body.iter()
            .filter_map(|stmt| match stmt {
                Stmt::AnnAssign(assign) => match assign.target.as_ref() {
                    Expr::Name(target) => Some((
                        target.id.to_string(),
                        self.resolver.resolve(&assign.annotation),
                    )),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// Record `def`'s lexical scope — annotated parameters through the
    /// cascade, plus the implicit `self`/`cls` receiver when the function
    /// sits in a class body — then walk the whole definition inside it.
    fn enter_function(&mut self, stmt: &'m Stmt, def: &'m StmtFunctionDef) {
        self.scopes.push(FunctionScope {
            range: Span::from(def.range),
            bindings: self.function_bindings(def),
        });
        walk_stmt(self, stmt);
    }

    /// Parameter name → declared type for every annotated parameter, with the
    /// enclosing class bound to an unannotated leading `self`/`cls`.
    fn function_bindings(&self, def: &StmtFunctionDef) -> HashMap<String, Ty> {
        let mut bindings: HashMap<String, Ty> = def
            .parameters
            .iter_non_variadic_params()
            .filter_map(|param| {
                let annotation = param.parameter.annotation.as_deref()?;
                Some((
                    param.parameter.name.to_string(),
                    Ty::from_inferred(&self.resolver.resolve(annotation)),
                ))
            })
            .collect();
        self.bind_receiver(def, &mut bindings);
        bindings
    }

    /// Bind an unannotated leading `self`/`cls` to the enclosing class: both
    /// denote it through the engine's class/instance conflation, and `cls()`
    /// then synthesizes an instance exactly like `C()` does.
    fn bind_receiver(&self, def: &StmtFunctionDef, bindings: &mut HashMap<String, Ty>) {
        let Some(class_name) = self.class_stack.last() else {
            return;
        };
        let receiver = def
            .parameters
            .posonlyargs
            .iter()
            .chain(def.parameters.args.iter())
            .next();
        let Some(param) = receiver else { return };
        let name = param.parameter.name.as_str();
        if param.parameter.annotation.is_none() && (name == "self" || name == "cls") {
            let _ = bindings.insert(
                name.to_string(),
                Ty::Ground(InferredType::Named(class_name.clone())),
            );
        }
    }
}

impl<'m> Visitor<'m> for Collector<'m, '_> {
    fn visit_stmt(&mut self, stmt: &'m Stmt) {
        match stmt {
            Stmt::FunctionDef(def) => self.enter_function(stmt, def),
            Stmt::ClassDef(def) => {
                self.class_stack.push(def.name.to_string());
                walk_stmt(self, stmt);
                let _ = self.class_stack.pop();
            }
            other => walk_stmt(self, other),
        }
    }

    fn visit_expr(&mut self, expr: &'m Expr) {
        let range = expr.range();
        let _ = self
            .expressions
            .insert((range.start().to_u32(), range.end().to_u32()), expr);
        walk_expr(self, expr);
    }
}
