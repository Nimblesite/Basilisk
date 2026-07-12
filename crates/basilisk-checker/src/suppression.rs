//! Implements [CHKARCH-STRICTNESS-SUPPRESSION] / [STUBRES-SUPPRESSION]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-strictness-suppression
//! Inline suppression and mode override parser.
//!
//! Parses `# type: ignore`, `# type: warning[CODE]`, `# type: disabled[CODE]`,
//! block directives, and file-level directives from Python source comments.

use crate::diagnostic::{Diagnostic, RuleMode, Severity};
use ruff_python_ast::{token::TokenKind, PySourceType};
use ruff_text_size::Ranged;

/// All parsed overrides for a source file.
#[derive(Debug)]
pub struct SourceOverrides {
    /// File-level mode: `# basilisk: relaxed` or `# basilisk: file-disabled[CODE]`.
    pub file_mode: Option<FileOverride>,
    /// Per-line overrides keyed by 0-based line number.
    pub line_overrides: Vec<(usize, LineOverride)>,
    /// Block overrides: (start line, end line, override data).
    pub block_overrides: Vec<(usize, usize, LineOverride)>,
}

/// A file-level override directive.
#[derive(Debug, Clone)]
pub enum FileOverride {
    /// `# basilisk: relaxed` — all errors become warnings.
    Relaxed,
    /// `# basilisk: file-<mode>[CODE, ...]` — override specific rules for the file.
    Specific {
        /// The mode to apply to matching rules.
        mode: RuleMode,
        /// Specific rule codes this applies to.
        codes: Vec<String>,
    },
}

/// A per-line or per-block override.
#[derive(Debug, Clone)]
pub struct LineOverride {
    /// The mode to apply.
    pub mode: RuleMode,
    /// Specific rule codes, or empty for all rules.
    pub codes: Vec<String>,
}

/// Parse all inline overrides from the source text.
#[must_use]
pub fn parse_source_overrides(source: &str) -> SourceOverrides {
    let parsed = ruff_python_parser::parse_unchecked_source(source, PySourceType::Python);
    let comment_ranges: Vec<basilisk_resolver::Span> = parsed
        .tokens()
        .iter()
        .filter(|token| token.kind() == TokenKind::Comment)
        .map(|token| token.range().into())
        .collect();
    parse_source_overrides_with_comments(source, &comment_ranges)
}

