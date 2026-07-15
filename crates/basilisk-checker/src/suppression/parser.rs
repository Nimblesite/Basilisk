//! Single-pass parser for applied overrides and their optional audit ledger.

use ruff_python_ast::{token::TokenKind, PySourceType};
use ruff_text_size::Ranged;

use crate::diagnostic::RuleMode;
use crate::suppression_audit::model::{self, Boundary, Scope};

use super::syntax::{
    directive_problem, find_all_type_directives, find_block_end_directive,
    find_matching_block_start, known_rule_codes, maybe_push_audit_directive, maybe_push_malformed,
    pair_audit_block, parse_line_directive, parse_mode_directive, LineDirectiveParse,
    SourceComment,
};
use super::{FileOverride, LineOverride, SourceOverrides};

/// Parse all inline overrides from source text, tokenizing comments on demand.
#[must_use]
pub fn parse_source_overrides(source: &str) -> SourceOverrides {
    let parsed = ruff_python_parser::parse_unchecked_source(source, PySourceType::Python);
    let comment_ranges = parsed
        .tokens()
        .iter()
        .filter(|token| token.kind() == TokenKind::Comment)
        .map(|token| token.range().into())
        .collect::<Vec<basilisk_resolver::Span>>();
    parse_source_overrides_with_comments(source, &comment_ranges, false)
}

