use super::*;
use crate::diagnostic::RuleMode;

#[test]
fn parse_type_ignore() {
    let overrides = parse_source_overrides("from fastmcp import FastMCP  # type: ignore\n");
    assert_eq!(overrides.line_overrides.len(), 1);
    assert_eq!(overrides.line_overrides[0].0, 0);
    assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Ignore);
    assert!(overrides.line_overrides[0].1.codes.is_empty());
}

#[test]
fn parse_type_ignore_with_code() {
    let overrides =
        parse_source_overrides("from fastmcp import FastMCP  # type: ignore[imports_unresolved]\n");
    assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Ignore);
    assert_eq!(
        overrides.line_overrides[0].1.codes,
        vec!["imports_unresolved"]
    );
}

#[test]
fn parse_type_warning() {
    let overrides = parse_source_overrides(
        "from fastmcp import FastMCP  # type: warning[imports_unresolved]\n",
    );
    assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Warning);
    assert_eq!(
        overrides.line_overrides[0].1.codes,
        vec!["imports_unresolved"]
    );
}

#[test]
fn parse_type_disabled_and_info() {
    let disabled = parse_source_overrides(
        "from fastmcp import FastMCP  # type: disabled[imports_unresolved]\n",
    );
    assert_eq!(disabled.line_overrides[0].1.mode, RuleMode::Disabled);

    let info = parse_source_overrides("from fastmcp import FastMCP  # type: info\n");
    assert_eq!(info.line_overrides[0].1.mode, RuleMode::Info);
    assert!(info.line_overrides[0].1.codes.is_empty());
}

#[test]
fn two_type_directives_on_one_line_are_both_parsed() {
    let source =
        "x: int = \"hi\"  # type: ignore[BSK-9999]  # type: warning[assignment_compatibility]\n";
    let overrides = parse_source_overrides(source);
    let line = overrides
        .line_overrides
        .iter()
        .filter(|(index, _)| *index == 0)
        .map(|(_, line_override)| line_override)
        .collect::<Vec<_>>();
    assert_eq!(line.len(), 2);
    assert!(line.iter().any(|line_override| {
        line_override.mode == RuleMode::Ignore && line_override.codes == ["BSK-9999"]
    }));
    assert!(line.iter().any(|line_override| {
        line_override.mode == RuleMode::Warning
            && line_override.codes == ["assignment_compatibility"]
    }));
}

#[test]
fn foreign_ignore_bracket_is_blanket_but_basilisk_code_is_specific() {
    let foreign = parse_source_overrides("z: int = \"\"  # type: ignore[additional_stuff]\n");
    assert!(foreign.line_overrides[0].1.codes.is_empty());
    assert!(override_matches(
        "assignment_compatibility",
        &foreign.line_overrides[0].1.codes
    ));

    let specific = parse_source_overrides("x = foo()  # type: ignore[imports_unresolved]\n");
    assert_eq!(
        specific.line_overrides[0].1.codes,
        vec!["imports_unresolved"]
    );
    assert!(!override_matches(
        "assignment_compatibility",
        &specific.line_overrides[0].1.codes
    ));
}

#[test]
fn leading_standalone_ignore_is_file_level() {
    let source = "#!/usr/bin/env python\n\n# type: ignore\n\n\"\"\"Doc.\"\"\"\n\nx: int = \"\"\n";
    let overrides = parse_source_overrides(source);
    match &overrides.file_mode {
        Some(FileOverride::Specific { mode, codes }) => {
            assert_eq!(*mode, RuleMode::Ignore);
            assert!(codes.is_empty());
        }
        other => panic!("expected file-level ignore, got {other:?}"),
    }
}

#[test]
fn standalone_ignore_after_code_is_not_file_level() {
    let source = "\"\"\"Doc.\"\"\"\n\n# type: ignore\n\nx: int = \"\"\n";
    let overrides = parse_source_overrides(source);
    assert!(overrides.file_mode.is_none());
}

