//! Implements [RESOLV-CANONICAL-BINDING].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-BINDING
//!
//! Binding resolution: the ONLY lawful path from a use-site expression to a
//! specification form.
//!
//! This module answers "what definition does this expression refer to?" from
//! the module's own import statements and bindings, in the AST. It never reads
//! the characters at the use site to decide meaning. That is what makes all
//! three of these correct, where character matching got every one of them
//! wrong:
//!
//! ```python
//! from typing import ClassVar as CV     # CV     -> typing.ClassVar
//! import typing as t; t.ClassVar        # t.X    -> typing.ClassVar
//! class ClassVar: ...                   # local  -> not a specification form
//! ```
//!
//! Scope and order. Only statements that execute in the module's own frame
//! contribute bindings: an import inside a `def` or `class` body binds that
//! scope, never the module's. Bindings are POSITIONAL — a use refers to the
//! latest module-level binding at or before its own offset — so a later
//! rebind does not corrupt earlier uses, and an import placed after a rebind
//! wins for uses after the import.
//!
//! Known gaps, recorded rather than guessed around: assignment expressions
//! (`:=`) inside module-level expressions are not collected, and a star
//! import from a module the registry does not describe contributes no
//! bindings. A module that rebinds a specification name through either still
//! resolves the earlier import for later uses.

use std::collections::HashMap;

use ruff_python_ast::{ExceptHandler, Expr, Pattern, Stmt};
use ruff_text_size::{Ranged, TextSize};

use crate::form::{form_at, CanonicalSymbol, TypingForm};
use crate::registry::registry;

/// The name a star-import binds, as it appears in the AST.
const STAR_IMPORT: &str = "*";

/// One module-level binding of a name, at a statement position.
#[derive(Debug, Clone)]
struct BindingEvent {
    /// Start offset of the binding statement.
    offset: TextSize,
    /// What the name refers to from this point on.
    kind: BindingKind,
}

/// How a driver wants an `if` statement's branches collected.
///
/// A driver with static knowledge of the execution target (a stub parser
/// with a concrete Python version, PEP 484 version checks) selects the one
/// branch that executes; without knowledge, every branch contributes — the
/// conservative union.
#[derive(Debug, Clone, Copy)]
pub enum BranchView<'a> {
    /// No static selection: collect every branch.
    AllBranches,
    /// Exactly this branch's body executes for the driver's target.
    Only(&'a [Stmt]),
    /// No branch executes for the driver's target.
    NoBranch,
}

/// A driver's static branch selection for `if` statements.
pub type BranchFilter<'f> = &'f dyn for<'a> Fn(&'a ruff_python_ast::StmtIf) -> BranchView<'a>;

/// What a binding makes a name refer to.
#[derive(Debug, Clone)]
enum BindingKind {
    /// A definition imported from another module.
    Symbol(CanonicalSymbol),
    /// A module object (`import x`, `import x as y`).
    Module(String),
    /// A local definition or assignment — not an imported symbol.
    LocalDefinition,
}

/// Every name the module scope binds, and what each one refers to where.
///
/// Built once per module from its AST. Lookups are pure functions of the
/// bindings and the use-site offset — no source text is consulted.
#[derive(Debug, Default, Clone)]
pub struct BindingTable {
    /// Name → its binding events, ascending by offset.
    names: HashMap<String, Vec<BindingEvent>>,
}

impl BindingTable {
    /// Build the binding table for a module body.
    #[must_use]
    pub fn from_module(body: &[Stmt]) -> Self {
        Self::from_module_with_branch_filter(body, &|_| BranchView::AllBranches)
    }

    /// Build the binding table with a driver-supplied branch selection.
    ///
    /// Mutually exclusive `if` branches (version/platform guards in stubs)
    /// never execute together; a table flattened over all of them lets an
    /// infeasible branch's binding control resolution in the selected one.
    /// The filter answers, per `if`, which branches the driver's target
    /// actually executes.
    #[must_use]
    pub fn from_module_with_branch_filter(body: &[Stmt], filter: BranchFilter<'_>) -> Self {
        let mut table = Self::default();
        table.collect(body, filter);
        for events in table.names.values_mut() {
            events.sort_by_key(|event| event.offset);
        }
        table
    }