/// Parse inline overrides from ranges already classified as Python comments.
///
/// The normal checker path passes the ranges retained by `basilisk-parser`, so
/// suppression does not parse the file a second time. The public source-only
/// helper above tokenizes on demand for callers that do not have a resolved
/// module.
#[must_use]
pub(crate) fn parse_source_overrides_with_comments(
    source: &str,
    comment_ranges: &[basilisk_resolver::Span],
) -> SourceOverrides {
    // Suppression directives are rare. Avoid allocating a per-line comment map
    // and rescanning the whole file unless a real Python comment contains a
    // directive marker. The parser-provided ranges preserve the string-literal
    // safety property while keeping the no-directive path proportional only to
    // the number of comments.
    if !contains_override_directive(source, comment_ranges) {
        return SourceOverrides::empty();
    }

    let mut file_mode = None;
    let mut line_overrides = Vec::new();
    let mut block_starts: Vec<(usize, LineOverride)> = Vec::new();
    let mut block_overrides = Vec::new();
    // Whether a docstring, import, or executable statement has been seen yet.
    // A file-level `# type: ignore` is only valid before any such line.
    let mut seen_substantial = false;
    let comments = comments_by_line(source, comment_ranges);

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let comment = comments.get(line_idx).and_then(|comment| *comment);
        let trimmed_comment = comment.map(str::trim);
        let is_standalone_comment = trimmed_comment == Some(trimmed);

        // File-level `# type: ignore` (PEP 484): a standalone `# type: ignore`
        // on its own line, before any docstring/import/executable code, silences
        // all errors in the file. Only blank lines and comments (shebang lines,
        // coding cookies) may precede it.
        if file_mode.is_none()
            && trimmed_comment == Some("# type: ignore")
            && is_standalone_comment
            && !seen_substantial
        {
            file_mode = Some(FileOverride::Specific {
                mode: RuleMode::Ignore,
                codes: Vec::new(),
            });
            continue;
        }
        if !trimmed.is_empty() && !is_standalone_comment {
            seen_substantial = true;
        }

        // File-level directives (must be standalone comment lines).
        if trimmed_comment == Some("# basilisk: relaxed") && is_standalone_comment {
            file_mode = Some(FileOverride::Relaxed);
            continue;
        }
        if let Some(rest) = trimmed_comment
            .filter(|_| is_standalone_comment)
            .and_then(|comment| comment.strip_prefix("# basilisk: file-"))
        {
            if let Some(parsed) = parse_mode_directive(rest) {
                file_mode = Some(FileOverride::Specific {
                    mode: parsed.mode,
                    codes: parsed.codes,
                });
            }
            continue;
        }

        // Block end directives: `# type: end-<mode>[CODE]`
        if let Some(rest) = comment.and_then(|text| find_comment_directive(text, "# type: end-")) {
            if let Some(parsed) = parse_mode_directive(rest) {
                // Find matching block start and close it.
                if let Some(start_idx) = find_matching_block_start(&block_starts, &parsed) {
                    let (start_line, override_data) = block_starts.remove(start_idx);
                    block_overrides.push((start_line, line_idx, override_data));
                }
            }
            continue;
        }

        // Block start directives: standalone `# type: <mode>[CODE]` on their own
        // line. The `# type: ` prefix on the *trimmed* line already guarantees a
        // standalone comment (a code-bearing line would start with the code).
        if is_standalone_comment && trimmed_comment.is_some_and(|text| text.starts_with("# type: "))
        {
            if let Some(rest) = trimmed_comment.and_then(|text| text.strip_prefix("# type: ")) {
                if rest.starts_with("disabled")
                    || rest.starts_with("warning")
                    || rest.starts_with("info")
                {
                    if let Some(parsed) = parse_mode_directive(rest) {
                        // Only treat as block start if this is a standalone comment line
                        // (no code before the comment).
                        block_starts.push((line_idx, parsed));
                        continue;
                    }
                }
            }
        }

        // Per-line directives: `# type: ignore`, `# type: warning[CODE]`, etc.
        // A single line may carry several directives (e.g. `ignore` one code
        // while `warning`-demoting another), so scan for EVERY `# type:` on the
        // line and apply each independently (issue #78).
        for directive in comment.into_iter().flat_map(find_all_type_directives) {
            if let Some(line_override) = parse_line_directive(directive) {
                line_overrides.push((line_idx, line_override));
            }
        }
    }

    // Close any unclosed block starts at EOF.
    let total_lines = source.lines().count();
    for (start_line, override_data) in block_starts {
        block_overrides.push((start_line, total_lines, override_data));
    }

    SourceOverrides {
        file_mode,
        line_overrides,
        block_overrides,
    }
}

impl SourceOverrides {
    pub(crate) fn is_empty(&self) -> bool {
        self.file_mode.is_none()
            && self.line_overrides.is_empty()
            && self.block_overrides.is_empty()
    }

    fn empty() -> Self {
        Self {
            file_mode: None,
            line_overrides: Vec::new(),
            block_overrides: Vec::new(),
        }
    }
}

fn contains_override_directive(source: &str, comment_ranges: &[basilisk_resolver::Span]) -> bool {
    comment_ranges.iter().any(|range| {
        range
            .slice_source(source)
            .is_some_and(|comment| comment.contains("# type:") || comment.contains("# basilisk:"))
    })
}

/// Return the actual Python comment token on each physical line.
///
/// Ruff's tokenizer distinguishes comments from `# type:` text embedded in
/// ordinary, raw, byte, triple-quoted, and interpolated string literals. Using
/// its token ranges here keeps suppression directives tied to Python comments
/// without maintaining a second, incomplete string lexer.
fn comments_by_line<'a>(
    source: &'a str,
    comment_ranges: &[basilisk_resolver::Span],
) -> Vec<Option<&'a str>> {
    let line_index = basilisk_common::text::LineIndex::new(source);
    let mut comments = vec![None; source.lines().count()];
    for range in comment_ranges {
        let line = line_index.line(range.start_usize()).saturating_sub(1);
        if let (Some(slot), Some(text)) = (comments.get_mut(line), range.slice_source(source)) {
            *slot = Some(text);
        }
    }
    comments
}