#[test]
fn parse_relaxed_and_file_specific_directives() {
    let relaxed = parse_source_overrides("# basilisk: relaxed\nimport os\n");
    assert!(matches!(relaxed.file_mode, Some(FileOverride::Relaxed)));

    let disabled =
        parse_source_overrides("# basilisk: file-disabled[imports_unresolved]\nimport fastmcp\n");
    match &disabled.file_mode {
        Some(FileOverride::Specific { mode, codes }) => {
            assert_eq!(*mode, RuleMode::Disabled);
            assert_eq!(codes, &["imports_unresolved"]);
        }
        other => panic!("expected specific file override, got {other:?}"),
    }

    let warning = parse_source_overrides(
        "# basilisk: file-warning[imports_unresolved, returns_compatibility]\nimport fastmcp\n",
    );
    match &warning.file_mode {
        Some(FileOverride::Specific { mode, codes }) => {
            assert_eq!(*mode, RuleMode::Warning);
            assert_eq!(codes, &["imports_unresolved", "returns_compatibility"]);
        }
        other => panic!("expected specific file override, got {other:?}"),
    }
}

#[test]
fn parse_block_and_multiple_codes() {
    let block = "# type: disabled[imports_unresolved]\nfrom fastmcp import FastMCP\nfrom result import Result\n# type: end-disabled[imports_unresolved]\nimport os\n";
    let overrides = parse_source_overrides(block);
    assert_eq!(overrides.block_overrides.len(), 1);
    let (start, end, data) = &overrides.block_overrides[0];
    assert_eq!((*start, *end), (0, 3));
    assert_eq!(data.mode, RuleMode::Disabled);
    assert_eq!(data.codes, vec!["imports_unresolved"]);

    let multiple = parse_source_overrides(
        "x = foo()  # type: ignore[imports_unresolved, calls_argument_type]\n",
    );
    assert_eq!(
        multiple.line_overrides[0].1.codes,
        vec!["imports_unresolved", "calls_argument_type"]
    );
}

#[test]
fn override_matching_handles_blanket_and_specific() {
    assert!(override_matches("imports_unresolved", &[]));
    assert!(override_matches(
        "imports_unresolved",
        &["imports_unresolved".to_owned()]
    ));
    assert!(!override_matches(
        "returns_compatibility",
        &["imports_unresolved".to_owned()]
    ));
}

#[test]
fn byte_offset_to_line_uses_zero_based_lines() {
    assert_eq!(byte_offset_to_line_in_source("hello\nworld\n", 0), 0);
    assert_eq!(byte_offset_to_line_in_source("hello\nworld\n", 6), 1);
    assert_eq!(byte_offset_to_line_in_source("hello\nworld\n", 11), 1);
}

#[test]
fn ordinary_code_and_string_literals_do_not_create_directives() {
    let ordinary = parse_source_overrides("x = 1\ny = 2\n");
    assert!(ordinary.line_overrides.is_empty());
    assert!(ordinary.block_overrides.is_empty());
    assert!(ordinary.file_mode.is_none());

    let string = parse_source_overrides("x: int = '# type: ignore'\n");
    assert!(string.line_overrides.is_empty());
    assert!(string.block_overrides.is_empty());
    assert!(string.file_mode.is_none());
}

#[test]
fn malformed_suppression_syntax_never_creates_an_override() {
    for source in [
        "x: int = \"bad\"  # type: ignoree\n",
        "x: int = \"bad\"  # type: warningg\n",
        "x: int = \"bad\"  # type: warnin[assignment_compatibility]\n",
        "x: int = \"bad\"  # type: infoo\n",
        "x: int = \"bad\"  # type: disable[assignment_compatibility]\n",
        // A structurally malformed *bracket* selector is still an error: an
        // unclosed `[` or an empty `[]` cannot select any codes. (Trailing text
        // after a *closed* `]` is a comment, not junk — see the ignore tests.)
        "x: int = \"bad\"  # type: ignore[assignment_compatibility\n",
        "x: int = \"bad\"  # type: ignore[]\n",
    ] {
        let overrides = parse_source_overrides(source);
        assert!(
            overrides.line_overrides.is_empty(),
            "malformed directive was applied: {source}"
        );
    }
}

