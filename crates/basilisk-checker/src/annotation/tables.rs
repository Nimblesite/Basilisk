//! Implements [TYPEINF-ANNOTATION-RESOLUTION] — the name tables the cascade
//! resolves against. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//!
//! Every table is built from the module's Ruff AST, never from source text:
//! aliases keep a borrowed reference to their right-hand-side **expression**
//! so the cascade expands them by evaluating a type expression, and imports
//! keep the defining module plus the name as spelled there, so an aliased
//! `from`-import and an aliased module import record the same facts.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, ModModule, Stmt, StmtClassDef};

/// One alias definition reachable from a type expression: `type X[P..] = rhs`
/// or the implicit `X = <type expression>`.
#[derive(Debug)]
pub(super) struct AliasEntry<'m> {
    /// PEP 695 type-parameter names, in declaration order (empty otherwise).
    pub(super) params: Vec<String>,
    /// The right-hand side, as an AST expression.
    pub(super) value: &'m Expr,
}

/// A name bound into this module by an `import` statement.
#[derive(Debug)]
pub(super) struct ImportedName {
    /// The defining module's dotted path (`typing`, `collections.abc`).
    pub(super) module: String,
    /// The name as spelled in the defining module — alias-independent, so
    /// `from m import A as B` records `A`.
    pub(super) original: String,
}

/// The resolution tables for one module.
#[derive(Debug, Default)]
pub(super) struct Tables<'m> {
    /// Alias name → definition.
    pub(super) aliases: HashMap<String, AliasEntry<'m>>,
    /// Same-file classes, by declared name.
    pub(super) nominal: HashSet<String>,
    /// Names bound by `from X import name`.
    pub(super) imports: HashMap<String, ImportedName>,
    /// Local binding → module path, for `import X` / `import X as Y`.
    pub(super) modules: HashMap<String, String>,
}

impl<'m> Tables<'m> {
    /// Build every table from one module AST.
    pub(super) fn build(module: &'m ModModule) -> Self {
        let mut tables = Tables::default();
        tables.collect(&module.body);
        tables.collect_implicit_aliases(&module.body);
        tables
    }

    /// Walk every statement body: aliases, classes, and imports are collected
    /// at any nesting depth, because a type expression may name a symbol
    /// declared inside a conditional (`if TYPE_CHECKING:`) or a class body.
    fn collect(&mut self, body: &'m [Stmt]) {
        for stmt in body {
            self.collect_one(stmt);
            for nested in child_bodies(stmt) {
                self.collect(nested);
            }
        }
    }

    /// The explicit declarations of one statement.
    fn collect_one(&mut self, stmt: &'m Stmt) {
        match stmt {
            Stmt::TypeAlias(alias) => self.insert_type_statement(alias),
            Stmt::ClassDef(class) => self.insert_class(class),
            Stmt::Import(import) => self.insert_plain_imports(import),
            Stmt::ImportFrom(import) => self.insert_from_imports(import),
            _ => {}
        }
    }