    /// Collect binding events from statements executing in the module frame.
    fn collect(&mut self, body: &[Stmt], filter: BranchFilter<'_>) {
        for stmt in body {
            self.collect_stmt(stmt, filter);
        }
    }

    /// Binding events of one module-frame statement.
    ///
    /// Python creates a binding AFTER evaluating the statement that makes it
    /// (<https://docs.python.org/3/reference/executionmodel.html#binding-of-names>),
    /// so events are timestamped at the statement's END: an assignment's
    /// RHS, a class's bases, and a function's decorators — all inside the
    /// statement's range — resolve to the PRECEDING binding, while any later
    /// use sees the new one. (A PEP 695 `type X = …` alias may lazily
    /// reference itself; that inner use resolves to nothing here, which
    /// every consumer treats as abstention.)
    fn collect_stmt(&mut self, stmt: &Stmt, filter: BranchFilter<'_>) {
        let offset = stmt.range().end();
        match stmt {
            Stmt::Import(import) => self.bind_plain_import(offset, import),
            Stmt::ImportFrom(import) => self.bind_from_import(offset, import),
            Stmt::ClassDef(class) => self.push_local(offset, class.name.as_str()),
            Stmt::FunctionDef(function) => self.push_local(offset, function.name.as_str()),
            Stmt::Assign(assign) => self.bind_each_target(offset, &assign.targets),
            Stmt::AnnAssign(assign) => self.bind_target(offset, &assign.target),
            Stmt::AugAssign(assign) => self.bind_target(offset, &assign.target),
            Stmt::TypeAlias(alias) => self.bind_target(offset, &alias.name),
            Stmt::Delete(delete) => self.bind_each_target(offset, &delete.targets),
            _ => self.collect_compound(stmt, filter),
        }
    }

    /// Descend into compound statements whose bodies run in the module frame.
    ///
    /// `def` and `class` bodies are NOT among them: they execute in their own
    /// scopes, so nothing inside them binds a module-level name.
    fn collect_compound(&mut self, stmt: &Stmt, filter: BranchFilter<'_>) {
        match stmt {
            Stmt::If(node) => match filter(node) {
                BranchView::AllBranches => {
                    self.collect(&node.body, filter);
                    for clause in &node.elif_else_clauses {
                        self.collect(&clause.body, filter);
                    }
                }
                BranchView::Only(body) => self.collect(body, filter),
                BranchView::NoBranch => {}
            },
            Stmt::While(node) => {
                self.collect(&node.body, filter);
                self.collect(&node.orelse, filter);
            }
            Stmt::For(node) => {
                // The target binds once the iterable has evaluated — before
                // the body runs — so it is visible at every body offset.
                self.bind_target(node.iter.range().end(), &node.target);
                self.collect(&node.body, filter);
                self.collect(&node.orelse, filter);
            }
            Stmt::With(node) => self.collect_with(node, filter),
            Stmt::Try(node) => self.collect_try(node, filter),
            Stmt::Match(node) => self.collect_match(node, filter),
            _ => {}
        }
    }

    /// `with … as target:` targets and body. Each target binds once its own
    /// context expression has evaluated, before the body runs.
    fn collect_with(&mut self, node: &ruff_python_ast::StmtWith, filter: BranchFilter<'_>) {
        for item in &node.items {
            if let Some(target) = &item.optional_vars {
                self.bind_target(item.context_expr.range().end(), target);
            }
        }
        self.collect(&node.body, filter);
    }

    /// `try` body, handlers (and their `as` names), `else`, `finally`.
    fn collect_try(&mut self, node: &ruff_python_ast::StmtTry, filter: BranchFilter<'_>) {
        self.collect(&node.body, filter);
        for handler in &node.handlers {
            let ExceptHandler::ExceptHandler(handler) = handler;
            if let Some(name) = &handler.name {
                self.push_local(name.range().start(), name.as_str());
            }
            self.collect(&handler.body, filter);
        }
        self.collect(&node.orelse, filter);
        self.collect(&node.finalbody, filter);
    }