/// PEP 484 `# type: ignore` keeps its blanket line-suppression semantics when
/// followed by free-form trailing text — `- reason`, `# comment`, or a bare
/// word (see the typing directives spec). Such text must silence every error on
/// the line, never be demoted to a false positive. See
/// `directives_type_ignore.py` in the conformance suite.
#[test]
fn ignore_with_trailing_text_is_a_blanket_suppression() {
    for source in [
        "x: int = \"bad\"  # type: ignore - additional stuff\n",
        "x: int = \"bad\"  # type: ignore # other comment\n",
        "x: int = \"bad\"  # type: ignore assignment_compatibility\n",
        // Foreign bracket → blanket; trailing text after the closed `]` (here a
        // `# E?` marker) is a comment, not junk.
        "x: int = \"bad\"  # type: ignore[an-empty-str-is-not-an-int]  # E?\n",
    ] {
        let overrides = parse_source_overrides(source);
        assert_eq!(
            overrides.line_overrides.len(),
            1,
            "ignore directive did not create an override: {source}"
        );
        let line_override = &overrides.line_overrides[0].1;
        assert_eq!(line_override.mode, RuleMode::Ignore, "{source}");
        assert!(
            line_override.codes.is_empty(),
            "trailing text must be a blanket ignore (empty codes): {source}"
        );
        assert!(
            override_matches("assignment_compatibility", &line_override.codes),
            "blanket ignore must silence every code: {source}"
        );
    }
}

/// A closed all-Basilisk `[codes]` selector narrows the ignore to those codes,
/// and trailing text after the `]` (a comment) neither invalidates it nor
/// broadens it to a blanket ignore.
#[test]
fn ignore_bracket_selector_with_trailing_comment_stays_specific() {
    let overrides = parse_source_overrides(
        "x = foo()  # type: ignore[imports_unresolved]  # explanatory comment\n",
    );
    assert_eq!(overrides.line_overrides.len(), 1);
    let line_override = &overrides.line_overrides[0].1;
    assert_eq!(line_override.mode, RuleMode::Ignore);
    assert_eq!(line_override.codes, vec!["imports_unresolved"]);
    assert!(!override_matches(
        "assignment_compatibility",
        &line_override.codes
    ));
}

#[test]
fn pep_484_type_comments_do_not_create_suppression_overrides() {
    let source = "values = []  # type: list[int]\ndef render(value):\n    # type: (int) -> str\n    return str(value)\n";
    let overrides = parse_source_overrides(source);
    assert!(overrides.line_overrides.is_empty());
    assert!(overrides.block_overrides.is_empty());
    assert!(overrides.file_mode.is_none());
}

#[test]
fn misplaced_file_directives_do_not_create_file_overrides() {
    for source in [
        "marker = 1\n# basilisk: relaxed\n",
        "marker = 1\n# basilisk: file-disabled[assignment_compatibility]\n",
        "marker = 1\n# type: ignore\n",
    ] {
        assert!(parse_source_overrides(source).file_mode.is_none());
    }
}

#[test]
fn conflicting_line_directives_are_removed_without_auditing() {
    let source = "x: int = \"bad\"  # type: warning[assignment_compatibility]  # type: ignore[assignment_compatibility]\n";
    assert!(parse_source_overrides(source).line_overrides.is_empty());
}

#[test]
fn unclosed_blocks_are_removed_without_auditing() {
    let source = "# type: disabled[assignment_compatibility]\nx: int = \"bad\"\n";
    assert!(parse_source_overrides(source).block_overrides.is_empty());
}

#[test]
fn valid_line_and_closed_block_overrides_remain_available() {
    let line =
        parse_source_overrides("x: int = \"bad\"  # type: warning[assignment_compatibility]\n");
    assert_eq!(line.line_overrides.len(), 1);

    let block = parse_source_overrides(
        "# type: disabled[assignment_compatibility]\nx: int = \"bad\"\n# type: end-disabled[assignment_compatibility]\n",
    );
    assert_eq!(block.block_overrides.len(), 1);
}
