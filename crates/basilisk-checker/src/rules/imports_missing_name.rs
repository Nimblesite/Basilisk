//! Implements [CHKARCH-DIAG-IMPORT-MEMBER]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-IMPORT-MEMBER
//! `imports_missing_name`: Importing a name the resolved module does not define.
//!
//! `from M import name` only proves that the module path `M` resolves to a
//! file; it says nothing about `name`. When `M` is a workspace `.py` source
//! Basilisk can see every module-level binding, so importing a name that is
//! neither bound in `M`, nor an existing submodule of the package, is an
//! `ImportError` waiting for runtime (GitHub #55).
//!
//! ```python
//! from demo.late_module import provide_value  # late_module.py defines nothing
//! ```
//!
//! The rule is deliberately conservative — silence over guessing:
//!
//! - Every module-level binding form counts as defined: `def`/`class`,
//!   every assignment form, `import`/`from` re-exports, `for`/`with`/`match`/
//!   `except` targets, walrus expressions, and `type` alias statements.
//! - A module-level `__getattr__` (PEP 562) permits any name.
//! - A target containing `from x import *` has an unknowable member set and
//!   suppresses the rule for that module.
//! - `from pkg import mod` is satisfied by an existing `pkg/mod.py`,
//!   `pkg/mod.pyi`, or `pkg/mod/` submodule.
//!
//! Scope: `from`-imports resolved to workspace `.py` sources. Stub-backed
//! imports are covered by `imports_module_attribute`; site-packages sources
//! stay with `missing_type_stubs` (PEP 561 draws the trust boundary there).

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use basilisk_resolver::{ImportInfo, ImportResolution, ResolvedModule, Span};
use ruff_python_ast::visitor::{walk_expr, walk_stmt, Visitor};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged as _;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "imports_missing_name",
    docs_url: "https://www.basilisk-python.dev/errors/imports_missing_name",
};

/// Emits `imports_missing_name` for `from M import name` where the resolved
/// workspace module `M` defines no `name` (GitHub #55).
pub(crate) struct MissingImportedName;

impl Rule for MissingImportedName {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let candidates: Vec<&ImportInfo> = module
            .imports
            .iter()
            .filter(|import| is_checkable_from_import(import))
            .collect();
        if candidates.is_empty() {
            return;
        }
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let from_stmts = collect_from_imports(&parsed.ast.body);
        for import in candidates {
            let Some(stmt) = from_stmts.get(&(import.span.start, import.span.end)) else {
                continue;
            };
            check_from_import(import, stmt, module, diagnostics);
        }
    }
}

/// A `from`-import is checkable when it resolved to a workspace `.py` source.
///
/// Site-packages sources are excluded: untyped third-party code is
/// `missing_type_stubs`' domain, and PEP 561 says its contents must not be
/// trusted either way.
fn is_checkable_from_import(import: &ImportInfo) -> bool {
    if import.kind != basilisk_resolver::scope::ImportKind::From
        || import.resolution != ImportResolution::SourcePy
    {
        return false;
    }
    import.resolved_path.as_deref().is_some_and(|path| {
        path.extension().is_some_and(|ext| ext == "py")
            && !path.to_string_lossy().contains("site-packages")
    })
}

/// Index every `from`-import statement in the AST by its byte span, so each
/// resolved [`ImportInfo`] can be matched back to its alias list.
fn collect_from_imports(body: &[Stmt]) -> HashMap<(u32, u32), &ast::StmtImportFrom> {
    struct FromImportVisitor<'a> {
        found: HashMap<(u32, u32), &'a ast::StmtImportFrom>,
    }
    impl<'a> Visitor<'a> for FromImportVisitor<'a> {
        fn visit_stmt(&mut self, stmt: &'a Stmt) {
            if let Stmt::ImportFrom(import_from) = stmt {
                let range = import_from.range();
                let _previous = self
                    .found
                    .insert((range.start().to_u32(), range.end().to_u32()), import_from);
            }
            walk_stmt(self, stmt);
        }
    }
    let mut visitor = FromImportVisitor {
        found: HashMap::new(),
    };
    for stmt in body {
        visitor.visit_stmt(stmt);
    }
    visitor.found
}

