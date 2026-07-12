use std::collections::BTreeSet;

use basilisk_resolver::Span;

use crate::diagnostic::RuleMode;

use super::model::{Boundary, Directive, Scope, Selector};

const TYPE_MARKER: &str = "# type: ";

#[derive(Clone, Copy)]
struct Comment<'a> {
    text: &'a str,
    span: Span,
}

struct ParsedToken {
    boundary: Boundary,
    mode: Option<RuleMode>,
    selector: Selector,
    problem: Option<String>,
}

pub(super) fn parse_directives(
    source: &str,
    comment_ranges: &[Span],
    known_codes: &BTreeSet<&str>,
) -> Vec<Directive> {
    let comments = comments_by_line(source, comment_ranges);
    let mut directives = Vec::new();
    let mut seen_substantial = false;

    for (line_index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let Some(comment) = comments.get(line_index).and_then(|entry| *entry) else {
            if !trimmed.is_empty() {
                seen_substantial = true;
            }
            continue;
        };
        let standalone = trimmed == comment.text.trim();
        let exact = comment.text.trim();

        if exact == "# type: ignore" && standalone && !seen_substantial {
            directives.push(valid_directive(
                comment.span,
                Scope::File,
                Boundary::Ordinary,
                RuleMode::Ignore,
                Selector::Blanket,
            ));
        } else if exact.starts_with("# basilisk:") {
            directives.push(parse_basilisk_directive(
                comment,
                line_index,
                standalone,
                seen_substantial,
                known_codes,
            ));
        } else {
            parse_type_directives(
                comment,
                line_index,
                standalone,
                known_codes,
                &mut directives,
            );
        }

        if !trimmed.is_empty() && !standalone {
            seen_substantial = true;
        }
    }

    pair_blocks(&mut directives);
    mark_conflicts(&mut directives);
    directives
}

fn comments_by_line<'a>(source: &'a str, ranges: &[Span]) -> Vec<Option<Comment<'a>>> {
    let line_index = basilisk_common::text::LineIndex::new(source);
    let mut comments = vec![None; source.lines().count()];
    for span in ranges {
        let line = line_index.line(span.start_usize()).saturating_sub(1);
        if let (Some(slot), Some(text)) = (comments.get_mut(line), span.slice_source(source)) {
            *slot = Some(Comment { text, span: *span });
        }
    }
    comments
}

fn parse_basilisk_directive(
    comment: Comment<'_>,
    line: usize,
    standalone: bool,
    seen_substantial: bool,
    known_codes: &BTreeSet<&str>,
) -> Directive {
    let text = comment.text.trim();
    if !standalone {
        return malformed(comment.span, Scope::Line(line), "file directive must be standalone");
    }
    if seen_substantial {
        return malformed(
            comment.span,
            Scope::File,
            "file directive must appear before executable source",
        );
    }
    if text == "# basilisk: relaxed" {
        return valid_directive(
            comment.span,
            Scope::File,
            Boundary::Ordinary,
            RuleMode::Warning,
            Selector::Blanket,
        );
    }
    let Some(body) = text.strip_prefix("# basilisk: file-") else {
        return malformed(comment.span, Scope::File, "unsupported basilisk directive");
    };
    let parsed = parse_token(body, known_codes);
    directive_from_token(comment.span, Scope::File, parsed)
}

fn parse_type_directives(
    comment: Comment<'_>,
    line: usize,
    standalone: bool,
    known_codes: &BTreeSet<&str>,
    out: &mut Vec<Directive>,
) {
    let starts = comment
        .text
        .match_indices(TYPE_MARKER)
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    for (index, position) in starts.iter().copied().enumerate() {
        let body_start = position + TYPE_MARKER.len();
        let body_end = starts.get(index + 1).copied().unwrap_or(comment.text.len());
        let Some(body) = comment.text.get(body_start..body_end) else {
            continue;
        };
        let parsed = parse_token(body, known_codes);
        let scope = scope_for(&parsed, line, standalone);
        let span = subspan(comment.span, position, body_end);
        out.push(directive_from_token(span, scope, parsed));
    }
}

fn scope_for(parsed: &ParsedToken, line: usize, standalone: bool) -> Scope {
    match parsed.boundary {
        Boundary::BlockEnd => Scope::Block {
            start: line,
            end: Some(line),
        },
        Boundary::Ordinary
            if standalone
                && matches!(
                    parsed.mode,
                    Some(RuleMode::Disabled | RuleMode::Warning | RuleMode::Info)
                ) =>
        {
            Scope::Block {
                start: line,
                end: None,
            }
        }
        _ => Scope::Line(line),
    }
}

fn parse_token(text: &str, known_codes: &BTreeSet<&str>) -> ParsedToken {
    let trimmed = text.trim();
    let verb_end = trimmed
        .find(|character: char| character.is_whitespace() || character == '[')
        .unwrap_or(trimmed.len());
    let (raw_verb, rest) = trimmed.split_at(verb_end);
    let (boundary, verb) = raw_verb
        .strip_prefix("end-")
        .map_or((Boundary::Ordinary, raw_verb), |verb| {
            (Boundary::BlockEnd, verb)
        });
    let mode = match verb {
        "ignore" => Some(RuleMode::Ignore),
        "disabled" => Some(RuleMode::Disabled),
        "warning" => Some(RuleMode::Warning),
        "info" => Some(RuleMode::Info),
        _ => None,
    };
    let Some(mode) = mode else {
        return ParsedToken {
            boundary,
            mode: None,
            selector: Selector::Blanket,
            problem: Some(format!("unknown directive verb `{raw_verb}`")),
        };
    };
    let (selector, problem) = parse_selector(rest, mode, known_codes);
    let boundary = if boundary == Boundary::Ordinary {
        Boundary::Ordinary
    } else {
        Boundary::BlockEnd
    };
    ParsedToken {
        boundary,
        mode: Some(mode),
        selector,
        problem,
    }
}