/// Apply all overrides to a diagnostic given its pre-computed line number.
///
/// Returns `None` if the diagnostic is suppressed or disabled,
/// or `Some(modified_diagnostic)` with adjusted severity.
///
/// Precedence: per-line > block > file (most specific wins).
#[must_use]
pub fn apply_overrides_at_line(
    mut diag: Diagnostic,
    diag_line: usize,
    overrides: &SourceOverrides,
) -> Option<Diagnostic> {
    // 1. Per-line overrides (highest priority).
    for (line_idx, line_override) in &overrides.line_overrides {
        if *line_idx == diag_line && override_matches(diag.code.code, &line_override.codes) {
            return apply_mode(diag, line_override.mode);
        }
    }

    // 2. Block overrides.
    for (start, end, block_override) in &overrides.block_overrides {
        if diag_line >= *start
            && diag_line <= *end
            && override_matches(diag.code.code, &block_override.codes)
        {
            return apply_mode(diag, block_override.mode);
        }
    }

    // 3. File-level overrides.
    if let Some(file_override) = &overrides.file_mode {
        match file_override {
            FileOverride::Relaxed => {
                if diag.severity == Severity::Error || diag.severity == Severity::SafetyViolation {
                    diag.severity = Severity::Warning;
                }
                return Some(diag);
            }
            FileOverride::Specific { mode, codes } => {
                if override_matches(diag.code.code, codes) {
                    return apply_mode(diag, *mode);
                }
            }
        }
    }

    Some(diag)
}

/// Apply a mode to a diagnostic, returning `None` for Ignore/Disabled.
fn apply_mode(mut diag: Diagnostic, mode: RuleMode) -> Option<Diagnostic> {
    match mode {
        RuleMode::Ignore | RuleMode::Disabled => None,
        RuleMode::Warning => {
            diag.severity = Severity::Warning;
            Some(diag)
        }
        RuleMode::Info => {
            diag.severity = Severity::Info;
            Some(diag)
        }
        RuleMode::Error => Some(diag),
    }
}

/// Check if a diagnostic code matches an override's code list.
/// Empty codes list means "all rules".
fn override_matches(diag_code: &str, codes: &[String]) -> bool {
    codes.is_empty() || codes.iter().any(|c| c == diag_code)
}

/// Find a comment directive in a line, returning the rest of the string after it.
fn find_comment_directive<'a>(line: &'a str, directive: &str) -> Option<&'a str> {
    line.find(directive)
        .map(|pos| &line[pos + directive.len()..])
}

/// Return each `# type:` directive segment on a line — the text after each
/// `# type: ` marker up to the next marker (or the end of the line).
///
/// Scanning for *every* marker (rather than stopping at the first) is what lets
/// one physical line apply different verbs to different codes (issue #78).
fn find_all_type_directives(line: &str) -> Vec<&str> {
    const MARKER: &str = "# type: ";
    let starts: Vec<usize> = line.match_indices(MARKER).map(|(pos, _)| pos).collect();
    starts
        .iter()
        .enumerate()
        .filter_map(|(idx, &pos)| {
            let seg_start = pos + MARKER.len();
            let seg_end = starts.get(idx + 1).copied().unwrap_or(line.len());
            line.get(seg_start..seg_end)
        })
        .collect()
}

/// Parse a single per-line directive segment (the verb and optional `[codes]`
/// following a `# type: ` marker) into a [`LineOverride`]. Returns `None` when
/// the verb is unrecognised. `ignore` keeps its PEP 484 semantics (a non-`BSK-`
/// bracket suppresses every code on the line).
fn parse_line_directive(directive: &str) -> Option<LineOverride> {
    let directive = directive.trim_start();
    if let Some(rest) = directive.strip_prefix("ignore") {
        Some(LineOverride {
            mode: RuleMode::Ignore,
            codes: parse_ignore_codes(rest),
        })
    } else if let Some(rest) = directive.strip_prefix("disabled") {
        Some(LineOverride {
            mode: RuleMode::Disabled,
            codes: parse_bracketed_codes(rest),
        })
    } else if let Some(rest) = directive.strip_prefix("warning") {
        Some(LineOverride {
            mode: RuleMode::Warning,
            codes: parse_bracketed_codes(rest),
        })
    } else {
        directive.strip_prefix("info").map(|rest| LineOverride {
            mode: RuleMode::Info,
            codes: parse_bracketed_codes(rest),
        })
    }
}