    /// PEP 695 `type X[P..] = rhs`.
    fn insert_type_statement(&mut self, alias: &'m ruff_python_ast::StmtTypeAlias) {
        let Some(name) = simple_name(&alias.name) else {
            return;
        };
        let params = alias
            .type_params
            .as_deref()
            .map(|type_params| {
                type_params
                    .type_params
                    .iter()
                    .map(|param| param.name().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let _ = self.aliases.insert(
            name,
            AliasEntry {
                params,
                value: &alias.value,
            },
        );
    }

    // ######################################################################
    // # DELETED BODY — `insert_class`. THIS IS THE ROOT OF THE NOMINAL-LEAF #
    // # DEFECT. DO NOT RESTORE IT AND DO NOT INSERT AN EMPTY TABLE.         #
    // #                                                                     #
    // #   self.nominal.insert(class.name.to_string())                       #
    // #                                                                     #
    // # A `class` STATEMENT — a definition with a unique site — REDUCED TO  #
    // # ITS NAME AND PUT IN A `HashSet<String>`. This one line is where the #
    // # checker throws class identity away, and every downstream defect is  #
    // # a consequence of it, not a separate bug:                            #
    // #                                                                     #
    // #   * `AnnotationResolver::name` answers "is this a class?" by        #
    // #     `nominal.contains(spelling)`, then returns                      #
    // #     `InferredType::Named(spelling)` — so the TYPE ITSELF is a       #
    // #     rendering from birth;                                           #
    // #   * two classes spelled alike in one module collapse to ONE set     #
    // #     entry and therefore one type;                                   #
    // #   * a class reached under an alias is not in the set at all and     #
    // #     resolves to `Unknown`;                                          #
    // #   * `import typing as tp; class Sequence: ...` makes `tp.Sequence`  #
    // #     resolve to the LOCAL class, because `attribute` reduces the     #
    // #     dotted name to its member spelling and asks this same set.      #
    // #                                                                     #
    // # Everything that panics downstream — `is_structural_target`,         #
    // # `skip_names`'s TypedDict sets, `types.rs`'s `(Named, Named)`        #
    // # comparison, `nominal.rs::definition_site` — panics because it is    #
    // # handed the spelling this line produced. Fixing them individually is #
    // # impossible; they are one defect with one cause, and it is here.     #
    // #                                                                     #
    // # The rebuild: `nominal` becomes a map from the class's DEFINITION    #
    // # SITE (`StmtClassDef::name.range()`, matching                        #
    // # `ClassInfo::name_span`) to its declaration, and the leaf carries    #
    // # that `Span`. The name survives only as diagnostic MESSAGE text.     #
    // # `BindingTable::local_class_definition` already resolves a use site  #
    // # to exactly that span — the resolver's `TypedDict` schemas were      #
    // # rebuilt on it and are the working precedent.                        #
    // #                                                                     #
    // # Pinned by: tests/nominal_leaf_identity_tests.rs                     #
    // #            tests/nominal_spelling_surgery_pin_tests.rs              #
    // ######################################################################

    /// DELETED — panics; see the banner above.
    #[expect(
        clippy::panic,
        reason = "mandatory under CLAUDE.md: this body was deleted for reducing a class \
                  DEFINITION to its spelling, and no verdict may come from the name set it \
                  built until the leaf carries a definition site"
    )]
    fn insert_class(&mut self, _class: &'m StmtClassDef) {
        panic!(
            "basilisk-checker: `annotation::tables::insert_class` was DELETED because it \
             reduced a `class` STATEMENT to `class.name.to_string()` and stored it in a \
             `HashSet<String>`, making every nominal type a RENDERING from the moment it \
             was created. Two classes spelled alike collapsed into one type, a class \
             reached under an alias resolved to nothing, and `tp.Sequence` resolved to a \
             local class spelled `Sequence`. It panics because the real implementation — \
             `nominal` keyed by the class's definition site, and `InferredType`'s nominal \
             leaf carrying that site — DOES NOT EXIST YET. Do not restore the name set and \
             do not leave the table empty in its place."
        )
    }

    /// `import X`, `import X.Y`, `import X as Y`.
    fn insert_plain_imports(&mut self, import: &'m ruff_python_ast::StmtImport) {
        for alias in &import.names {
            let module = alias.name.to_string();
            let bound = alias
                .asname
                .as_ref()
                .map_or_else(|| top_level_module(&module), ToString::to_string);
            let _ = self.modules.insert(bound, module);
        }
    }

    /// `from X import A`, `from X import A as B`.
    fn insert_from_imports(&mut self, import: &'m ruff_python_ast::StmtImportFrom) {
        let Some(module) = import.module.as_ref().map(ToString::to_string) else {
            return;
        };
        for alias in &import.names {
            let original = alias.name.to_string();
            let bound = alias
                .asname
                .as_ref()
                .map_or_else(|| original.clone(), ToString::to_string);
            let _ = self.imports.insert(
                bound,
                ImportedName {
                    module: module.clone(),
                    original,
                },
            );
        }
    }

    /// Implicit aliases (`X = int`, `MyList = list[int]`) — a second pass, so
    /// an alias may name a class or alias declared later in the file
    /// (use-before-declaration is legal for type expressions).
    fn collect_implicit_aliases(&mut self, body: &'m [Stmt]) {
        for stmt in body {
            if let Stmt::Assign(assign) = stmt {
                self.insert_implicit_alias(assign);
            }
            for nested in child_bodies(stmt) {
                self.collect_implicit_aliases(nested);
            }
        }
    }

    /// A single-target assignment whose right-hand side is a type expression.
    fn insert_implicit_alias(&mut self, assign: &'m ruff_python_ast::StmtAssign) {
        let [target] = assign.targets.as_slice() else {
            return;
        };
        let Some(name) = simple_name(target) else {
            return;
        };
        if self.aliases.contains_key(&name) || self.nominal.contains(&name) {
            return;
        }
        if self.is_type_expression(&assign.value) {
            let _ = self.aliases.insert(
                name,
                AliasEntry {
                    params: Vec::new(),
                    value: &assign.value,
                },
            );
        }
    }

    /// Is `expr` shaped like a type expression whose head names something this
    /// module can resolve? Deliberately narrow: `X = 5` and `X = f("X")` are
    /// values, not aliases.
    fn is_type_expression(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Name(name) => self.names_a_type(name.id.as_str()),
            Expr::Attribute(_) => dotted_name(expr).is_some(),
            Expr::Subscript(sub) => self.is_type_expression(&sub.value),
            Expr::BinOp(bin) if bin.op == ruff_python_ast::Operator::BitOr => {
                self.is_type_expression(&bin.left) && self.is_type_expression(&bin.right)
            }
            _ => false,
        }
    }

