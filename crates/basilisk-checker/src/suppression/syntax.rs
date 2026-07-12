//! Shared directive syntax and audit-ledger helpers.

use crate::diagnostic::RuleMode;
use crate::suppression_audit::model::{Boundary, Directive, Scope, Selector};

use super::LineOverride;

#[derive(Debug, Clone, Copy)]
pub(super) struct SourceComment<'a> {
    pub(super) text: &'a str,
    pub(super) span: basilisk_resolver::Span,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TypeDirective<'a> {
    pub(super) body: &'a str,
    pub(super) span: basilisk_resolver::Span,
}

pub(super) enum LineDirectiveParse {
    Valid(LineOverride),
    Malformed(&'static str),
    TypeComment,
}

pub(super) fn find_block_end_directive(comment: SourceComment<'_>) -> Option<TypeDirective<'_>> {
    const MARKER: &str = "# type: end-";
    let position = comment.text.find(MARKER)?;
    let body_start = position + MARKER.len();
    Some(TypeDirective {
        body: comment.text.get(body_start..)?,
        span: directive_subspan(comment.span, position, comment.text.len()),
    })
}

/// Return every `# type:` segment in one real Python comment.
pub(super) fn find_all_type_directives(comment: SourceComment<'_>) -> Vec<TypeDirective<'_>> {
    const MARKER: &str = "# type: ";
    let starts = comment
        .text
        .match_indices(MARKER)
        .map(|(position, _)| position)
        .collect::<Vec<_>>();
    starts
        .iter()
        .enumerate()
        .filter_map(|(index, &position)| {
            let body_start = position + MARKER.len();
            let body_end = starts.get(index + 1).copied().unwrap_or(comment.text.len());
            Some(TypeDirective {
                body: comment.text.get(body_start..body_end)?,
                span: directive_subspan(comment.span, position, body_end),
            })
        })
        .collect()
}

#[expect(
    clippy::too_many_arguments,
    reason = "lossless directive records carry independent syntax and scope dimensions"
)]
pub(super) fn maybe_push_audit_directive(
    enabled: bool,
    directives: &mut Vec<Directive>,
    span: basilisk_resolver::Span,
    scope: Scope,
    boundary: Boundary,
    mode: Option<RuleMode>,
    codes: &[String],
    problem: Option<String>,
) -> Option<usize> {
    if !enabled {
        return None;
    }
    let selector = if codes.is_empty() {
        Selector::Blanket
    } else {
        Selector::Specific(codes.to_vec())
    };
    let index = directives.len();
    directives.push(Directive {
        span,
        scope,
        boundary,
        mode,
        selector,
        problem,
        paired_with: None,
        changed_diagnostics: 0,
    });
    Some(index)
}

pub(super) fn maybe_push_malformed(
    enabled: bool,
    directives: &mut Vec<Directive>,
    span: basilisk_resolver::Span,
    scope: Scope,
    problem: &str,
) {
    let _ = maybe_push_audit_directive(
        enabled,
        directives,
        span,
        scope,
        Boundary::Ordinary,
        None,
        &[],
        Some(problem.to_owned()),
    );
}

pub(super) fn known_rule_codes() -> &'static std::collections::BTreeSet<&'static str> {
    static CODES: std::sync::OnceLock<std::collections::BTreeSet<&'static str>> =
        std::sync::OnceLock::new();
    CODES.get_or_init(|| {
        crate::rule_catalog()
            .into_iter()
            .map(|rule| rule.code)
            .collect()
    })
}

pub(super) fn pair_audit_block(
    directives: &mut [Directive],
    start_index: usize,
    end_index: usize,
    start_line: usize,
    end_line: usize,
) {
    let scope = Scope::Block {
        start: start_line,
        end: Some(end_line),
    };
    if let Some(start) = directives.get_mut(start_index) {
        start.scope = scope;
        start.paired_with = Some(end_index);
    }
    if let Some(end) = directives.get_mut(end_index) {
        end.scope = scope;
        end.paired_with = Some(start_index);
    }
}

fn directive_subspan(
    parent: basilisk_resolver::Span,
    relative_start: usize,
    relative_end: usize,
) -> basilisk_resolver::Span {
    let start = parent.start_usize().saturating_add(relative_start);
    let end = parent.start_usize().saturating_add(relative_end);
    match (u32::try_from(start), u32::try_from(end)) {
        (Ok(start), Ok(end)) => basilisk_resolver::Span::new(start, end),
        _ => parent,
    }
}

pub(super) fn directive_problem(
    raw: &str,
    parsed: &LineOverride,
    known_codes: &std::collections::BTreeSet<&str>,
) -> Option<String> {
    let trimmed = raw.trim();
    let rest = ["disabled", "warning", "info", "ignore"]
        .iter()
        .find_map(|verb| trimmed.strip_prefix(verb))?
        .trim();
    if rest.starts_with('[') {
        if !rest.contains(']') {
            return Some("missing closing `]`".to_owned());
        }
        let contents = rest
            .strip_prefix('[')
            .and_then(|value| value.split_once(']').map(|(contents, _)| contents))
            .unwrap_or_default();
        if contents.split(',').all(|code| code.trim().is_empty()) {
            return Some("rule selector cannot be empty".to_owned());
        }
    } else if !rest.is_empty() && !rest.starts_with("--") {
        return Some("unexpected content after directive verb".to_owned());
    }

    // Foreign `ignore[...]` tokens intentionally become blanket PEP 484
    // suppression. Every code retained by the application parser must be live.
    parsed
        .codes
        .iter()
        .find(|code| !known_codes.contains(code.as_str()))
        .map(|code| format!("unknown Basilisk rule code `{code}`"))
}

pub(super) fn parse_line_directive(directive: &str) -> LineDirectiveParse {
    match parse_directive(directive) {
        Ok(Some(parsed)) => LineDirectiveParse::Valid(parsed),
        Ok(None) => LineDirectiveParse::TypeComment,
        Err(problem) => LineDirectiveParse::Malformed(problem),
    }
}

fn parse_ignore_codes(rest: &str) -> Result<Vec<String>, &'static str> {
    let codes = parse_bracketed_codes(rest)?;
    if !codes.is_empty() && codes.iter().all(|code| is_basilisk_code(code)) {
        Ok(codes)
    } else {
        Ok(Vec::new())
    }
}

