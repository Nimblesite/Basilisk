//! Post-suppression audit diagnostics for [CONFIGEDITOR-SUPPRESSIONS] and
//! [CHKARCH-STRICTNESS-SUPPRESSION-DIAGNOSTICS].

pub(crate) mod model;

use crate::diagnostic::Diagnostic;

use model::{Boundary, Directive};

/// Inspect source directives against diagnostics as they existed immediately
/// before inline suppression, returning one native-severity audit per directive.
pub(crate) fn diagnostics(
    source: &str,
    overrides: &crate::suppression::SourceOverrides,
    ordinary: &[Diagnostic],
    path: &str,
) -> Vec<Diagnostic> {
    let mut directives = overrides.audit_directives.clone();
    record_usage(source, overrides, ordinary, &mut directives);
    directives
        .iter()
        .map(|directive| make_audit_diagnostic(path, directive))
        .collect()
}

fn record_usage(
    source: &str,
    overrides: &crate::suppression::SourceOverrides,
    diagnostics: &[Diagnostic],
    directives: &mut [Directive],
) {
    let line_index = basilisk_common::text::LineIndex::new(source);
    for diagnostic in diagnostics {
        let line = line_index
            .line(diagnostic.span.start_usize())
            .saturating_sub(1);
        if let Some(index) = crate::suppression::changing_audit_directive(
            overrides,
            line,
            diagnostic.code.code,
            diagnostic.severity,
        ) {
            if let Some(directive) = directives.get_mut(index) {
                directive.changed_diagnostics += 1;
            }
        }
    }
    propagate_block_usage(directives);
}

fn propagate_block_usage(directives: &mut [Directive]) {
    let usage = directives
        .iter()
        .enumerate()
        .filter_map(|(index, directive)| {
            (directive.boundary == Boundary::BlockStart).then_some((
                index,
                directive.paired_with,
                directive.changed_diagnostics,
            ))
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
        return crate::rules::suppression_malformed::make_diagnostic(path, directive.span, problem);
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
