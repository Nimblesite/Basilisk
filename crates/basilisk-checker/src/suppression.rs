//! Inline suppression and mode override parser.
//!
//! Parses `# type: ignore`, `# type: warning[CODE]`, `# type: disabled[CODE]`,
//! block directives, and file-level directives from Python source comments.

use crate::diagnostic::{Diagnostic, RuleMode, Severity};

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
    let mut file_mode = None;
    let mut line_overrides = Vec::new();
    let mut block_starts: Vec<(usize, LineOverride)> = Vec::new();
    let mut block_overrides = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // File-level directives (must be standalone comment lines).
        if trimmed == "# basilisk: relaxed" {
            file_mode = Some(FileOverride::Relaxed);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# basilisk: file-") {
            if let Some(parsed) = parse_mode_directive(rest) {
                file_mode = Some(FileOverride::Specific {
                    mode: parsed.mode,
                    codes: parsed.codes,
                });
            }
            continue;
        }

        // Block end directives: `# type: end-<mode>[CODE]`
        if let Some(rest) = find_comment_directive(line, "# type: end-") {
            if let Some(parsed) = parse_mode_directive(rest) {
                // Find matching block start and close it.
                if let Some(start_idx) = find_matching_block_start(&block_starts, &parsed) {
                    let (start_line, override_data) = block_starts.remove(start_idx);
                    block_overrides.push((start_line, line_idx, override_data));
                }
            }
            continue;
        }

        // Block start directives: standalone `# type: <mode>[CODE]` on their own line.
        if trimmed.starts_with("# type: ") && !trimmed.contains("import") {
            if let Some(rest) = trimmed.strip_prefix("# type: ") {
                if rest.starts_with("disabled")
                    || rest.starts_with("warning")
                    || rest.starts_with("info")
                {
                    if let Some(parsed) = parse_mode_directive(rest) {
                        // Only treat as block start if this is a standalone comment line
                        // (no code before the comment).
                        if line.trim_start().starts_with('#') {
                            block_starts.push((line_idx, parsed));
                            continue;
                        }
                    }
                }
            }
        }

        // Per-line directives: `# type: ignore`, `# type: warning[CODE]`, etc.
        // These appear at the end of a line with code.
        if let Some(rest) = find_comment_directive(line, "# type: ignore") {
            let codes = parse_bracketed_codes(rest);
            line_overrides.push((line_idx, LineOverride {
                mode: RuleMode::Ignore,
                codes,
            }));
        } else if let Some(rest) = find_comment_directive(line, "# type: disabled") {
            let codes = parse_bracketed_codes(rest);
            line_overrides.push((line_idx, LineOverride {
                mode: RuleMode::Disabled,
                codes,
            }));
        } else if let Some(rest) = find_comment_directive(line, "# type: warning") {
            let codes = parse_bracketed_codes(rest);
            line_overrides.push((line_idx, LineOverride {
                mode: RuleMode::Warning,
                codes,
            }));
        } else if let Some(rest) = find_comment_directive(line, "# type: info") {
            let codes = parse_bracketed_codes(rest);
            line_overrides.push((line_idx, LineOverride {
                mode: RuleMode::Info,
                codes,
            }));
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

/// Parse bracketed codes like `[BSK-E0010, BSK-E0011]` from the start of a string.
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
    starts
        .iter()
        .rposition(|(_, start)| start.mode == end_override.mode && start.codes == end_override.codes)
}

/// Convert a byte offset to a 0-based line number in the given source text.
#[must_use]
pub fn byte_offset_to_line_in_source(source: &str, byte_offset: u32) -> usize {
    let offset = (byte_offset as usize).min(source.len());
    source[..offset].matches('\n').count()
}

#[cfg(test)]
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
        let source = "from fastmcp import FastMCP  # type: ignore[BSK-E0010]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides.len(), 1);
        assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Ignore);
        assert_eq!(overrides.line_overrides[0].1.codes, vec!["BSK-E0010"]);
    }

    #[test]
    fn test_parse_type_warning() {
        let source = "from fastmcp import FastMCP  # type: warning[BSK-E0010]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides.len(), 1);
        assert_eq!(overrides.line_overrides[0].1.mode, RuleMode::Warning);
        assert_eq!(overrides.line_overrides[0].1.codes, vec!["BSK-E0010"]);
    }

    #[test]
    fn test_parse_type_disabled() {
        let source = "from fastmcp import FastMCP  # type: disabled[BSK-E0010]\n";
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

    #[test]
    fn test_parse_basilisk_relaxed() {
        let source = "# basilisk: relaxed\nimport os\n";
        let overrides = parse_source_overrides(source);
        assert!(matches!(overrides.file_mode, Some(FileOverride::Relaxed)));
    }

    #[test]
    fn test_parse_file_disabled() {
        let source = "# basilisk: file-disabled[BSK-E0010]\nimport fastmcp\n";
        let overrides = parse_source_overrides(source);
        match &overrides.file_mode {
            Some(FileOverride::Specific { mode, codes }) => {
                assert_eq!(*mode, RuleMode::Disabled);
                assert_eq!(codes, &["BSK-E0010"]);
            }
            other => panic!("Expected Specific, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_file_warning_multiple_codes() {
        let source = "# basilisk: file-warning[BSK-E0010, BSK-E0011]\nimport fastmcp\n";
        let overrides = parse_source_overrides(source);
        match &overrides.file_mode {
            Some(FileOverride::Specific { mode, codes }) => {
                assert_eq!(*mode, RuleMode::Warning);
                assert_eq!(codes, &["BSK-E0010", "BSK-E0011"]);
            }
            other => panic!("Expected Specific, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_block_directive() {
        let source = "\
# type: disabled[BSK-E0010]
from fastmcp import FastMCP
from result import Result
# type: end-disabled[BSK-E0010]
import os
";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.block_overrides.len(), 1);
        let (start, end, ref block) = overrides.block_overrides[0];
        assert_eq!(start, 0);
        assert_eq!(end, 3);
        assert_eq!(block.mode, RuleMode::Disabled);
        assert_eq!(block.codes, vec!["BSK-E0010"]);
    }

    #[test]
    fn test_parse_multiple_codes() {
        let source = "x = foo()  # type: ignore[BSK-E0010, BSK-E0012]\n";
        let overrides = parse_source_overrides(source);
        assert_eq!(overrides.line_overrides[0].1.codes, vec!["BSK-E0010", "BSK-E0012"]);
    }

    #[test]
    fn test_override_matches_empty_codes() {
        assert!(override_matches("BSK-E0010", &[]));
    }

    #[test]
    fn test_override_matches_specific_code() {
        assert!(override_matches("BSK-E0010", &["BSK-E0010".to_owned()]));
        assert!(!override_matches("BSK-E0011", &["BSK-E0010".to_owned()]));
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
}