const CODE_PREFIXES: &[&str] = &[
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
    "imports",
    "returns",
    "calls",
    "assignment",
    "names",
    "dict",
    "match",
    "version",
];

fn is_basilisk_code(code: &str) -> bool {
    if code.starts_with("BSK-") {
        return true;
    }
    matches!(code.split_once('_'), Some((prefix, _)) if CODE_PREFIXES.contains(&prefix))
}

fn parse_bracketed_codes(rest: &str) -> Result<Vec<String>, &'static str> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Ok(Vec::new());
    }
    let Some(contents_and_tail) = rest.strip_prefix('[') else {
        return Err("rule selector must be enclosed in `[]`");
    };
    let Some((contents, tail)) = contents_and_tail.split_once(']') else {
        return Err("missing closing `]`");
    };
    if !tail.trim().is_empty() {
        return Err("unexpected content after rule selector");
    }
    let codes = contents
        .split(',')
        .map(|value| value.trim().to_owned())
        .collect::<Vec<_>>();
    if codes.iter().any(String::is_empty) {
        return Err("rule selector cannot be empty");
    }
    Ok(codes)
}

pub(super) fn parse_mode_directive(text: &str) -> Result<LineOverride, &'static str> {
    parse_directive(text)?.ok_or("unknown directive verb")
}

fn parse_directive(text: &str) -> Result<Option<LineOverride>, &'static str> {
    let text = text.trim();
    let (mode, rest) = if let Some(rest) = text.strip_prefix("disabled") {
        (RuleMode::Disabled, rest)
    } else if let Some(rest) = text.strip_prefix("warning") {
        (RuleMode::Warning, rest)
    } else if let Some(rest) = text.strip_prefix("info") {
        (RuleMode::Info, rest)
    } else if let Some(rest) = text.strip_prefix("ignore") {
        return parse_ignore_codes(rest).map(|codes| {
            Some(LineOverride {
                mode: RuleMode::Ignore,
                codes,
            })
        });
    } else {
        return if resembles_directive_verb(text) {
            Err("unknown directive verb")
        } else {
            Ok(None)
        };
    };
    parse_bracketed_codes(rest).map(|codes| Some(LineOverride { mode, codes }))
}

fn resembles_directive_verb(text: &str) -> bool {
    let head = text
        .split(|character: char| character.is_whitespace() || character == '[')
        .next()
        .unwrap_or_default();
    ["disabled", "warning", "info", "ignore"]
        .iter()
        .any(|verb| head.starts_with(verb) || (head.len() >= 3 && verb.starts_with(head)))
}

pub(super) fn find_matching_block_start(
    starts: &[(usize, LineOverride, Option<usize>)],
    end_override: &LineOverride,
) -> Option<usize> {
    starts.iter().rposition(|(_, start, _)| {
        start.mode == end_override.mode && start.codes == end_override.codes
    })
}