/// Parse ranges already classified as real Python comments.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "one ordered pass preserves existing file, block, and line directive precedence"
)]
pub(crate) fn parse_source_overrides_with_comments(
    source: &str,
    comment_ranges: &[basilisk_resolver::Span],
    collect_audit: bool,
) -> SourceOverrides {
    if !contains_override_directive(source, comment_ranges) {
        return SourceOverrides::empty();
    }

    let known_codes = collect_audit.then(known_rule_codes);
    let mut file_mode = None;
    let mut file_audit_index = None;
    let mut line_overrides = Vec::new();
    let mut line_audit_indices = Vec::new();
    let mut block_starts: Vec<(usize, LineOverride, Option<usize>)> = Vec::new();
    let mut block_overrides = Vec::new();
    let mut block_audit_indices = Vec::new();
    let mut audit_directives = Vec::new();
    let mut seen_substantial = false;
    let comments = comments_by_line(source, comment_ranges);

    for (line_index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let comment = comments.get(line_index).and_then(|entry| *entry);
        let trimmed_comment = comment.map(|entry| entry.text.trim());
        let standalone = trimmed_comment == Some(trimmed);

        if trimmed_comment == Some("# type: ignore") && standalone {
            let Some(comment) = comment else {
                continue;
            };
            if file_mode.is_none() && !seen_substantial {
                file_audit_index = maybe_push_audit_directive(
                    collect_audit,
                    &mut audit_directives,
                    comment.span,
                    Scope::File,
                    Boundary::Ordinary,
                    Some(RuleMode::Ignore),
                    &[],
                    None,
                );
                file_mode = Some(FileOverride::Specific {
                    mode: RuleMode::Ignore,
                    codes: Vec::new(),
                });
            } else {
                maybe_push_malformed(
                    collect_audit,
                    &mut audit_directives,
                    comment.span,
                    Scope::File,
                    "file directive must appear before code",
                );
            }
            continue;
        }
        if !trimmed.is_empty() && !standalone {
            seen_substantial = true;
        }

        if trimmed_comment == Some("# basilisk: relaxed") && standalone {
            let Some(comment) = comment else {
                continue;
            };
            if seen_substantial {
                maybe_push_malformed(
                    collect_audit,
                    &mut audit_directives,
                    comment.span,
                    Scope::File,
                    "file directive must appear before code",
                );
            } else {
                file_audit_index = maybe_push_audit_directive(
                    collect_audit,
                    &mut audit_directives,
                    comment.span,
                    Scope::File,
                    Boundary::Ordinary,
                    Some(RuleMode::Warning),
                    &[],
                    None,
                );
                file_mode = Some(FileOverride::Relaxed);
            }
            continue;
        }
        if let Some(rest) = trimmed_comment
            .filter(|_| standalone)
            .and_then(|text| text.strip_prefix("# basilisk: file-"))
        {
            if let Ok(parsed) = parse_mode_directive(rest) {
                let problem = known_codes.and_then(|known| directive_problem(rest, &parsed, known));
                let Some(comment) = comment else {
                    continue;
                };
                let placement_problem =
                    seen_substantial.then(|| "file directive must appear before code".to_owned());
                let problem = problem.or(placement_problem);
                let applies = problem.is_none();
                file_audit_index = maybe_push_audit_directive(
                    collect_audit,
                    &mut audit_directives,
                    comment.span,
                    Scope::File,
                    Boundary::Ordinary,
                    Some(parsed.mode),
                    &parsed.codes,
                    problem,
                );
                if applies {
                    file_mode = Some(FileOverride::Specific {
                        mode: parsed.mode,
                        codes: parsed.codes,
                    });
                }
            } else if let Some(comment) = comment {
                maybe_push_malformed(
                    collect_audit,
                    &mut audit_directives,
                    comment.span,
                    Scope::File,
                    "unknown file directive verb",
                );
            }
            continue;
        }
        if trimmed_comment.is_some_and(|text| text.starts_with("# basilisk:")) {
            if let Some(comment) = comment {
                let problem = if standalone {
                    "unsupported basilisk directive"
                } else {
                    "file directive must be standalone"
                };
                maybe_push_malformed(
                    collect_audit,
                    &mut audit_directives,
                    comment.span,
                    Scope::File,
                    problem,
                );
            }
            continue;
        }

        if let Some(segment) = comment.and_then(find_block_end_directive) {
            if let Ok(parsed) = parse_mode_directive(segment.body) {
                let problem =
                    known_codes.and_then(|known| directive_problem(segment.body, &parsed, known));
                let end_audit_index = maybe_push_audit_directive(
                    collect_audit,
                    &mut audit_directives,
                    segment.span,
                    Scope::Block {
                        start: line_index,
                        end: Some(line_index),
                    },
                    Boundary::BlockEnd,
                    Some(parsed.mode),
                    &parsed.codes,
                    problem,
                );
                if let Some(start_position) = find_matching_block_start(&block_starts, &parsed) {
                    let (start_line, override_data, start_audit_index) =
                        block_starts.remove(start_position);
                    block_overrides.push((start_line, line_index, override_data));
                    if let Some(index) = start_audit_index {
                        block_audit_indices.push(index);
                    }
                    if let (Some(start_index), Some(end_index)) =
                        (start_audit_index, end_audit_index)
                    {
                        pair_audit_block(
                            &mut audit_directives,
                            start_index,
                            end_index,
                            start_line,
                            line_index,
                        );
                    }
                } else if let Some(directive) =
                    end_audit_index.and_then(|index| audit_directives.get_mut(index))
                {
                    directive.problem = Some("unmatched block end directive".to_owned());
                }
            } else {
                maybe_push_malformed(
                    collect_audit,
                    &mut audit_directives,
                    segment.span,
                    Scope::Block {
                        start: line_index,
                        end: Some(line_index),
                    },
                    "unknown block end directive verb",
                );
            }
            continue;
        }

        if standalone && trimmed_comment.is_some_and(|text| text.starts_with("# type: ")) {
            let Some(rest) = trimmed_comment.and_then(|text| text.strip_prefix("# type: ")) else {
                continue;
            };
            match parse_line_directive(rest) {
                LineDirectiveParse::Valid(parsed) if parsed.mode != RuleMode::Ignore => {
                    let Some(comment) = comment else {
                        continue;
                    };
                    let problem =
                        known_codes.and_then(|known| directive_problem(rest, &parsed, known));
                    let applies = problem.is_none();
                    let audit_index = maybe_push_audit_directive(
                        collect_audit,
                        &mut audit_directives,
                        comment.span,
                        Scope::Block {
                            start: line_index,
                            end: None,
                        },
                        Boundary::BlockStart,
                        Some(parsed.mode),
                        &parsed.codes,
                        problem,
                    );
                    if applies {
                        block_starts.push((line_index, parsed, audit_index));
                    }
                    continue;
                }
                LineDirectiveParse::Malformed(problem) => {
                    if let Some(comment) = comment {
                        maybe_push_malformed(
                            collect_audit,
                            &mut audit_directives,
                            comment.span,
                            Scope::Block {
                                start: line_index,
                                end: None,
                            },
                            problem,
                        );
                    }
                    continue;
                }
                LineDirectiveParse::Valid(_) | LineDirectiveParse::TypeComment => {}
            }
        }

        for directive in comment.into_iter().flat_map(find_all_type_directives) {
            match parse_line_directive(directive.body) {
                LineDirectiveParse::Valid(line_override) => {
                    let problem = known_codes
                        .and_then(|known| directive_problem(directive.body, &line_override, known));
                    let applies = problem.is_none();
                    let audit_index = maybe_push_audit_directive(
                        collect_audit,
                        &mut audit_directives,
                        directive.span,
                        Scope::Line(line_index),
                        Boundary::Ordinary,
                        Some(line_override.mode),
                        &line_override.codes,
                        problem,
                    );
                    if applies {
                        line_overrides.push((line_index, line_override));
                        if let Some(index) = audit_index {
                            line_audit_indices.push(index);
                        }
                    }
                }
                LineDirectiveParse::Malformed(problem) => {
                    maybe_push_malformed(
                        collect_audit,
                        &mut audit_directives,
                        directive.span,
                        Scope::Line(line_index),
                        problem,
                    );
                }
                LineDirectiveParse::TypeComment => {}
            }
        }
    }

    let total_lines = source.lines().count();
    for (start_line, _, audit_index) in block_starts {
        if let Some(directive) = audit_index.and_then(|index| audit_directives.get_mut(index)) {
            directive.scope = Scope::Block {
                start: start_line,
                end: Some(total_lines),
            };
            directive.problem = Some("unclosed block directive".to_owned());
        }
    }
    if collect_audit {
        model::mark_conflicts(&mut audit_directives);
    }
    let conflicts = conflicting_line_overrides(&line_overrides);
    retain_non_conflicting(&mut line_overrides, &conflicts);
    retain_non_conflicting(&mut line_audit_indices, &conflicts);

    SourceOverrides {
        file_mode,
        line_overrides,
        block_overrides,
        audit_directives,
        line_audit_indices,
        block_audit_indices,
        file_audit_index,
    }
}

