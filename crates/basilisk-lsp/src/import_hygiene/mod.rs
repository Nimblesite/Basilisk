//! Implements [LSPFMT-IMPORTS]. See docs/specs/LSP-FORMATTING-SPEC.md#LSPFMT-IMPORTS
//!
//! Native import hygiene on the Ruff AST — **no `ruff check` subprocess**
//! (#261). Three source-to-source fixers, each returning `None` when there is
//! nothing to do:
//!
//! - [`organize_source`] — sort the leading import block with isort semantics
//!   (replaces `ruff check --select I --fix`).
//! - [`split_multi_imports`] — one `import` statement per module
//!   (replaces `ruff check --select E401 --fix`).
//! - [`expand_wildcard_source`] — replace `from X import *` with the names
//!   the file actually uses. Ruff has **no** autofix for F403, so the old
//!   subprocess path could never produce this fix; the native fixer defines
//!   the behavior.

use ruff_python_ast::visitor::{walk_stmt, Visitor};
use ruff_python_ast::{Stmt, StmtImport};
use ruff_python_parser::parse_module;
use ruff_text_size::Ranged;

mod sort;
mod wildcard;

pub use sort::organize_source;
pub use wildcard::expand_wildcard_source;

/// Collects every `import a, b, ...` statement (more than one alias), at any
/// nesting depth.
struct MultiImports<'a> {
    found: Vec<&'a StmtImport>,
}

impl<'a> Visitor<'a> for MultiImports<'a> {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        if let Stmt::Import(import) = stmt {
            if import.names.len() > 1 {
                self.found.push(import);
            }
        }
        walk_stmt(self, stmt);
    }
}

/// Split every `import a, b` statement into one statement per module,
/// preserving the original module order (Ruff E401 fix parity).
///
/// Returns `None` when no statement needs splitting or the source does not
/// parse.
#[must_use]
pub fn split_multi_imports(source: &str) -> Option<String> {
    let parsed = parse_module(source).ok()?;
    let mut collector = MultiImports { found: Vec::new() };
    for stmt in &parsed.syntax().body {
        collector.visit_stmt(stmt);
    }
    if collector.found.is_empty() {
        return None;
    }

    // Rewrite back-to-front so earlier byte offsets stay valid.
    let mut new_source = source.to_owned();
    for import in collector.found.iter().rev() {
        let start = usize::from(import.range().start());
        let end = usize::from(import.range().end());
        let indent = line_indent(source, start)?;
        let statements: Vec<String> = import
            .names
            .iter()
            .map(|alias| match &alias.asname {
                Some(asname) => format!("import {} as {asname}", alias.name),
                None => format!("import {}", alias.name),
            })
            .collect();
        let separator = format!("\n{indent}");
        new_source.replace_range(start..end, &statements.join(&separator));
    }
    Some(new_source)
}

/// The whitespace indentation of the line `offset` sits on, or `None` when
/// anything other than whitespace precedes `offset` on its line (e.g. after a
/// `;`) — rewriting such statements line-wise would corrupt the code.
fn line_indent(source: &str, offset: usize) -> Option<String> {
    let line_start = source
        .get(..offset)?
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    let indent = source.get(line_start..offset)?;
    indent
        .chars()
        .all(char::is_whitespace)
        .then(|| indent.to_owned())
}