    /// Does a bare name denote a type — a builtin, a same-file class, another
    /// alias, or an imported symbol?
    fn names_a_type(&self, name: &str) -> bool {
        super::builtins::is_builtin_type_name(name)
            || self.nominal.contains(name)
            || self.aliases.contains_key(name)
            || self.imports.contains_key(name)
    }
}

/// Bodies nested inside a compound statement — every scope a declaration may
/// hide in.
fn child_bodies(stmt: &Stmt) -> Vec<&[Stmt]> {
    match stmt {
        Stmt::ClassDef(class) => vec![class.body.as_slice()],
        Stmt::FunctionDef(func) => vec![func.body.as_slice()],
        Stmt::If(if_stmt) => std::iter::once(if_stmt.body.as_slice())
            .chain(
                if_stmt
                    .elif_else_clauses
                    .iter()
                    .map(|clause| clause.body.as_slice()),
            )
            .collect(),
        Stmt::For(for_stmt) => vec![for_stmt.body.as_slice(), for_stmt.orelse.as_slice()],
        Stmt::While(while_stmt) => vec![while_stmt.body.as_slice(), while_stmt.orelse.as_slice()],
        Stmt::With(with_stmt) => vec![with_stmt.body.as_slice()],
        Stmt::Try(try_stmt) => std::iter::once(try_stmt.body.as_slice())
            .chain(try_stmt.handlers.iter().map(
                |ruff_python_ast::ExceptHandler::ExceptHandler(handler)| handler.body.as_slice(),
            ))
            .chain([try_stmt.orelse.as_slice(), try_stmt.finalbody.as_slice()])
            .collect(),
        Stmt::Match(match_stmt) => match_stmt
            .cases
            .iter()
            .map(|case| case.body.as_slice())
            .collect(),
        _ => Vec::new(),
    }
}

/// The top-level component of a dotted module path (`os.path` → `os`).
fn top_level_module(module: &str) -> String {
    module.split('.').next().unwrap_or(module).to_owned()
}

/// The simple name of a `Name` expression.
pub(super) fn simple_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        _ => None,
    }
}

/// The dotted text of a `Name` / `Attribute` chain (`typing.Sequence`).
pub(super) fn dotted_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attr) => Some(format!("{}.{}", dotted_name(&attr.value)?, attr.attr)),
        _ => None,
    }
}
