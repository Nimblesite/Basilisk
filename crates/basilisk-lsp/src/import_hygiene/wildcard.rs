//! Implements [LSPFMT-IMPORTS] — expand `from X import *` into the names the
//! file actually uses from it.
//!
//! Ruff has no autofix for F403, so there is no fixer to mirror; the native
//! behavior is defined here: collect every name the module *reads* but never
//! *binds* (and that is not a builtin) — those can only come from the
//! wildcard — and import them explicitly, sorted. Binding is intentionally
//! coarse (a name bound in any scope shadows the candidate), which
//! under-expands in pathological shadowing cases but never invents an import
//! the file does not use.

use std::collections::HashSet;

use ruff_python_ast::visitor::{
    walk_except_handler, walk_expr, walk_parameter, walk_pattern, walk_stmt, Visitor,
};
use ruff_python_ast::{ExceptHandler, Expr, ExprContext, Parameter, Pattern, Stmt, StmtImportFrom};
use ruff_python_parser::parse_module;
use ruff_text_size::Ranged;

/// Canonical Python minor version for the builtins table (3.12).
const PYTHON_MINOR: u8 = 12;

/// Names a module reads (`used`) and names it binds anywhere (`bound`).
#[derive(Default)]
struct NameUsage<'a> {
    used: HashSet<&'a str>,
    bound: HashSet<&'a str>,
}

impl<'a> Visitor<'a> for NameUsage<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(function) => {
                let _ = self.bound.insert(function.name.as_str());
            }
            Stmt::ClassDef(class) => {
                let _ = self.bound.insert(class.name.as_str());
            }
            Stmt::Global(global) => {
                self.bound
                    .extend(global.names.iter().map(ruff_python_ast::Identifier::as_str));
            }
            Stmt::Nonlocal(nonlocal) => {
                self.bound.extend(
                    nonlocal
                        .names
                        .iter()
                        .map(ruff_python_ast::Identifier::as_str),
                );
            }
            Stmt::Import(import) => {
                for alias in &import.names {
                    let name = alias.asname.as_ref().map_or_else(
                        || alias.name.split('.').next().unwrap_or(alias.name.as_str()),
                        ruff_python_ast::Identifier::as_str,
                    );
                    let _ = self.bound.insert(name);
                }
            }
            Stmt::ImportFrom(import) => {
                for alias in &import.names {
                    if alias.name.as_str() != "*" {
                        let name = alias
                            .asname
                            .as_ref()
                            .map_or_else(|| alias.name.as_str(), |asname| asname.as_str());
                        let _ = self.bound.insert(name);
                    }
                }
            }
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        if let Expr::Name(name) = expr {
            let _ = match name.ctx {
                ExprContext::Load => self.used.insert(name.id.as_str()),
                _ => self.bound.insert(name.id.as_str()),
            };
        }
        walk_expr(self, expr);
    }

    fn visit_parameter(&mut self, parameter: &'a Parameter) {
        let _ = self.bound.insert(parameter.name.as_str());
        walk_parameter(self, parameter);
    }

    fn visit_except_handler(&mut self, except_handler: &'a ExceptHandler) {
        let ExceptHandler::ExceptHandler(handler) = except_handler;
        if let Some(name) = &handler.name {
            let _ = self.bound.insert(name.as_str());
        }
        walk_except_handler(self, except_handler);
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern) {
        match pattern {
            Pattern::MatchAs(match_as) => {
                if let Some(name) = &match_as.name {
                    let _ = self.bound.insert(name.as_str());
                }
            }
            Pattern::MatchStar(match_star) => {
                if let Some(name) = &match_star.name {
                    let _ = self.bound.insert(name.as_str());
                }
            }
            Pattern::MatchMapping(match_mapping) => {
                if let Some(rest) = &match_mapping.rest {
                    let _ = self.bound.insert(rest.as_str());
                }
            }
            _ => {}
        }
        walk_pattern(self, pattern);
    }
}

/// Find the module's wildcard imports (`from x import *`) at any top level.
fn wildcard_imports(body: &[Stmt]) -> Vec<&StmtImportFrom> {
    body.iter()
        .filter_map(|stmt| match stmt {
            Stmt::ImportFrom(import) if import.names.iter().any(|a| a.name.as_str() == "*") => {
                Some(import)
            }
            _ => None,
        })
        .collect()
}

/// Replace the module's single `from X import *` with explicit imports of
/// the names the file uses from it.
///
/// Returns `None` when there is no wildcard import, more than one (the
/// origin of each name would be ambiguous), no used name remains after
/// removing bound names and builtins, or the source does not parse.
#[must_use]
pub fn expand_wildcard_source(source: &str) -> Option<String> {
    let parsed = parse_module(source).ok()?;
    let body = &parsed.syntax().body;

    let wildcards = wildcard_imports(body);
    let [wildcard] = wildcards.as_slice() else {
        return None;
    };

    let mut usage = NameUsage::default();
    for stmt in body {
        usage.visit_stmt(stmt);
    }

    let builtins: HashSet<&str> =
        ruff_python_stdlib::builtins::python_builtins(PYTHON_MINOR, false)
            .chain(ruff_python_stdlib::builtins::python_magic_globals(
                PYTHON_MINOR,
            ))
            .collect();

    let mut names: Vec<&str> = usage
        .used
        .iter()
        .filter(|name| !usage.bound.contains(*name) && !builtins.contains(*name))
        .copied()
        .collect();
    names.sort_unstable();
    if names.is_empty() {
        return None;
    }

    let dots = ".".repeat(usize::try_from(wildcard.level).unwrap_or(0));
    let module = wildcard.module.as_ref().map_or("", |m| m.as_str());
    let replacement = format!("from {dots}{module} import {}", names.join(", "));

    let start = usize::from(wildcard.range().start());
    let end = usize::from(wildcard.range().end());
    Some(format!(
        "{}{replacement}{}",
        source.get(..start)?,
        source.get(end..)?
    ))
}
