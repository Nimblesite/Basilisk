//! Post-suppression audit diagnostics for [CONFIGEDITOR-SUPPRESSIONS].

mod model;
mod parser;

use std::collections::BTreeSet;

use basilisk_resolver::Span;

use crate::diagnostic::{Diagnostic, RuleMode, Severity};

use model::{Boundary, Directive};

/// Inspect source directives against diagnostics as they existed immediately
/// before inline suppression, returning one native-severity audit per directive.
pub(crate) fn diagnostics(
    source: &str,
    comment_ranges: &[Span],
    ordinary: &[Diagnostic],
    path: &str,
) -> Vec<Diagnostic> {
    let known_codes = crate::rule_catalog()
        .into_iter()
        .map(|rule| rule.code)
        .collect::<BTreeSet<_>>();
    let mut directives = parser::parse_directives(source, comment_ranges, &known_codes);
    record_usage(source, ordinary, &mut directives);
    directives
        .iter()
        .map(|directive| make_audit_diagnostic(path, directive))
        .collect()
}

fn record_usage(source: &str, diagnostics: &[Diagnostic], directives: &mut [Directive]) {
    let line_index = basilisk_common::text::LineIndex::new(source);
    for diagnostic in diagnostics {
        let line = line_index
            .line(diagnostic.span.start_usize())
            .saturating_sub(1);
        if let Some(index) = winning_directive(directives, line, diagnostic.code.code) {
            if mode_changes(directives[index].mode, diagnostic.severity) {
                directives[index].changed_diagnostics += 1;
            }
        }
    }
    propagate_block_usage(directives);
}

fn winning_directive(directives: &[Directive], line: usize, code: &str) -> Option<usize> {
    let mut winner: Option<(usize, u8)> = None;
    for (index, directive) in directives.iter().enumerate() {
        if !directive.controls_diagnostics()
            || !directive.scope.contains(line)
            || !directive.selector.matches(code)
        {
            continue;
        }
        let priority = directive.scope.priority();
        if winner.is_none_or(|(_, current)| priority > current) {
            winner = Some((index, priority));
        }
    }
    winner.map(|(index, _)| index)
}

fn mode_changes(mode: Option<RuleMode>, severity: Severity) -> bool {
    match mode {
        Some(RuleMode::Ignore | RuleMode::Disabled) => true,
        Some(RuleMode::Warning) => severity != Severity::Warning,
        Some(RuleMode::Info) => severity != Severity::Info,
        Some(RuleMode::Error) => severity != Severity::Error,
        None => false,
    }
}

fn propagate_block_usage(directives: &mut [Directive]) {
    let usage = directives
        .iter()
        .enumerate()
        .filter_map(|(index, directive)| {
            (directive.boundary == Boundary::BlockStart)
                .then_some((index, directive.paired_with, directive.changed_diagnostics))
        })
        .collect::<Vec<_>>();
    for (_, paired, changed) in usage {
        if let Some(end) = paired.and_then(|index| directives.get_mut(index)) {
            end.changed_diagnostics = changed;
        }
    }
}

fn make_audit_diagnostic(path: &str, directive: &Directive) -> Diagnostic {
    if let Some(problem) = directive.problem.as_deref() {
        return crate::rules::suppression_malformed::make_diagnostic(
            path,
            directive.span,
            problem,
        );
    }
    if directive.changed_diagnostics == 0 {
        return crate::rules::suppression_unused::make_diagnostic(path, directive.span);
    }
    if directive.selector.is_blanket() {
        crate::rules::suppression_blanket::make_diagnostic(
            path,
            directive.span,
            directive.changed_diagnostics,
        )
    } else {
        crate::rules::suppression_active_specific::make_diagnostic(
            path,
            directive.span,
            directive.changed_diagnostics,
        )
    }
}
