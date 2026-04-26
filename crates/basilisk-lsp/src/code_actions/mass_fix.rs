//! Mass autofix engine — apply all fixable diagnostics in a single action.
//!
//! Collects per-diagnostic fixes, resolves overlapping edits (keeping the
//! earlier fix when ranges conflict), and returns a single `WorkspaceEdit`
//! that VS Code can undo in one step.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, NumberOrString, Range, TextEdit, Url, WorkspaceEdit,
};

use super::fixes;

/// All diagnostic codes that have an autofix implementation.
pub const ALL_FIXABLE_RULES: &[&str] = &[
    "BSK-E0001", // Missing parameter type → `: Any`
    "BSK-E0002", // Missing return type → `-> None`
    "BSK-E0003", // Missing variable type → `: <inferred>`
    "BSK-E0005", // Missing class attribute type → `: Any`
    "BSK-W0050", // Redundant annotation → remove
];

/// Rules classified as safe (guaranteed not to change runtime semantics).
///
/// BSK-E0003 is intentionally excluded: its fix (adding `: Any`) conflicts
/// with BSK-W0050's fix (removing redundant annotations), producing a
/// non-idempotent cycle. Users who want E0003 auto-fixed must pass it
/// explicitly via `--rules BSK-E0003` or use `--unsafe`.
pub const SAFE_FIXABLE_RULES: &[&str] = &["BSK-E0001", "BSK-E0002", "BSK-E0005", "BSK-W0050"];

/// Custom `CodeActionKind` for "fix all safe diagnostics in this file".
///
/// VS Code triggers this when the user runs `source.fixAll.basilisk` from
/// the command palette or a keybinding.
pub(crate) fn fix_all_kind() -> CodeActionKind {
    CodeActionKind::new("source.fixAll.basilisk")
}

/// Build a single code action that fixes every fixable diagnostic in the file.
///
/// Returns `None` if no diagnostics have applicable fixes.
/// Uses `source.fixAll.basilisk` kind (for on-save / command palette).
#[must_use]
pub fn fix_all_in_file(uri: &Url, diagnostics: &[Diagnostic], source: &str) -> Option<CodeAction> {
    build_fix_all(
        uri,
        diagnostics,
        source,
        CodeActionKind::new("source.fixAll.basilisk"),
    )
}

/// Build a "Fix all" code action with `quickfix` kind so it appears
/// in the lightbulb Quick Fix menu alongside per-diagnostic fixes.
///
/// Only returned when there are 2+ fixable diagnostics — for a single
/// fixable diagnostic the per-diagnostic fix is sufficient.
pub(crate) fn fix_all_quickfix(
    uri: &Url,
    diagnostics: &[Diagnostic],
    source: &str,
) -> Option<CodeAction> {
    let action = build_fix_all(uri, diagnostics, source, CodeActionKind::QUICKFIX)?;
    let edit_count = action
        .edit
        .as_ref()
        .and_then(|ws| ws.changes.as_ref())
        .and_then(|c| c.get(uri))
        .map_or(0, Vec::len);
    if edit_count < 2 {
        return None;
    }
    Some(action)
}

/// Build a "Fix all <rule> in this file" quickfix action for a single rule.
///
/// Filters `diagnostics` to only the given `rule_code`, then builds edits.
/// Returns `None` when fewer than 2 instances of that rule are fixable
/// (the per-diagnostic fix already covers the single-instance case).
pub(crate) fn fix_all_by_rule(
    uri: &Url,
    diagnostics: &[Diagnostic],
    source: &str,
    rule_code: &str,
) -> Option<CodeAction> {
    let filtered: Vec<_> = diagnostics
        .iter()
        .filter(|d| matches!(&d.code, Some(NumberOrString::String(c)) if c == rule_code))
        .cloned()
        .collect();

    let edits = collect_non_overlapping_edits(uri, &filtered, source);
    if edits.len() < 2 {
        return None;
    }

    let count = edits.len();
    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), edits);

    Some(CodeAction {
        title: format!("Fix all `{rule_code}` in this file ({count} fixes)"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(filtered),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}

/// Build a single code action that fixes diagnostics matching any of the given rule codes.
///
/// Returns `None` if no matching diagnostics have applicable fixes.
#[must_use]
pub fn fix_filtered_in_file(
    uri: &Url,
    diagnostics: &[Diagnostic],
    source: &str,
    allowed_rules: &[&str],
) -> Option<CodeAction> {
    let filtered: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            matches!(&d.code, Some(NumberOrString::String(c)) if allowed_rules.contains(&c.as_str()))
        })
        .cloned()
        .collect();

    build_fix_all(
        uri,
        &filtered,
        source,
        CodeActionKind::new("source.fixAll.basilisk"),
    )
}