    /// `match` case bodies and the names their patterns capture.
    fn collect_match(&mut self, node: &ruff_python_ast::StmtMatch, filter: BranchFilter<'_>) {
        for case in &node.cases {
            let offset = case.pattern.range().start();
            self.bind_pattern(offset, &case.pattern);
            self.collect(&case.body, filter);
        }
    }

    /// The names a match pattern captures.
    fn bind_pattern(&mut self, offset: TextSize, pattern: &Pattern) {
        match pattern {
            Pattern::MatchValue(_) | Pattern::MatchSingleton(_) => {}
            Pattern::MatchSequence(node) => self.bind_patterns(offset, &node.patterns),
            Pattern::MatchMapping(node) => self.bind_match_mapping(offset, node),
            Pattern::MatchClass(node) => self.bind_match_class(offset, node),
            Pattern::MatchStar(node) => {
                if let Some(name) = &node.name {
                    self.push_local(offset, name.as_str());
                }
            }
            Pattern::MatchAs(node) => self.bind_match_as(offset, node),
            Pattern::MatchOr(node) => self.bind_patterns(offset, &node.patterns),
        }
    }

    /// Each pattern in a sequence.
    fn bind_patterns(&mut self, offset: TextSize, patterns: &[Pattern]) {
        for pattern in patterns {
            self.bind_pattern(offset, pattern);
        }
    }

    /// `case {…, **rest}:` — value sub-patterns and the rest capture.
    fn bind_match_mapping(&mut self, offset: TextSize, node: &ruff_python_ast::PatternMatchMapping) {
        self.bind_patterns(offset, &node.patterns);
        if let Some(rest) = &node.rest {
            self.push_local(offset, rest.as_str());
        }
    }

    /// `case Point(x=px):` — positional and keyword sub-patterns.
    fn bind_match_class(&mut self, offset: TextSize, node: &ruff_python_ast::PatternMatchClass) {
        self.bind_patterns(offset, &node.arguments.patterns);
        for keyword in &node.arguments.keywords {
            self.bind_pattern(offset, &keyword.pattern);
        }
    }

    /// `case … as name:` — the inner pattern and the capture name.
    fn bind_match_as(&mut self, offset: TextSize, node: &ruff_python_ast::PatternMatchAs) {
        if let Some(pattern) = &node.pattern {
            self.bind_pattern(offset, pattern);
        }
        if let Some(name) = &node.name {
            self.push_local(offset, name.as_str());
        }
    }

    /// `import X`, `import X.Y`, `import X as Z`.
    fn bind_plain_import(&mut self, offset: TextSize, import: &ruff_python_ast::StmtImport) {
        for alias in &import.names {
            let module = alias.name.as_str();
            // `import X.Y as Z` binds Z to the submodule X.Y; a plain
            // `import X.Y` binds only the top-level package name X.
            let (local, target) = alias.asname.as_ref().map_or_else(
                || {
                    let top = module.split('.').next().unwrap_or(module);
                    (top.to_owned(), top.to_owned())
                },
                |asname| (asname.to_string(), module.to_owned()),
            );
            self.push_event(local, offset, BindingKind::Module(target));
        }
    }

    /// `from X import A`, `from X import A as B`, `from X import *`.
    ///
    /// A relative import cannot reach a specification module, but it still
    /// BINDS its local names, so each becomes a local-definition event.
    fn bind_from_import(&mut self, offset: TextSize, import: &ruff_python_ast::StmtImportFrom) {
        let module = (import.level == 0)
            .then(|| import.module.as_ref())
            .flatten()
            .map(ruff_python_ast::Identifier::as_str);
        for alias in &import.names {
            let name = alias.name.as_str();
            if name == STAR_IMPORT {
                if let Some(module) = module {
                    self.bind_star_import(offset, module);
                }
                continue;
            }
            let local = alias
                .asname
                .as_ref()
                .map_or(name, ruff_python_ast::Identifier::as_str);
            let kind = module.map_or(BindingKind::LocalDefinition, |module| {
                BindingKind::Symbol(CanonicalSymbol::new(module, name))
            });
            self.push_event(local.to_owned(), offset, kind);
        }
    }

