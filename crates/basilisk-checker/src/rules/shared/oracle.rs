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
use ruff_python_ast::{Expr, ExprCall, Stmt, StmtClassDef, StmtFunctionDef};
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
    /// Every `Call` expression in every expression position, in source order
    /// (outer call before its nested calls) — THE call traversal every
    /// call-shaped rule rides ([NARROWPLAN-CALLSITES]), collected by the same
    /// walk that indexes expressions so no rule pays a walk of its own.
    calls: Vec<&'m ExprCall>,
    /// Memoized synthesis per span. Several rules judge the SAME expression
    /// (both return rules share every `return` span; assignment and
    /// redundancy share every RHS; every call argument is seen by more than
    /// one pass), and each un-memoized query pays a scope-overlay clone plus
    /// a solver run — the dominant per-file cost once every rule rides the
    /// engine ([CHKARCH-TESTING-BENCH]).
    synth_cache: RefCell<HashMap<(u32, u32), Option<InferredType>>>,
}

/// One function's lexical scope: the range it spans and the parameter
/// bindings visible inside it.
struct FunctionScope {
    range: Span,
    bindings: std::sync::Arc<HashMap<String, Ty>>,
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
            calls: Vec::new(),
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
            calls: collector.calls,
            synth_cache: RefCell::new(HashMap::new()),
        })
    }

    /// The expression node occupying exactly `span`, if any.
    pub(crate) fn expr(&self, span: Span) -> Option<&'m Expr> {
        self.expressions.get(&(span.start, span.end)).copied()
    }

    /// Every `Call` expression in every expression position, in source order —
    /// the one call traversal ([NARROWPLAN-CALLSITES]).
    pub(crate) fn calls(&self) -> &[&'m ExprCall] {
        &self.calls
    }

    /// Synthesize the type of the expression at `span`, seen from its own
    /// lexical scope. `None` when no expression occupies the span; a bare
    /// name that denotes a module class answers `None` too — the value is the
    /// class OBJECT, which the engine's instance-conflating `Named` cannot
    /// represent without inventing errors on `x: type[C] = C`.
    pub(crate) fn synth_span(&self, span: Span) -> Option<InferredType> {
        let key = (span.start, span.end);
        if let Some(hit) = self.synth_cache.borrow().get(&key) {
            return hit.clone();
        }
        let answer = self.synth_span_uncached(span);
        let _ = self.synth_cache.borrow_mut().insert(key, answer.clone());
        answer
    }

    /// The un-memoized synthesis behind [`ModuleOracle::synth_span`].
    fn synth_span_uncached(&self, span: Span) -> Option<InferredType> {
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
            engine.push_scope_shared(std::sync::Arc::clone(&scope.bindings));
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

/// Mask every name bound by an assignment target — plain names, and names
/// inside tuple/list/starred unpacking.
fn mask_target_names(target: &Expr, bindings: &mut HashMap<String, Ty>) {
    match target {
        Expr::Name(name) => {
            let _ = bindings
                .entry(name.id.to_string())
                .or_insert_with(Ty::unknown);
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                mask_target_names(element, bindings);
            }
        }
        Expr::List(list) => {
            for element in &list.elts {
                mask_target_names(element, bindings);
            }
        }
        Expr::Starred(starred) => mask_target_names(&starred.value, bindings),
        _ => {}
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
    calls: Vec<&'m ExprCall>,
}

impl<'m> Collector<'m, '_> {
    /// Bind module-level `def`s, `class`es and `name: T` declarations into
    /// the engine's global scope.
    fn collect_globals(&mut self, body: &'m [Stmt]) {
        for stmt in body {
            match stmt {
                Stmt::FunctionDef(def) => self.bind_function(def),
                Stmt::ClassDef(def) => self.bind_class(def),
                Stmt::AnnAssign(assign) => self.bind_module_variable(assign),
                _ => {}
            }
        }
    }

    /// A module-level `name: T` declaration binds the name to its DECLARED
    /// type — the annotation is the module's own statement of what the name
    /// holds, so every later reference carries it
    /// ([TYPEINF-ANNOTATION-RESOLUTION]).
    fn bind_module_variable(&mut self, assign: &'m ruff_python_ast::StmtAnnAssign) {
        let Expr::Name(target) = assign.target.as_ref() else {
            return;
        };
        let resolved = self.resolver.resolve(&assign.annotation);
        let _ = self
            .globals
            .insert(target.id.to_string(), Ty::from_inferred(&resolved));
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
            bindings: std::sync::Arc::new(self.function_bindings(def)),
        });
        walk_stmt(self, stmt);
    }

    /// Parameter name → declared type for every annotated parameter, with the
    /// enclosing class bound to an unannotated leading `self`/`cls` — laid
    /// over a mask for every name the body ASSIGNS. A function-local binding
    /// SHADOWS a same-named module global; without the mask, `v1 = …` inside
    /// a function would read the module's `v1: SomeType` and answer with the
    /// wrong symbol's type.
    fn function_bindings(&self, def: &'m StmtFunctionDef) -> HashMap<String, Ty> {
        let mut bindings: HashMap<String, Ty> = HashMap::new();
        self.mask_local_assignments(&def.body, &mut bindings);
        // EVERY parameter shadows — an unannotated one to `Unknown`, never to
        // a same-named module global.
        for param in def.parameters.iter_non_variadic_params() {
            let ty = param
                .parameter
                .annotation
                .as_deref()
                .map_or_else(Ty::unknown, |annotation| {
                    Ty::from_inferred(&self.resolver.resolve(annotation))
                });
            let _ = bindings.insert(param.parameter.name.to_string(), ty);
        }
        for variadic in [
            def.parameters.vararg.as_deref(),
            def.parameters.kwarg.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = bindings.insert(variadic.name.to_string(), Ty::unknown());
        }
        self.bind_receiver(def, &mut bindings);
        bindings
    }

    /// Mask every name `body` assigns: an annotated local carries its
    /// declared type, everything else is `Unknown` — never the module
    /// global it shadows. Nested `def`/`class` bodies are their own scopes
    /// and are not walked.
    fn mask_local_assignments(&self, body: &'m [Stmt], bindings: &mut HashMap<String, Ty>) {
        for stmt in body {
            match stmt {
                Stmt::AnnAssign(assign) => {
                    if let Expr::Name(target) = assign.target.as_ref() {
                        let _ = bindings.insert(
                            target.id.to_string(),
                            Ty::from_inferred(&self.resolver.resolve(&assign.annotation)),
                        );
                    }
                }
                Stmt::Assign(assign) => {
                    for target in &assign.targets {
                        mask_target_names(target, bindings);
                    }
                }
                Stmt::AugAssign(assign) => mask_target_names(&assign.target, bindings),
                Stmt::For(for_stmt) => {
                    mask_target_names(&for_stmt.target, bindings);
                    self.mask_local_assignments(&for_stmt.body, bindings);
                    self.mask_local_assignments(&for_stmt.orelse, bindings);
                }
                Stmt::While(while_stmt) => {
                    self.mask_local_assignments(&while_stmt.body, bindings);
                    self.mask_local_assignments(&while_stmt.orelse, bindings);
                }
                Stmt::If(if_stmt) => {
                    self.mask_local_assignments(&if_stmt.body, bindings);
                    for clause in &if_stmt.elif_else_clauses {
                        self.mask_local_assignments(&clause.body, bindings);
                    }
                }
                Stmt::With(with_stmt) => {
                    for item in &with_stmt.items {
                        if let Some(vars) = item.optional_vars.as_deref() {
                            mask_target_names(vars, bindings);
                        }
                    }
                    self.mask_local_assignments(&with_stmt.body, bindings);
                }
                Stmt::Try(try_stmt) => {
                    self.mask_local_assignments(&try_stmt.body, bindings);
                    self.mask_local_assignments(&try_stmt.orelse, bindings);
                    self.mask_local_assignments(&try_stmt.finalbody, bindings);
                }
                _ => {}
            }
        }
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
        if let Expr::Call(call) = expr {
            self.calls.push(call);
        }
        walk_expr(self, expr);
    }
}