fn conflicting_line_overrides(overrides: &[(usize, LineOverride)]) -> Vec<bool> {
    let mut conflicts = vec![false; overrides.len()];
    for (left_index, (left_line, left)) in overrides.iter().enumerate() {
        for (right_index, (right_line, right)) in overrides.iter().enumerate().skip(left_index + 1)
        {
            if left_line == right_line
                && left.mode != right.mode
                && model::selectors_overlap(&left.codes, &right.codes)
            {
                model::mark_conflict_pair(&mut conflicts, left_index, right_index);
            }
        }
    }
    conflicts
}

fn retain_non_conflicting<T>(values: &mut Vec<T>, conflicts: &[bool]) {
    *values = std::mem::take(values)
        .into_iter()
        .enumerate()
        .filter_map(|(index, value)| {
            (!conflicts.get(index).copied().unwrap_or(false)).then_some(value)
        })
        .collect();
}

fn contains_override_directive(source: &str, ranges: &[basilisk_resolver::Span]) -> bool {
    ranges.iter().any(|range| {
        range
            .slice_source(source)
            .is_some_and(|comment| comment.contains("# type:") || comment.contains("# basilisk:"))
    })
}

fn comments_by_line<'a>(
    source: &'a str,
    ranges: &[basilisk_resolver::Span],
) -> Vec<Option<SourceComment<'a>>> {
    let line_index = basilisk_common::text::LineIndex::new(source);
    let mut comments = vec![None; source.lines().count()];
    for range in ranges {
        let line = line_index.line(range.start_usize()).saturating_sub(1);
        if let (Some(slot), Some(text)) = (comments.get_mut(line), range.slice_source(source)) {
            *slot = Some(SourceComment { text, span: *range });
        }
    }
    comments
}