    /// `from M import *` for a registry module binds every specification name
    /// M defines. A module outside the registry contributes nothing — see the
    /// module docs.
    fn bind_star_import(&mut self, offset: TextSize, module: &str) {
        let Some(names) = registry().get(module) else {
            return;
        };
        for name in names.keys() {
            let symbol = CanonicalSymbol::new(module, name.clone());
            self.push_event(name.clone(), offset, BindingKind::Symbol(symbol));
        }
    }

    /// Record binding events for an assignment-like target expression.
    fn bind_target(&mut self, offset: TextSize, target: &Expr) {
        match target {
            Expr::Name(name) => self.push_local(offset, name.id.as_str()),
            Expr::Tuple(tuple) => self.bind_each_target(offset, &tuple.elts),
            Expr::List(list) => self.bind_each_target(offset, &list.elts),
            Expr::Starred(starred) => self.bind_target(offset, &starred.value),
            // Attribute and subscript targets bind no module-level name.
            _ => {}
        }
    }

    /// Record binding events for each target in a list.
    fn bind_each_target(&mut self, offset: TextSize, targets: &[Expr]) {
        for target in targets {
            self.bind_target(offset, target);
        }
    }

    /// Append one binding event for `name`.
    fn push_event(&mut self, name: String, offset: TextSize, kind: BindingKind) {
        self.names
            .entry(name)
            .or_default()
            .push(BindingEvent { offset, kind });
    }

    /// Append a local-definition event for `name`.
    fn push_local(&mut self, offset: TextSize, name: &str) {
        self.push_event(name.to_owned(), offset, BindingKind::LocalDefinition);
    }

    /// The binding governing a use of `name` at `offset`: the latest event at
    /// or before it.
    fn binding_at(&self, name: &str, offset: TextSize) -> Option<&BindingKind> {
        let events = self.names.get(name)?;
        events
            .iter()
            .rev()
            .find(|event| event.offset <= offset)
            .map(|event| &event.kind)
    }

    /// Whether the module scope binds this name anywhere, by import or by
    /// definition.
    ///
    /// The question a builtin recognition must ask first. `staticmethod` is a
    /// builtin only while the module has not rebound the name — after
    /// `from x import staticmethod`, `import staticmethod`, or a local
    /// `def staticmethod`, the name refers to that binding and nothing may
    /// assume otherwise. Existential on purpose: a rebinding anywhere in the
    /// module suppresses builtin recognition conservatively.
    #[must_use]
    pub fn binds_name(&self, name: &str) -> bool {
        self.names.contains_key(name)
    }

    /// The definition an expression refers to.
    ///
    /// Unwraps the forms a specification symbol is used through: subscripting
    /// (`Final[int]`), calling (`TypeVar("T")`), and module attribute access
    /// (`t.ClassVar`). Resolution is positional: the expression refers to the
    /// latest module-level binding at or before its own offset.
    #[must_use]
    pub fn canonical_of(&self, expr: &Expr) -> Option<CanonicalSymbol> {
        match expr {
            Expr::Name(name) => {
                match self.binding_at(name.id.as_str(), name.range().start())? {
                    BindingKind::Symbol(symbol) => Some(symbol.clone()),
                    BindingKind::Module(_) | BindingKind::LocalDefinition => None,
                }
            }
            Expr::Attribute(attribute) => {
                let module = self.module_path_of(&attribute.value)?;
                Some(CanonicalSymbol::new(module, attribute.attr.as_str()))
            }
            Expr::Subscript(subscript) => self.canonical_of(&subscript.value),
            Expr::Call(call) => self.canonical_of(&call.func),
            _ => None,
        }
    }

