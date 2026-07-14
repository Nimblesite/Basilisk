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

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;

    /// Basic expansion: a single wildcard is replaced in place by the one
    /// used-unbound-non-builtin name. Implements [LSPFMT-IMPORTS].
    #[test]
    fn expands_single_used_name() {
        let source = "from os import *\nprint(getcwd())\n";
        let expanded = expand_wildcard_source(source).unwrap();
        assert_eq!(expanded, "from os import getcwd\nprint(getcwd())\n");
    }

    /// Multiple used names are sorted and deduped in the replacement.
    #[test]
    fn names_are_sorted_and_deduped() {
        let source = "from os import *\nx = getcwd()\ny = sep\nz = getcwd()\n";
        let expanded = expand_wildcard_source(source).unwrap();
        assert_eq!(
            expanded,
            "from os import getcwd, sep\nx = getcwd()\ny = sep\nz = getcwd()\n"
        );
    }

    /// A name assigned in the module (Store ctx) is bound and never re-imported,
    /// even though it is also read.
    #[test]
    fn assigned_name_is_bound_not_imported() {
        let source = "from os import *\nsep = 3\nvalue = sep + getcwd()\n";
        let expanded = expand_wildcard_source(source).unwrap();
        assert_eq!(
            expanded,
            "from os import getcwd\nsep = 3\nvalue = sep + getcwd()\n"
        );
    }

    /// A name defined via `def`/`class` shadows the candidate (nested defs too).
    #[test]
    fn function_and_class_defs_are_bound() {
        let source = concat!(
            "from os import *\n",
            "def getcwd():\n",
            "    def helper():\n",
            "        return inner()\n",
            "    return sep\n",
            "class sep:\n",
            "    pass\n",
        );
        // `getcwd` and `sep` are bound by def/class; only `inner` remains used-unbound.
        let expanded = expand_wildcard_source(source).unwrap();
        assert!(expanded.starts_with("from os import inner\n"), "{expanded}");
    }

    /// A name imported elsewhere (`Import` / `ImportFrom`) is bound.
    #[test]
    fn imported_names_are_bound() {
        let source = concat!(
            "from os import *\n",
            "import sys\n",
            "from collections import getcwd\n",
            "x = sys.argv\n",
            "y = getcwd()\n",
            "z = sep\n",
        );
        // `sys` bound by import, `getcwd` bound by from-import; only `sep` remains.
        let expanded = expand_wildcard_source(source).unwrap();
        assert_eq!(
            expanded,
            concat!(
                "from os import sep\n",
                "import sys\n",
                "from collections import getcwd\n",
                "x = sys.argv\n",
                "y = getcwd()\n",
                "z = sep\n",
            )
        );
    }

    /// A dotted import binds only its top-level name; an aliased import binds
    /// the alias.
    #[test]
    fn dotted_and_aliased_imports_bind_correctly() {
        let source = concat!(
            "from os import *\n",
            "import a.b.c\n",
            "import d as sep\n",
            "x = a.thing\n",
            "y = sep\n",
            "z = getcwd()\n",
        );
        // `a` bound (top of a.b.c), `sep` bound (alias); only `getcwd` remains.
        let expanded = expand_wildcard_source(source).unwrap();
        assert!(
            expanded.starts_with("from os import getcwd\n"),
            "{expanded}"
        );
    }

    /// Builtins and magic globals are never re-imported.
    #[test]
    fn builtins_are_excluded() {
        let source = "from os import *\nprint(len(__name__))\nx = getcwd()\n";
        let expanded = expand_wildcard_source(source).unwrap();
        // `print`, `len`, `__name__` are builtins/magic; only `getcwd` remains.
        assert_eq!(
            expanded,
            "from os import getcwd\nprint(len(__name__))\nx = getcwd()\n"
        );
    }

    /// No wildcard import at all → None.
    #[test]
    fn no_wildcard_returns_none() {
        let source = "from os import getcwd\nprint(getcwd())\n";
        assert_eq!(expand_wildcard_source(source), None);
    }

    /// Two wildcard imports → ambiguous origin → None.
    #[test]
    fn two_wildcards_returns_none() {
        let source = "from os import *\nfrom sys import *\nx = getcwd()\n";
        assert_eq!(expand_wildcard_source(source), None);
    }

    /// A wildcard with no used-unbound-non-builtin name → None.
    #[test]
    fn wildcard_with_no_remaining_name_returns_none() {
        let source = "from os import *\nprint(len([]))\n";
        assert_eq!(expand_wildcard_source(source), None);
    }

    /// Unparseable source → None.
    #[test]
    fn unparseable_source_returns_none() {
        let source = "from os import *\ndef (:\n";
        assert_eq!(expand_wildcard_source(source), None);
    }

    /// A bare relative wildcard (`from . import *`, module None, level 1)
    /// produces a single leading dot and empty module.
    #[test]
    fn bare_relative_wildcard_module_none() {
        let source = "from . import *\nx = getcwd()\n";
        let expanded = expand_wildcard_source(source).unwrap();
        assert_eq!(expanded, "from . import getcwd\nx = getcwd()\n");
    }

    /// A multi-level relative wildcard (`from ..pkg import *`, level 2)
    /// produces two leading dots plus the module name.
    #[test]
    fn multi_level_relative_wildcard() {
        let source = "from ..pkg import *\nx = getcwd()\n";
        let expanded = expand_wildcard_source(source).unwrap();
        assert_eq!(expanded, "from ..pkg import getcwd\nx = getcwd()\n");
    }

    /// `global` declarations bind their names.
    #[test]
    fn global_declaration_binds() {
        let source = concat!(
            "from os import *\n",
            "def f():\n",
            "    global sep\n",
            "    sep = getcwd()\n",
        );
        // `sep` bound by global; only `getcwd` remains.
        let expanded = expand_wildcard_source(source).unwrap();
        assert!(
            expanded.starts_with("from os import getcwd\n"),
            "{expanded}"
        );
    }

    /// `nonlocal` declarations bind their names.
    #[test]
    fn nonlocal_declaration_binds() {
        let source = concat!(
            "from os import *\n",
            "def outer():\n",
            "    sep = 1\n",
            "    def inner():\n",
            "        nonlocal sep\n",
            "        sep = getcwd()\n",
            "    return inner\n",
        );
        // `sep` bound by nonlocal/assign; only `getcwd` remains.
        let expanded = expand_wildcard_source(source).unwrap();
        assert!(
            expanded.starts_with("from os import getcwd\n"),
            "{expanded}"
        );
    }

    /// An except handler binding name (`except E as sep`) is bound.
    #[test]
    fn except_handler_name_is_bound() {
        let source = concat!(
            "from os import *\n",
            "try:\n",
            "    x = getcwd()\n",
            "except Exception as sep:\n",
            "    y = sep\n",
        );
        // `sep` bound by the handler; only `getcwd` remains.
        let expanded = expand_wildcard_source(source).unwrap();
        assert_eq!(
            expanded,
            concat!(
                "from os import getcwd\n",
                "try:\n",
                "    x = getcwd()\n",
                "except Exception as sep:\n",
                "    y = sep\n",
            )
        );
    }

    /// A function parameter is bound and not re-imported.
    #[test]
    fn function_parameter_is_bound() {
        let source = concat!(
            "from os import *\n",
            "def f(sep):\n",
            "    return sep + getcwd()\n",
        );
        // `sep` bound as a parameter; only `getcwd` remains.
        let expanded = expand_wildcard_source(source).unwrap();
        assert!(
            expanded.starts_with("from os import getcwd\n"),
            "{expanded}"
        );
    }

    /// Match `case`-capture patterns (`MatchAs`, `MatchStar`, `MatchMapping`) bind.
    #[test]
    fn match_patterns_bind_names() {
        let source = concat!(
            "from os import *\n",
            "def f(subject):\n",
            "    match subject:\n",
            "        case [first, *sep]:\n",
            "            return first\n",
            "        case {\"k\": value, **getcwd}:\n",
            "            return value\n",
            "        case other:\n",
            "            return other + linesep\n",
        );
        // `sep` (MatchStar), `getcwd` (MatchMapping rest), `first`/`value`/`other`
        // (MatchAs captures) are all bound; only `linesep` remains used-unbound.
        let expanded = expand_wildcard_source(source).unwrap();
        assert!(
            expanded.starts_with("from os import linesep\n"),
            "{expanded}"
        );
    }
}