/// Parse the code list for a `# type: ignore[...]` directive.
///
/// Per the typing spec, a `# type: ignore` comment silences *all* errors on the
/// line; any bracketed content is type-checker-specific. Basilisk honours
/// code-specific suppression only when every bracketed token is a Basilisk code:
/// a conformance-test code (always `chapter_subtopic`, i.e. contains `_`) or a
/// `BSK-…` opt-in code. Any other content — mypy's `# type: ignore[assignment]`
/// or `[arg-type]` (hyphenated / single words, never underscored), an arbitrary
/// tag, or no brackets at all — suppresses every error on the line, as the spec
/// requires. (`Vec::new()` means "all rules" in `override_matches`.)
fn parse_ignore_codes(rest: &str) -> Vec<String> {
    let codes = parse_bracketed_codes(rest);
    if !codes.is_empty() && codes.iter().all(|code| is_basilisk_code(code)) {
        codes
    } else {
        Vec::new()
    }
}

/// Namespace prefixes of every Basilisk diagnostic code: the 21 python/typing
/// conformance chapters plus the cross-cutting core-check prefixes. A
/// conformance code is always `<prefix>_<subtopic>`.
const CODE_PREFIXES: &[&str] = &[
    // conformance-test chapters
    "aliases",
    "annotations",
    "callables",
    "classes",
    "constructors",
    "dataclasses",
    "directives",
    "enums",
    "exceptions",
    "generics",
    "historical",
    "literals",
    "namedtuples",
    "narrowing",
    "overloads",
    "protocols",
    "qualifiers",
    "specialtypes",
    "tuples",
    "typeddicts",
    "typeforms",
    // cross-cutting core checks
    "imports",
    "returns",
    "calls",
    "assignment",
    "names",
    "dict",
    "match",
    "version",
];

/// Whether a bracketed token is a Basilisk diagnostic code (as opposed to a
/// foreign mypy/pyright code). Basilisk codes are either `BSK-…` opt-in codes or
/// conformance-test codes of the form `<prefix>_<subtopic>` whose prefix is a
/// known Basilisk namespace. Foreign codes — mypy's `assignment`, `arg-type` —
/// are bare words or hyphenated and never match.
fn is_basilisk_code(code: &str) -> bool {
    if code.starts_with("BSK-") {
        return true;
    }
    matches!(code.split_once('_'), Some((prefix, _)) if CODE_PREFIXES.contains(&prefix))
}

/// Parse bracketed codes like `[imports_unresolved, returns_compatibility]` from the start of a string.
fn parse_bracketed_codes(rest: &str) -> Vec<String> {
    let rest = rest.trim();
    if !rest.starts_with('[') {
        return Vec::new();
    }
    let end = rest.find(']').unwrap_or(rest.len());
    rest[1..end]
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse a mode directive like `disabled[CODE, CODE]` or `warning[CODE]`.
fn parse_mode_directive(text: &str) -> Option<LineOverride> {
    let text = text.trim();
    let (mode, rest) = if let Some(rest) = text.strip_prefix("disabled") {
        (RuleMode::Disabled, rest)
    } else if let Some(rest) = text.strip_prefix("warning") {
        (RuleMode::Warning, rest)
    } else if let Some(rest) = text.strip_prefix("info") {
        (RuleMode::Info, rest)
    } else {
        return text.strip_prefix("ignore").map(|rest| LineOverride {
            mode: RuleMode::Ignore,
            codes: parse_bracketed_codes(rest),
        });
    };
    Some(LineOverride {
        mode,
        codes: parse_bracketed_codes(rest),
    })
}

/// Find the index of a matching block start for a block end directive.
fn find_matching_block_start(
    starts: &[(usize, LineOverride)],
    end_override: &LineOverride,
) -> Option<usize> {
    // Match by mode and codes (last matching start wins).
    starts.iter().rposition(|(_, start)| {
        start.mode == end_override.mode && start.codes == end_override.codes
    })
}

/// Convert a byte offset to a 0-based line number in the given source text.
#[must_use]
#[expect(
    clippy::as_conversions,
    reason = "u32 to usize is always safe on 32-bit+ targets"
)]
pub fn byte_offset_to_line_in_source(source: &str, byte_offset: u32) -> usize {
    let offset = (byte_offset as usize).min(source.len());
    source.get(..offset).map_or(0, |s| s.matches('\n').count())
}