/// Shared builder for fix-all code actions.
fn build_fix_all(
    uri: &Url,
    diagnostics: &[Diagnostic],
    source: &str,
    kind: CodeActionKind,
) -> Option<CodeAction> {
    let edits = collect_non_overlapping_edits(uri, diagnostics, source);
    if edits.is_empty() {
        return None;
    }

    let count = edits.len();
    let mut changes = HashMap::new();
    let _ = changes.insert(uri.clone(), edits);

    Some(CodeAction {
        title: format!("Fix all auto-fixable issues ({count} fixes)"),
        kind: Some(kind),
        diagnostics: Some(diagnostics.to_vec()),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

/// Collect text edits for all fixable diagnostics, discarding any that
/// overlap with an earlier (by start position) edit.
fn collect_non_overlapping_edits(
    uri: &Url,
    diagnostics: &[Diagnostic],
    source: &str,
) -> Vec<TextEdit> {
    // 1. Generate candidate edits from each fixable diagnostic.
    let mut candidates: Vec<TextEdit> = diagnostics
        .iter()
        .filter_map(|diag| single_fix_edit(uri, diag, source))
        .collect();

    // 2. Sort by start position (line, then character).
    candidates.sort_by(|a, b| {
        a.range
            .start
            .line
            .cmp(&b.range.start.line)
            .then(a.range.start.character.cmp(&b.range.start.character))
    });

    // 3. Greedily keep non-overlapping edits.
    let mut accepted: Vec<TextEdit> = Vec::with_capacity(candidates.len());
    for edit in candidates {
        if let Some(last) = accepted.last() {
            if ranges_overlap(&last.range, &edit.range) {
                continue; // Skip — conflicts with an already-accepted edit.
            }
        }
        accepted.push(edit);
    }

    accepted
}

/// Extract the single `TextEdit` from a per-diagnostic fix, if one exists.
fn single_fix_edit(uri: &Url, diag: &Diagnostic, source: &str) -> Option<TextEdit> {
    let code = match &diag.code {
        Some(NumberOrString::String(s)) => s.as_str(),
        _ => return None,
    };

    let action = match code {
        "BSK-E0001" => fixes::fix_missing_param_annotation(uri, diag),
        "BSK-E0002" => fixes::fix_missing_return_annotation(uri, diag),
        "BSK-E0003" => fixes::fix_missing_variable_annotation(uri, diag),
        "BSK-W0050" => fixes::fix_remove_redundant_annotation(uri, diag, source),
        "BSK-E0005" => fixes::fix_missing_attribute_annotation(uri, diag),
        _ => return None,
    };

    // Each of our fix functions produces exactly one TextEdit.
    action
        .edit
        .and_then(|ws| ws.changes)
        .and_then(|mut map| map.remove(uri))
        .and_then(|mut edits| {
            if edits.len() == 1 {
                Some(edits.remove(0))
            } else {
                None
            }
        })
}

/// Two ranges overlap if one starts before the other ends.
fn ranges_overlap(a: &Range, b: &Range) -> bool {
    let a_before_b = position_le(a.end, b.start);
    let b_before_a = position_le(b.end, a.start);
    !(a_before_b || b_before_a)
}

/// `a <= b` in terms of document position.
fn position_le(a: tower_lsp::lsp_types::Position, b: tower_lsp::lsp_types::Position) -> bool {
    a.line < b.line || (a.line == b.line && a.character <= b.character)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString, Position};

    fn make_diag(code: &str, start_line: u32, start_char: u32, end_char: u32) -> Diagnostic {
        Diagnostic {
            range: Range {
                start: Position::new(start_line, start_char),
                end: Position::new(start_line, end_char),
            },
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String(code.to_owned())),
            source: Some("basilisk".to_owned()),
            message: String::new(),
            ..Default::default()
        }
    }

    #[test]
    fn test_fix_all_no_fixable_diagnostics() {
        let uri = Url::parse("file:///test.py").unwrap();
        let diag = make_diag("BSK-E9999", 0, 0, 5); // Unknown code — no fix.
        let result = fix_all_in_file(&uri, &[diag], "x = 42\n");
        assert!(result.is_none());
    }

    #[test]
    fn test_fix_all_single_w0050() {
        let uri = Url::parse("file:///test.py").unwrap();
        let diag = make_diag("BSK-W0050", 0, 0, 1);
        let source = "x: int = 42\n";
        let action = fix_all_in_file(&uri, &[diag], source);
        assert!(action.is_some());
        let action = action.unwrap();
        assert!(action.title.contains("1 fixes"));
        assert_eq!(action.kind, Some(fix_all_kind()));
        let changes = action.edit.unwrap().changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits.len(), 1);
    }

    #[test]
    fn test_fix_all_multiple_non_overlapping() {
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "x: int = 42\ny: str = 'hello'\n";
        let diags = vec![
            make_diag("BSK-W0050", 0, 0, 1),
            make_diag("BSK-W0050", 1, 0, 1),
        ];
        let action = fix_all_in_file(&uri, &diags, source);
        assert!(action.is_some());
        let changes = action.unwrap().edit.unwrap().changes.unwrap();
        let edits = changes.get(&uri).unwrap();
        assert_eq!(edits.len(), 2);
    }

    #[test]
    fn test_fix_all_by_rule_filters_to_single_code() {
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "x: int = 42\ny: str = 'hello'\n";
        let diags = vec![
            make_diag("BSK-W0050", 0, 0, 1),
            make_diag("BSK-W0050", 1, 0, 1),
            make_diag("BSK-E0001", 1, 5, 10), // different rule
        ];
        let action = fix_all_by_rule(&uri, &diags, source, "BSK-W0050");
        assert!(action.is_some());
        let action = action.unwrap();
        assert!(
            action.title.contains("BSK-W0050"),
            "title should include rule code: {}",
            action.title
        );
        assert!(action.title.contains("2 fixes"), "title: {}", action.title);
        assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
        // Verify only W0050 diagnostics are attached.
        let attached = action.diagnostics.unwrap();
        assert!(attached.iter().all(|d| matches!(
            &d.code,
            Some(NumberOrString::String(c)) if c == "BSK-W0050"
        )));
    }

    #[test]
    fn test_fix_all_by_rule_returns_none_for_single_instance() {
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "x: int = 42\n";
        let diags = vec![make_diag("BSK-W0050", 0, 0, 1)];
        // Only 1 instance — per-diagnostic fix is sufficient.
        let action = fix_all_by_rule(&uri, &diags, source, "BSK-W0050");
        assert!(action.is_none());
    }

    #[test]
    fn test_fix_all_by_rule_returns_none_for_unfixable_code() {
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "x = 42\n";
        let diags = vec![
            make_diag("BSK-E9999", 0, 0, 5),
            make_diag("BSK-E9999", 0, 6, 10),
        ];
        let action = fix_all_by_rule(&uri, &diags, source, "BSK-E9999");
        assert!(action.is_none());
    }

    #[test]
    fn test_ranges_overlap_no_overlap() {
        let a = Range::new(Position::new(0, 0), Position::new(0, 5));
        let b = Range::new(Position::new(0, 5), Position::new(0, 10));
        assert!(!ranges_overlap(&a, &b));
    }

    #[test]
    fn test_ranges_overlap_yes() {
        let a = Range::new(Position::new(0, 0), Position::new(0, 6));
        let b = Range::new(Position::new(0, 5), Position::new(0, 10));
        assert!(ranges_overlap(&a, &b));
    }

    #[test]
    fn test_ranges_overlap_different_lines() {
        let a = Range::new(Position::new(0, 0), Position::new(0, 10));
        let b = Range::new(Position::new(1, 0), Position::new(1, 10));
        assert!(!ranges_overlap(&a, &b));
    }

    #[test]
    fn test_fix_filtered_includes_only_matching_rules() {
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "x: int = 42\ny: str = 'hello'\n";
        let diags = vec![
            make_diag("BSK-W0050", 0, 0, 1),
            make_diag("BSK-W0050", 1, 0, 1),
            make_diag("BSK-E0001", 1, 5, 10), // different rule
        ];
        // Only allow W0050 — E0001 should be excluded.
        let action = fix_filtered_in_file(&uri, &diags, source, &["BSK-W0050"]);
        assert!(action.is_some(), "should produce a fix for W0050");
        let action = action.unwrap();
        // All attached diagnostics must be W0050.
        let attached = action.diagnostics.unwrap();
        assert!(attached.iter().all(|d| matches!(
            &d.code,
            Some(NumberOrString::String(c)) if c == "BSK-W0050"
        )));
    }

    #[test]
    fn test_fix_filtered_returns_none_for_no_matching_rules() {
        let uri = Url::parse("file:///test.py").unwrap();
        let source = "x: int = 42\n";
        let diags = vec![make_diag("BSK-W0050", 0, 0, 1)];
        // Only allow E0001 — W0050 should be filtered out.
        let action = fix_filtered_in_file(&uri, &diags, source, &["BSK-E0001"]);
        assert!(
            action.is_none(),
            "should return None when no diagnostics match allowed rules"
        );
    }
}