    /// The module an expression refers to, following dotted access.
    ///
    /// `import collections.abc` binds `collections`; `collections.abc` then
    /// resolves to the submodule so `collections.abc.Callable` resolves.
    fn module_path_of(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Name(name) => {
                match self.binding_at(name.id.as_str(), name.range().start())? {
                    BindingKind::Module(module) => Some(module.clone()),
                    BindingKind::Symbol(_) | BindingKind::LocalDefinition => None,
                }
            }
            Expr::Attribute(attribute) => {
                let base = self.module_path_of(&attribute.value)?;
                Some(format!("{base}.{}", attribute.attr))
            }
            _ => None,
        }
    }

    /// The specification form an expression denotes, if any.
    ///
    /// This is the single entry point rules use. It cannot be called with a
    /// spelling: the question is an expression, and the answer is a form.
    #[must_use]
    pub fn form_of(&self, expr: &Expr) -> Option<TypingForm> {
        form_at(&self.canonical_of(expr)?)
    }

    /// The specification form an expression denotes, extended to the builtin
    /// scope: a bare name the module never rebinds resolves to the form its
    /// `builtins` definition carries.
    ///
    /// This is how `@staticmethod` is recognised without an import, while
    /// `from builtins import staticmethod as sm` resolves by its binding and
    /// a module-level `def staticmethod(…)` stops both.
    ///
    /// The rebind test is positional, matching Python's sequential module
    /// execution: a use *before* the module's first rebinding of the name
    /// still refers to the builtin; only a binding at or before the use site
    /// suppresses it.
    #[must_use]
    pub fn form_of_with_builtins(&self, expr: &Expr) -> Option<TypingForm> {
        if let Some(form) = self.form_of(expr) {
            return Some(form);
        }
        let Expr::Name(name) = expr else {
            return None;
        };
        if self
            .binding_at(name.id.as_str(), name.range().start())
            .is_some()
        {
            return None;
        }
        crate::form::builtin_form_of_name(name.id.as_str())
    }

    /// Whether an expression denotes exactly `form`.
    #[must_use]
    pub fn is_form(&self, expr: &Expr, form: TypingForm) -> bool {
        self.form_of(expr) == Some(form)
    }

    /// Whether an expression resolves to `name` defined in `module`.
    ///
    /// The lawful recognition for a specific symbol outside the typing
    /// registry (`sys.version_info`, `sys.platform`): the use-site expression
    /// is resolved through the module's bindings and its canonical identity
    /// compared — so aliased imports match and rebound names never do.
    #[must_use]
    pub fn resolves_to(&self, expr: &Expr, module: &str, name: &str) -> bool {
        self.canonical_of(expr)
            .is_some_and(|symbol| symbol.module == module && symbol.name == name)
    }

    /// The specification form a QUOTED annotation denotes.
    ///
    /// PEP 484 forward references: a string annotation contains a type
    /// expression evaluated lazily, after the module has executed
    /// (<https://peps.python.org/pep-0484/#forward-references>). The
    /// contents are parsed with `ruff_python_parser` — never inspected as
    /// text — and resolved against each name's FINAL binding, the namespace
    /// a deferred evaluation sees. The parsed expression's own offsets are
    /// relative to the string and never consulted.
    #[must_use]
    pub fn form_of_quoted_annotation(&self, source: &str) -> Option<TypingForm> {
        let parsed = ruff_python_parser::parse_expression(source).ok()?;
        self.form_of_final(&parsed.into_syntax().body)
    }

    /// Whether `form` appears anywhere within a quoted annotation's type
    /// expression — through subscripts, unions, and tuples, mirroring the
    /// composition forms an item type is built from (`"Required[ReadOnly[int]]"`,
    /// PEP 705).
    #[must_use]
    pub fn quoted_annotation_mentions(&self, source: &str, form: TypingForm) -> bool {
        let Ok(parsed) = ruff_python_parser::parse_expression(source) else {
            return false;
        };
        self.expr_mentions_final(&parsed.into_syntax().body, form)
    }

    /// [`Self::quoted_annotation_mentions`]'s walk over a parsed expression.
    fn expr_mentions_final(&self, expr: &Expr, form: TypingForm) -> bool {
        if self.form_of_final(expr) == Some(form) {
            return true;
        }
        match expr {
            Expr::Subscript(sub) => self.expr_mentions_final(&sub.slice, form),
            Expr::BinOp(bin) => {
                self.expr_mentions_final(&bin.left, form)
                    || self.expr_mentions_final(&bin.right, form)
            }
            Expr::Tuple(tuple) => tuple
                .elts
                .iter()
                .any(|element| self.expr_mentions_final(element, form)),
            _ => false,
        }
    }

    /// [`Self::form_of`] against the module's FINAL namespace, with the
    /// builtin fallback: the resolution a lazily evaluated forward
    /// reference sees.
    fn form_of_final(&self, expr: &Expr) -> Option<TypingForm> {
        if let Some(form) = self.canonical_of_final(expr).and_then(|s| form_at(&s)) {
            return Some(form);
        }
        let Expr::Name(name) = expr else {
            return None;
        };
        if self.names.contains_key(name.id.as_str()) {
            return None;
        }
        crate::form::builtin_form_of_name(name.id.as_str())
    }

    /// [`Self::canonical_of`] using each name's LAST binding.
    fn canonical_of_final(&self, expr: &Expr) -> Option<CanonicalSymbol> {
        match expr {
            Expr::Name(name) => match self.last_binding(name.id.as_str())? {
                BindingKind::Symbol(symbol) => Some(symbol.clone()),
                BindingKind::Module(_) | BindingKind::LocalDefinition => None,
            },
            Expr::Attribute(attribute) => {
                let module = self.module_path_of_final(&attribute.value)?;
                Some(CanonicalSymbol::new(module, attribute.attr.as_str()))
            }
            Expr::Subscript(subscript) => self.canonical_of_final(&subscript.value),
            Expr::Call(call) => self.canonical_of_final(&call.func),
            _ => None,
        }
    }

    /// [`Self::module_path_of`] using each name's LAST binding.
    fn module_path_of_final(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Name(name) => match self.last_binding(name.id.as_str())? {
                BindingKind::Module(module) => Some(module.clone()),
                BindingKind::Symbol(_) | BindingKind::LocalDefinition => None,
            },
            Expr::Attribute(attribute) => {
                let base = self.module_path_of_final(&attribute.value)?;
                Some(format!("{base}.{}", attribute.attr))
            }
            _ => None,
        }
    }

    /// The final binding of a name — what the name refers to once the whole
    /// module has executed.
    fn last_binding(&self, name: &str) -> Option<&BindingKind> {
        self.names
            .get(name)
            .and_then(|events| events.last())
            .map(|event| &event.kind)
    }

    /// Whether a bare-name use refers to a module-level local definition —
    /// a `def`, `class`, or assignment this module makes — rather than an
    /// import or the builtin scope.
    ///
    /// Positional: the latest binding at or before the use site decides, so
    /// a class name later rebound by an import stops referring to the class
    /// from the rebinding onward.
    #[must_use]
    pub fn refers_to_local_definition(&self, expr: &Expr) -> bool {
        let Expr::Name(name) = expr else {
            return false;
        };
        matches!(
            self.binding_at(name.id.as_str(), name.range().start()),
            Some(BindingKind::LocalDefinition)
        )
    }

    /// The element expression of a subscript whose base denotes `form`.
    ///
    /// `Final[int]` with [`TypingForm::FinalQualifier`] yields `int`.
    #[must_use]
    pub fn subscript_element<'a>(&self, expr: &'a Expr, form: TypingForm) -> Option<&'a Expr> {
        let Expr::Subscript(subscript) = expr else {
            return None;
        };
        self.is_form(&subscript.value, form)
            .then_some(subscript.slice.as_ref())
    }
}