fn parse_selector(
    rest: &str,
    mode: RuleMode,
    known_codes: &BTreeSet<&str>,
) -> (Selector, Option<String>) {
    let rest = rest.trim();
    if rest.is_empty() || rest.starts_with("--") {
        return (Selector::Blanket, None);
    }
    let Some(after_open) = rest.strip_prefix('[') else {
        return (
            Selector::Blanket,
            Some("unexpected content after directive verb".to_owned()),
        );
    };
    let Some(close) = after_open.find(']') else {
        return (
            Selector::Blanket,
            Some("missing closing `]`".to_owned()),
        );
    };
    let codes = after_open[..close]
        .split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if codes.is_empty() {
        return (
            Selector::Blanket,
            Some("rule selector cannot be empty".to_owned()),
        );
    }

    let unknown = codes
        .iter()
        .filter(|code| !known_codes.contains(code.as_str()))
        .collect::<Vec<_>>();
    if unknown.is_empty() {
        return (Selector::Specific(codes), None);
    }
    if mode == RuleMode::Ignore && unknown.iter().all(|code| !looks_like_basilisk_code(code)) {
        return (Selector::Blanket, None);
    }
    let unknown_code = unknown.first().map_or("<unknown>", |code| code.as_str());
    (
        Selector::Specific(codes),
        Some(format!("unknown Basilisk rule code `{unknown_code}`")),
    )
}

fn looks_like_basilisk_code(code: &str) -> bool {
    code.starts_with("BSK-") || code.contains('_')
}

fn directive_from_token(span: Span, mut scope: Scope, parsed: ParsedToken) -> Directive {
    let boundary = if parsed.boundary == Boundary::Ordinary
        && matches!(scope, Scope::Block { end: None, .. })
    {
        Boundary::BlockStart
    } else {
        parsed.boundary
    };
    if boundary == Boundary::BlockEnd {
        scope = match scope {
            Scope::Block { start, .. } => Scope::Block {
                start,
                end: Some(start),
            },
            other => other,
        };
    }
    Directive {
        span,
        scope,
        boundary,
        mode: parsed.mode,
        selector: parsed.selector,
        problem: parsed.problem,
        paired_with: None,
        changed_diagnostics: 0,
    }
}

fn valid_directive(
    span: Span,
    scope: Scope,
    boundary: Boundary,
    mode: RuleMode,
    selector: Selector,
) -> Directive {
    Directive {
        span,
        scope,
        boundary,
        mode: Some(mode),
        selector,
        problem: None,
        paired_with: None,
        changed_diagnostics: 0,
    }
}

fn malformed(span: Span, scope: Scope, problem: &str) -> Directive {
    Directive {
        span,
        scope,
        boundary: Boundary::Ordinary,
        mode: None,
        selector: Selector::Blanket,
        problem: Some(problem.to_owned()),
        paired_with: None,
        changed_diagnostics: 0,
    }
}

fn subspan(parent: Span, relative_start: usize, relative_end: usize) -> Span {
    let start = parent.start_usize().saturating_add(relative_start);
    let end = parent.start_usize().saturating_add(relative_end);
    match (u32::try_from(start), u32::try_from(end)) {
        (Ok(start), Ok(end)) => Span::new(start, end),
        _ => parent,
    }
}

fn pair_blocks(directives: &mut [Directive]) {
    let mut starts = Vec::<usize>::new();
    for index in 0..directives.len() {
        match directives[index].boundary {
            Boundary::BlockStart if directives[index].is_valid() => starts.push(index),
            Boundary::BlockEnd if directives[index].is_valid() => {
                let matching = starts.iter().rposition(|start| {
                    directives[*start].mode == directives[index].mode
                        && directives[*start].selector == directives[index].selector
                });
                if let Some(position) = matching {
                    let start_index = starts.remove(position);
                    let start_line = match directives[start_index].scope {
                        Scope::Block { start, .. } => start,
                        _ => 0,
                    };
                    let end_line = match directives[index].scope {
                        Scope::Block { start, .. } => start,
                        _ => start_line,
                    };
                    directives[start_index].scope = Scope::Block {
                        start: start_line,
                        end: Some(end_line),
                    };
                    directives[index].scope = directives[start_index].scope;
                    directives[start_index].paired_with = Some(index);
                    directives[index].paired_with = Some(start_index);
                } else {
                    directives[index].problem = Some("unmatched block end directive".to_owned());
                }
            }
            _ => {}
        }
    }
    for index in starts {
        directives[index].problem = Some("unclosed block directive".to_owned());
    }
}

fn mark_conflicts(directives: &mut [Directive]) {
    for left in 0..directives.len() {
        for right in left + 1..directives.len() {
            if !directives[left].is_valid() || !directives[right].is_valid() {
                continue;
            }
            let same_scope = match (directives[left].scope, directives[right].scope) {
                (Scope::Line(a), Scope::Line(b)) => a == b,
                (Scope::File, Scope::File) => true,
                _ => false,
            };
            if same_scope
                && directives[left].mode != directives[right].mode
                && directives[left].selector.overlaps(&directives[right].selector)
            {
                let problem = "conflicting directives at the same scope".to_owned();
                directives[left].problem = Some(problem.clone());
                directives[right].problem = Some(problem);
            }
        }
    }
}
