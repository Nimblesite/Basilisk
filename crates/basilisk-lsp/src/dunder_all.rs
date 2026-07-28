//! Implements [LSPARCH-FEATURES-RENAME]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-RENAME
//!
//! `__all__` export-entry lookup for rename.
//!
//! Renaming a module-level symbol must also rewrite the matching string in the
//! module's `__all__` export list. Those strings are the one place where a
//! string literal really is a reference, so they are found here — from the AST,
//! never by scanning lines for quotes. A text scan cannot tell where the
//! `__all__` statement ends (a trailing comment or the tuple form has no
//! closing `]` on its own line), and running past the end rewrites unrelated
//! string literals throughout the file.

use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;
use tower_lsp::lsp_types::Range;

/// Find the `__all__` entries in `source` whose value is exactly `name`.
///
/// Returns the range of the name *inside* the quotes, ready to be replaced.
/// Returns nothing when `source` does not parse — better to skip the `__all__`
/// update than to guess at its extent.
#[must_use]
pub(crate) fn find_dunder_all_entries(source: &str, name: &str) -> Vec<Range> {
    let Ok(parsed) = ruff_python_parser::parse_module(source) else {
        return Vec::new();
    };
    let mut results = Vec::new();
    for stmt in &parsed.syntax().body {
        if let Some(value) = dunder_all_value(stmt) {
            collect_entries(value, name, source, &mut results);
        }
    }
    results
}

/// The assigned value of a statement that binds `__all__`, if this is one.
///
/// Covers `__all__ = [...]`, `__all__: list[str] = [...]` and `__all__ += [...]`.
fn dunder_all_value(stmt: &Stmt) -> Option<&Expr> {
    match stmt {
        Stmt::Assign(assign) => assign
            .targets
            .iter()
            .any(is_dunder_all)
            .then(|| assign.value.as_ref()),
        Stmt::AnnAssign(assign) => is_dunder_all(&assign.target)
            .then_some(assign.value.as_deref())
            .flatten(),
        Stmt::AugAssign(assign) => is_dunder_all(&assign.target).then(|| assign.value.as_ref()),
        _ => None,
    }
}

/// Whether `expr` is the name `__all__`.
fn is_dunder_all(expr: &Expr) -> bool {
    matches!(expr, Expr::Name(name) if name.id.as_str() == "__all__")
}

/// Collect matching string entries from an `__all__` value expression.
///
/// Handles the list, tuple and set spellings, plus `+`-concatenated lists.
fn collect_entries(value: &Expr, name: &str, source: &str, results: &mut Vec<Range>) {
    match value {
        Expr::List(list) => push_matching(&list.elts, name, source, results),
        Expr::Tuple(tuple) => push_matching(&tuple.elts, name, source, results),
        Expr::Set(set) => push_matching(&set.elts, name, source, results),
        Expr::BinOp(bin_op) => {
            collect_entries(&bin_op.left, name, source, results);
            collect_entries(&bin_op.right, name, source, results);
        }
        _ => {}
    }
}

/// Push a range for every element that is the string literal `name`.
fn push_matching(elements: &[Expr], name: &str, source: &str, results: &mut Vec<Range>) {
    for element in elements {
        let Expr::StringLiteral(literal) = element else {
            continue;
        };
        if literal.value.to_str() != name {
            continue;
        }
        // The literal's range spans the quotes (and any prefix); the name sits
        // at the first occurrence of its own text inside that span.
        let start = literal.range().start().to_usize();
        let end = literal.range().end().to_usize();
        let Some(name_start) = source
            .get(start..end)
            .and_then(|text| text.find(name))
            .map(|offset| start + offset)
        else {
            continue;
        };
        results.push(Range {
            start: crate::util::byte_offset_to_position(source, name_start),
            end: crate::util::byte_offset_to_position(source, name_start + name.len()),
        });
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::find_dunder_all_entries;

    #[test]
    fn finds_a_plain_list_entry() {
        let source = "__all__ = [\"run\"]\n";
        let found = find_dunder_all_entries(source, "run");
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().map(|range| range.start.character), Some(12));
        assert_eq!(found.first().map(|range| range.end.character), Some(15));
    }

    #[test]
    fn a_trailing_comment_does_not_extend_the_block() {
        let source =
            "__all__ = [\"run\"]  # public API\n\n\ndef run() -> None:\n    print(\"run\")\n";
        let found = find_dunder_all_entries(source, "run");
        assert_eq!(found.len(), 1, "only the export entry may match");
        assert_eq!(found.first().map(|range| range.start.line), Some(0));
    }

    #[test]
    fn the_tuple_form_does_not_extend_the_block() {
        let source = "__all__ = (\"run\",)\n\nmessage = \"run\"\n";
        let found = find_dunder_all_entries(source, "run");
        assert_eq!(found.len(), 1, "the later string literal is not an export");
        assert_eq!(found.first().map(|range| range.start.line), Some(0));
    }

    #[test]
    fn finds_every_entry_on_one_line() {
        let source = "__all__ = [\"run\", \"run\"]\n";
        assert_eq!(find_dunder_all_entries(source, "run").len(), 2);
    }

    #[test]
    fn handles_annotated_and_augmented_forms() {
        let annotated = "__all__: list[str] = [\"run\"]\n";
        assert_eq!(find_dunder_all_entries(annotated, "run").len(), 1);
        let augmented = "__all__ = [\"a\"]\n__all__ += [\"run\"]\n";
        assert_eq!(find_dunder_all_entries(augmented, "run").len(), 1);
    }

    #[test]
    fn columns_are_utf16_offsets() {
        // The en dash is 3 bytes but 1 UTF-16 code unit.
        let source = "header = \"a \u{2013} b\"\n__all__ = [\"run\"]\n";
        let found = find_dunder_all_entries(source, "run");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].start.line, 1);
        assert_eq!(found[0].start.character, 12);
    }

    #[test]
    fn unparseable_source_yields_nothing() {
        assert!(find_dunder_all_entries("def broken(:\n", "run").is_empty());
    }

    #[test]
    fn ignores_a_non_matching_name() {
        assert!(find_dunder_all_entries("__all__ = [\"other\"]\n", "run").is_empty());
    }
}