/// Check one resolved `from`-import's aliases against the target module.
fn check_from_import(
    import: &ImportInfo,
    stmt: &ast::StmtImportFrom,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target_path) = import.resolved_path.as_deref() else {
        return;
    };
    let Some(bindings) = target_bindings(target_path) else {
        return; // unreadable or unparseable target: silence over guessing
    };
    if bindings.has_getattr || bindings.has_star_import {
        return;
    }
    let is_package_init = target_path
        .file_name()
        .is_some_and(|name| name == "__init__.py");
    for alias in &stmt.names {
        let imported = alias.name.as_str();
        if bindings.names.contains(imported) {
            continue;
        }
        if is_package_init && submodule_exists(target_path, imported) {
            continue;
        }
        // Cross-module mode resolves imports against live buffer content; if
        // it found the bound name, the on-disk view is stale — trust it.
        let bound = alias.asname.as_ref().map_or(imported, |name| name.as_str());
        if module.imported_symbols.contains_key(bound) {
            continue;
        }
        let range = alias.name.range();
        diagnostics.push(error_diagnostic_owned(
            CODE,
            format!(
                "Cannot import name `{imported}` from `{}` — the module defines no such name",
                import.module
            ),
            Span::new(range.start().to_u32(), range.end().to_u32()),
            &module.path,
            Some(format!(
                "define `{imported}` in `{}`, or fix the typo. A module-level \
                 `def __getattr__(name: str) -> Any: ...` makes the module dynamic \
                 and permits any name (PEP 562).",
                target_path.display()
            )),
            Some(
                "`from M import X` requires `X` to be an attribute of `M` or one of its \
                 submodules; otherwise it raises `ImportError` at runtime. \
                 https://docs.python.org/3/reference/import.html#submodules"
                    .to_owned(),
            ),
        ));
    }
}

/// Whether the package directory owning `init_path` contains a submodule
/// (`name.py`, `name.pyi`, or a `name/` package) — `from pkg import name`
/// imports the submodule when the `__init__` binds no such attribute.
fn submodule_exists(init_path: &std::path::Path, name: &str) -> bool {
    let Some(package_dir) = init_path.parent() else {
        return false;
    };
    package_dir.join(format!("{name}.py")).is_file()
        || package_dir.join(format!("{name}.pyi")).is_file()
        || package_dir.join(name).is_dir()
}

// ---------------------------------------------------------------------------
// Target-module binding collection
// ---------------------------------------------------------------------------

/// The importable surface of one target module, as visible statically.
struct TargetBindings {
    /// Every module-level bound name, across all binding statement forms.
    names: HashSet<String>,
    /// `__getattr__` present (PEP 562) — any name is importable.
    has_getattr: bool,
    /// The module star-imports another — its member set is unknowable.
    has_star_import: bool,
}

/// Memo key: target path plus its content hash, so an edited file re-parses.
type BindingsKey = (String, u64);
/// The memo table shared by every checked file in the process.
type BindingsMemo = HashMap<BindingsKey, std::sync::Arc<TargetBindings>>;

/// Per-process cache of parsed target bindings, keyed by path and content
/// hash. The content is re-read (and read-set-tracked) on every check so the
/// CLI result cache still records the dependency edge; only the parse and
/// AST walk are amortised.
fn bindings_memo() -> &'static Mutex<BindingsMemo> {
    static MEMO: OnceLock<Mutex<BindingsMemo>> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Read, parse, and index the target module's bindings. `None` when the file
/// is unreadable or does not parse — the rule stays silent rather than guess.
fn target_bindings(path: &std::path::Path) -> Option<std::sync::Arc<TargetBindings>> {
    let source = basilisk_common::fs::read_tracked(path).ok()?;
    let key = (
        path.to_string_lossy().into_owned(),
        basilisk_common::fs::content_hash(&source),
    );
    if let Some(cached) = bindings_memo().lock().ok()?.get(&key) {
        return Some(std::sync::Arc::clone(cached));
    }
    let parsed = basilisk_parser::parse_source(source, key.0.clone()).ok()?;
    let bindings = std::sync::Arc::new(collect_bindings(&parsed.ast.body));
    if let Ok(mut memo) = bindings_memo().lock() {
        let _previous = memo.insert(key, std::sync::Arc::clone(&bindings));
    }
    Some(bindings)
}

/// Collect every module-level binding in `body`, recursing through control
/// flow (`if`/`for`/`while`/`with`/`try`/`match`) but not into function or
/// class bodies — those bind their own scopes, not the module.
fn collect_bindings(body: &[Stmt]) -> TargetBindings {
    let mut bindings = TargetBindings {
        names: HashSet::new(),
        has_getattr: false,
        has_star_import: false,
    };
    collect_from_statements(body, &mut bindings);
    // Walrus targets bind the enclosing scope from arbitrary expression
    // positions; collect them from the whole tree. Over-inclusive on purpose
    // (a function-local walrus also lands here) — silence over guessing.
    let mut walrus = WalrusVisitor {
        names: &mut bindings.names,
    };
    for stmt in body {
        walrus.visit_stmt(stmt);
    }
    bindings.has_getattr = bindings.names.contains("__getattr__");
    bindings
}