#[cfg(test)]
#[expect(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Tests use assert macros (panic) and direct indexing for clarity"
)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_type_ignore() {
        let source = "from fastmcp import FastMCP  # type: ignore\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides.len(), 1);
        assert_eq!(overrides.line_overrides[0].0, 0);
        assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Ignore);
        assert!(overrides.line_overrides[0].1.codes.is_empty());
    }

    #[test]
    fn test_parse_type_ignore_with_code() {
        let source = "from fastmcp import FastMCP  # type: ignore[imports_unresolved]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides.len(), 1);
        assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Ignore);
        assert_eq!(
            overrides.line_overrides[0].1.codes,
            vec!["imports_unresolved"]
        );
    }

    #[test]
    fn test_parse_type_warning() {
        let source = "from fastmcp import FastMCP  # type: warning[imports_unresolved]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides.len(), 1);
        assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Warning);
        assert_eq!(
            overrides.line_overrides[0].1.codes,
            vec!["imports_unresolved"]
        );
    }

    #[test]
    fn test_parse_type_disabled() {
        let source = "from fastmcp import FastMCP  # type: disabled[imports_unresolved]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides.len(), 1);
        assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Disabled);
    }

    #[test]
    fn test_parse_type_info() {
        let source = "from fastmcp import FastMCP  # type: info\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides.len(), 1);
        assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Info);
        assert!(overrides.line_overrides[0].1.codes.is_empty());
    }

    /// Regression for issue #78: a line carrying TWO `# type:` directives must
    /// have BOTH parsed (e.g. `ignore` one code while `warning`-demoting
    /// another). Today only the first is honoured and the second is silently
    /// dropped. The bug is order-independent, so this asserts the directive set
    /// rather than positions.
    #[test]
    fn test_two_type_directives_on_one_line_both_parsed() {
        let source = "x: int = \"hi\"  # type: ignore[BSK-E9999]  # type: warning[assignment_compatibility]\n";
        let overrides = parse_source_overrides(source);
        let line0: Vec<&LineOverride> = overrides
            .line_overrides
            .iter()
            .filter(|(idx, _)| *idx == 0)
            .map(|(_, ov)| ov)
            .collect();
        assert_eq!(
            line0.len(),
            2,
            "both directives on the line must be parsed, got: {line0:?}"
        );
        assert!(
            line0
                .iter()
                .any(|ov| ov.mode == RuleMode::Ignore && ov.codes == ["BSK-E9999"]),
            "missing the `ignore[BSK-E9999]` directive: {line0:?}"
        );
        assert!(
            line0
                .iter()
                .any(|ov| ov.mode == RuleMode::Warning && ov.codes == ["assignment_compatibility"]),
            "missing the `warning[assignment_compatibility]` directive (silently dropped): {line0:?}"
        );
    }

    #[test]
    fn test_type_ignore_non_basilisk_bracket_suppresses_all() {
        // PEP 484: `# type: ignore[<anything>]` silences all errors on the line.
        // A non-Basilisk tag must not be treated as a code-specific filter.
        let source = "z: int = \"\"  # type: ignore[additional_stuff]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides.len(), 1);
        assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Ignore);
        assert!(
            overrides.line_overrides[0].1.codes.is_empty(),
            "non-Basilisk bracket content must suppress all rules"
        );
        // And it actually suppresses an arbitrary diagnostic code.
        assert!(override_matches(
            "assignment_compatibility",
            &overrides.line_overrides[0].1.codes
        ));
    }

    #[test]
    fn test_type_ignore_basilisk_bracket_stays_code_specific() {
        let source = "x = foo()  # type: ignore[imports_unresolved]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(
            overrides.line_overrides[0].1.codes,
            vec!["imports_unresolved"]
        );
        assert!(!override_matches(
            "assignment_compatibility",
            &overrides.line_overrides[0].1.codes
        ));
    }

    #[test]
    fn test_file_level_type_ignore_before_docstring() {
        // A standalone `# type: ignore` after only a shebang and blank line, before
        // the docstring, silences the whole file.
        let source =
            "#!/usr/bin/env python\n\n# type: ignore\n\n\"\"\"Doc.\"\"\"\n\nx: int = \"\"\n";
        let overrides = parse_source_overrides(source);
        match &overrides.file_mode {
            Some(FileOverride::Specific { mode, codes }) => {
                assert_eq!(*mode, RuleMode::Ignore);
                assert!(codes.is_empty());
            }
            other => panic!("Expected file-level Ignore, got {other:?}"),
        }
    }

    #[test]
    fn test_type_ignore_after_code_is_not_file_level() {
        // The same comment after a docstring/code is NOT file-level (it must still
        // report errors elsewhere in the file).
        let source = "\"\"\"Doc.\"\"\"\n\n# type: ignore\n\nx: int = \"\"\n";
        let overrides = parse_source_overrides(source);
        assert!(
            overrides.file_mode.is_none(),
            "comment after the docstring must not become a file-level directive"
        );
    }

    #[test]
    fn test_parse_basilisk_relaxed() {
        let source = "# basilisk: relaxed\nimport os\n";
        let overrides = parse_source_overrides(source);
        assert!(matches!(overrides.file_mode, Some(FileOverride::Relaxed)));
    }

    #[test]
    fn test_parse_file_disabled() {
        let source = "# basilisk: file-disabled[imports_unresolved]\nimport fastmcp\n";
        let overrides = parse_source_overrides(source);
        match &overrides.file_mode {
            Some(FileOverride::Specific { mode, codes }) => {
                assert_eq!(*mode, RuleMode::Disabled);
                assert_eq!(codes, &["imports_unresolved"]);
            }
            other => panic!("Expected Specific, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_file_warning_multiple_codes() {
        let source =
            "# basilisk: file-warning[imports_unresolved, returns_compatibility]\nimport fastmcp\n";
        let overrides = parse_source_overrides(source);
        match &overrides.file_mode {
            Some(FileOverride::Specific { mode, codes }) => {
                assert_eq!(*mode, RuleMode::Warning);
                assert_eq!(codes, &["imports_unresolved", "returns_compatibility"]);
            }
            other => panic!("Expected Specific, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_block_directive() {
        let source = "\
# type: disabled[imports_unresolved]
from fastmcp import FastMCP
from result import Result
# type: end-disabled[imports_unresolved]
import os
";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.block_overrides.len(), 1);
        let (start, end, ref block) = overrides.block_overrides[0];
        assert_eq!(start, 0);
        assert_eq!(end, 3);
        assert_eq!(block.mode, RuleMode::Disabled);
        assert_eq!(block.codes, vec!["imports_unresolved"]);
    }

    #[test]
    fn test_parse_multiple_codes() {
        let source = "x = foo()  # type: ignore[imports_unresolved, calls_argument_type]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(
            overrides.line_overrides[0].1.codes,
            vec!["imports_unresolved", "calls_argument_type"]
        );
    }

    #[test]
    fn test_override_matches_empty_codes() {
        assert!(override_matches("imports_unresolved", &[]));
    }

    #[test]
    fn test_override_matches_specific_code() {
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
    fn test_byte_offset_to_line() {
        assert_eq!(byte_offset_to_line_in_source("hello\nworld\n", 0), 0);
        assert_eq!(byte_offset_to_line_in_source("hello\nworld\n", 6), 1);
        assert_eq!(byte_offset_to_line_in_source("hello\nworld\n", 11), 1);
    }

    #[test]
    fn test_no_false_positive_on_code_lines() {
        // A line with actual code containing "# type: ignore" in a string should not match
        // as a suppression — but our simple parser does match it. This is acceptable because
        // the `# type: ignore` pattern in a comment position is the standard convention.
        let source = "x = 1\ny = 2\n";
        let overrides = parse_source_overrides(source);
        assert!(overrides.line_overrides.is_empty());
        assert!(overrides.block_overrides.is_empty());
        assert!(overrides.file_mode.is_none());
    }

    /// A directive-looking substring inside a Python string is data, not a
    /// comment, and must never suppress diagnostics on that line.
    #[test]
    fn type_ignore_inside_string_literal_is_not_a_directive() {
        let source = "x: int = '# type: ignore'\n";
        let overrides = parse_source_overrides(source);

        assert!(
            overrides.line_overrides.is_empty(),
            "string contents must not create line overrides: {overrides:?}"
        );
        assert!(overrides.block_overrides.is_empty());
        assert!(overrides.file_mode.is_none());
    }
}
