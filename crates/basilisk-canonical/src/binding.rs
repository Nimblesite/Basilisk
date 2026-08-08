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

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt};

use crate::form::{form_at, form_in_module, CanonicalSymbol, TypingForm};

/// The name a star-import binds, as it appears in the AST.
const STAR_IMPORT: &str = "*";

/// Every name a module binds, and what each one refers to.
///
/// Built once per module from its AST. Lookups are pure functions of the
/// bindings — no source text is consulted.
#[derive(Debug, Default, Clone)]
pub struct BindingTable {
    /// Local name → the definition it was imported from.
    /// `from typing import ClassVar as CV` → `CV` → `typing.ClassVar`.
    symbols: HashMap<String, CanonicalSymbol>,
    /// Local name → the module it refers to.
    /// `import typing as t` → `t` → `typing`.
    modules: HashMap<String, String>,
    /// Modules star-imported into this one.
    star_modules: Vec<String>,
    /// Module-level names rebound by a definition or assignment. A rebound
    /// name is NOT the imported symbol, however it is spelled.
    shadowed: HashSet<String>,
}

impl BindingTable {
    /// Build the binding table for a module body.
    #[must_use]
    pub fn from_module(body: &[Stmt]) -> Self {
        let mut table = Self::default();
        table.collect_imports(body);
        table.collect_module_level_shadows(body);
        table
    }

    /// Collect every import in the module, at any nesting depth.
    ///
    /// Imports under `if TYPE_CHECKING:` or in a `try`/`except ImportError`
    /// fallback still bind at module level, so the walk descends into compound
    /// statements rather than looking only at the top level.
    fn collect_imports(&mut self, body: &[Stmt]) {
        for stmt in body {
            match stmt {
                Stmt::Import(import) => self.bind_plain_import(import),
                Stmt::ImportFrom(import) => self.bind_from_import(import),
                _ => {}
            }
            for nested in nested_bodies(stmt) {
                self.collect_imports(nested);
            }
        }
    }

    /// `import X`, `import X.Y`, `import X as Z`.
    fn bind_plain_import(&mut self, import: &ruff_python_ast::StmtImport) {
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
            let _ = self.modules.insert(local, target);
        }
    }

    /// `from X import A`, `from X import A as B`, `from X import *`.
    ///
    /// Relative imports are skipped: they cannot reach a specification module.
    fn bind_from_import(&mut self, import: &ruff_python_ast::StmtImportFrom) {
        if import.level > 0 {
            return;
        }
        let Some(module) = import
            .module
            .as_ref()
            .map(ruff_python_ast::Identifier::as_str)
        else {
            return;
        };
        for alias in &import.names {
            let name = alias.name.as_str();
            if name == STAR_IMPORT {
                self.star_modules.push(module.to_owned());
                continue;
            }
            let local = alias
                .asname
                .as_ref()
                .map_or(name, ruff_python_ast::Identifier::as_str);
            let _ = self
                .symbols
                .insert(local.to_owned(), CanonicalSymbol::new(module, name));
        }
    }

    /// Record module-level names bound by something other than an import.
    ///
    /// A module-level `class Protocol:` or `Protocol = object` rebinds the
    /// name, so uses of it are not the specification form.
    fn collect_module_level_shadows(&mut self, body: &[Stmt]) {
        for stmt in body {
            match stmt {
                Stmt::ClassDef(class) => {
                    let _ = self.shadowed.insert(class.name.to_string());
                }
                Stmt::FunctionDef(function) => {
                    let _ = self.shadowed.insert(function.name.to_string());
                }
                Stmt::Assign(assign) => {
                    for target in &assign.targets {
                        self.shadow_if_name(target);
                    }
                }
                Stmt::AnnAssign(assign) => self.shadow_if_name(&assign.target),
                _ => {}
            }
        }
    }

    /// Mark a plain `Name` assignment target as shadowed.
    fn shadow_if_name(&mut self, target: &Expr) {
        if let Expr::Name(name) = target {
            let _ = self.shadowed.insert(name.id.to_string());
        }
    }

    /// The definition a local name refers to, if it refers to an import.
    #[must_use]
    pub fn canonical_of_name(&self, name: &str) -> Option<CanonicalSymbol> {
        if self.shadowed.contains(name) {
            return None;
        }
        if let Some(symbol) = self.symbols.get(name) {
            return Some(symbol.clone());
        }
        // A star-import binds every public name of the module, so a name the
        // registry knows in a star-imported module resolves there.
        self.star_modules
            .iter()
            .find(|module| form_in_module(module, name).is_some())
            .map(|module| CanonicalSymbol::new(module.clone(), name))
    }

    /// Whether the module binds this name itself, by import or by definition.
    ///
    /// The question a builtin recognition must ask first. `staticmethod` is a
    /// builtin only while the module has not rebound the name — after
    /// `from x import staticmethod` or a local `def staticmethod`, the name
    /// refers to that definition and nothing may assume otherwise.
    #[must_use]
    pub fn binds_name(&self, name: &str) -> bool {
        self.shadowed.contains(name) || self.symbols.contains_key(name)
    }

    /// The definition an expression refers to.
    ///
    /// Unwraps the forms a specification symbol is used through: subscripting
    /// (`Final[int]`), calling (`TypeVar("T")`), and module attribute access
    /// (`t.ClassVar`).
    #[must_use]
    pub fn canonical_of(&self, expr: &Expr) -> Option<CanonicalSymbol> {
        match expr {
            Expr::Name(name) => self.canonical_of_name(name.id.as_str()),
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
                let id = name.id.as_str();
                if self.shadowed.contains(id) {
                    return None;
                }
                self.modules.get(id).cloned()
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

    /// The specification form a local name denotes, if any.
    #[must_use]
    pub fn form_of_name(&self, name: &str) -> Option<TypingForm> {
        form_at(&self.canonical_of_name(name)?)
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

/// The statement bodies nested inside a compound statement.
///
/// Used to find imports wherever they appear. Returning borrowed slices keeps
/// the walk allocation-free apart from the outer vector.
fn nested_bodies(stmt: &Stmt) -> Vec<&[Stmt]> {
    match stmt {
        Stmt::If(node) => {
            let mut bodies = vec![node.body.as_slice()];
            bodies.extend(
                node.elif_else_clauses
                    .iter()
                    .map(|clause| clause.body.as_slice()),
            );
            bodies
        }
        Stmt::Try(node) => {
            let mut bodies = vec![
                node.body.as_slice(),
                node.orelse.as_slice(),
                node.finalbody.as_slice(),
            ];
            bodies.extend(node.handlers.iter().map(|handler| {
                let ruff_python_ast::ExceptHandler::ExceptHandler(handler) = handler;
                handler.body.as_slice()
            }));
            bodies
        }
        Stmt::For(node) => vec![node.body.as_slice(), node.orelse.as_slice()],
        Stmt::While(node) => vec![node.body.as_slice(), node.orelse.as_slice()],
        Stmt::With(node) => vec![node.body.as_slice()],
        Stmt::FunctionDef(node) => vec![node.body.as_slice()],
        Stmt::ClassDef(node) => vec![node.body.as_slice()],
        Stmt::Match(node) => node.cases.iter().map(|case| case.body.as_slice()).collect(),
        _ => Vec::new(),
    }
}