fn collect_from_statements(body: &[Stmt], bindings: &mut TargetBindings) {
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(def) => {
                let _new = bindings.names.insert(def.name.to_string());
            }
            Stmt::ClassDef(def) => {
                let _new = bindings.names.insert(def.name.to_string());
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    collect_target(target, &mut bindings.names);
                }
            }
            Stmt::AnnAssign(assign) => collect_target(&assign.target, &mut bindings.names),
            Stmt::AugAssign(assign) => collect_target(&assign.target, &mut bindings.names),
            Stmt::TypeAlias(alias) => collect_target(&alias.name, &mut bindings.names),
            Stmt::Import(import) => {
                for alias in &import.names {
                    let bound = alias.asname.as_ref().map_or_else(
                        // Plain `import a.b` binds the top-level `a`.
                        || {
                            alias
                                .name
                                .split('.')
                                .next()
                                .unwrap_or(alias.name.as_str())
                                .to_owned()
                        },
                        ToString::to_string,
                    );
                    let _new = bindings.names.insert(bound);
                }
            }
            Stmt::ImportFrom(import) => {
                for alias in &import.names {
                    if alias.name.as_str() == "*" {
                        bindings.has_star_import = true;
                    } else {
                        let bound = alias.asname.as_ref().unwrap_or(&alias.name);
                        let _new = bindings.names.insert(bound.to_string());
                    }
                }
            }
            Stmt::For(for_stmt) => {
                collect_target(&for_stmt.target, &mut bindings.names);
                collect_from_statements(&for_stmt.body, bindings);
                collect_from_statements(&for_stmt.orelse, bindings);
            }
            Stmt::While(while_stmt) => {
                collect_from_statements(&while_stmt.body, bindings);
                collect_from_statements(&while_stmt.orelse, bindings);
            }
            Stmt::If(if_stmt) => {
                collect_from_statements(&if_stmt.body, bindings);
                for clause in &if_stmt.elif_else_clauses {
                    collect_from_statements(&clause.body, bindings);
                }
            }
            Stmt::With(with_stmt) => {
                for item in &with_stmt.items {
                    if let Some(vars) = &item.optional_vars {
                        collect_target(vars, &mut bindings.names);
                    }
                }
                collect_from_statements(&with_stmt.body, bindings);
            }
            Stmt::Try(try_stmt) => {
                collect_from_statements(&try_stmt.body, bindings);
                for handler in &try_stmt.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(name) = &handler.name {
                        let _new = bindings.names.insert(name.to_string());
                    }
                    collect_from_statements(&handler.body, bindings);
                }
                collect_from_statements(&try_stmt.orelse, bindings);
                collect_from_statements(&try_stmt.finalbody, bindings);
            }
            Stmt::Match(match_stmt) => {
                for case in &match_stmt.cases {
                    collect_pattern(&case.pattern, &mut bindings.names);
                    collect_from_statements(&case.body, bindings);
                }
            }
            _ => {}
        }
    }
}

/// Bind every name in an assignment-target expression (`a`, `a, b`, `[a, *b]`).
fn collect_target(target: &Expr, names: &mut HashSet<String>) {
    match target {
        Expr::Name(name) => {
            let _new = names.insert(name.id.to_string());
        }
        Expr::Tuple(tuple) => {
            for element in &tuple.elts {
                collect_target(element, names);
            }
        }
        Expr::List(list) => {
            for element in &list.elts {
                collect_target(element, names);
            }
        }
        Expr::Starred(starred) => collect_target(&starred.value, names),
        // Attribute/subscript targets mutate existing objects, binding nothing.
        _ => {}
    }
}

/// Bind every capture name in a `match` pattern.
fn collect_pattern(pattern: &ast::Pattern, names: &mut HashSet<String>) {
    match pattern {
        ast::Pattern::MatchAs(as_pattern) => {
            if let Some(name) = &as_pattern.name {
                let _new = names.insert(name.to_string());
            }
            if let Some(inner) = &as_pattern.pattern {
                collect_pattern(inner, names);
            }
        }
        ast::Pattern::MatchStar(star) => {
            if let Some(name) = &star.name {
                let _new = names.insert(name.to_string());
            }
        }
        ast::Pattern::MatchSequence(sequence) => {
            for inner in &sequence.patterns {
                collect_pattern(inner, names);
            }
        }
        ast::Pattern::MatchMapping(mapping) => {
            for inner in &mapping.patterns {
                collect_pattern(inner, names);
            }
            if let Some(rest) = &mapping.rest {
                let _new = names.insert(rest.to_string());
            }
        }
        ast::Pattern::MatchClass(class) => {
            for inner in &class.arguments.patterns {
                collect_pattern(inner, names);
            }
            for keyword in &class.arguments.keywords {
                collect_pattern(&keyword.pattern, names);
            }
        }
        ast::Pattern::MatchOr(or_pattern) => {
            for inner in &or_pattern.patterns {
                collect_pattern(inner, names);
            }
        }
        ast::Pattern::MatchValue(_) | ast::Pattern::MatchSingleton(_) => {}
    }
}

/// Records walrus (`:=`) targets from every expression in the tree.
struct WalrusVisitor<'names> {
    names: &'names mut HashSet<String>,
}

impl Visitor<'_> for WalrusVisitor<'_> {
    fn visit_expr(&mut self, expr: &Expr) {
        if let Expr::Named(named) = expr {
            collect_target(&named.target, self.names);
        }
        walk_expr(self, expr);
    }
}
