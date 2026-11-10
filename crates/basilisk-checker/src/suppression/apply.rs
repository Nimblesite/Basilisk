//! Shared override selection and application.

use crate::diagnostic::{Diagnostic, RuleMode, Severity};

use super::{FileOverride, SourceOverrides};

/// Apply all overrides to a diagnostic at its pre-computed zero-based line.
#[must_use]
pub fn apply_overrides_at_line(
    mut diagnostic: Diagnostic,
    line: usize,
    overrides: &SourceOverrides,
) -> Option<Diagnostic> {
    match selected_override(diagnostic.code.code, line, overrides) {
        Some(SelectedOverride::Mode { mode, .. }) => apply_mode(diagnostic, mode),
        Some(SelectedOverride::Relaxed { .. }) => {
            if matches!(
                diagnostic.severity,
                Severity::Error | Severity::SafetyViolation
            ) {
                diagnostic.severity = Severity::Warning;
            }
            Some(diagnostic)
        }
        None => Some(diagnostic),
    }
}

#[derive(Debug, Clone, Copy)]
enum SelectedOverride {
    Mode {
        mode: RuleMode,
        audit_index: Option<usize>,
    },
    Relaxed {
        audit_index: Option<usize>,
    },
}

/// Select the exact override both application and auditing observe.
fn selected_override(
    code: &str,
    line: usize,
    overrides: &SourceOverrides,
) -> Option<SelectedOverride> {
    for (index, (target, line_override)) in overrides.line_overrides.iter().enumerate() {
        if *target == line && override_matches(code, &line_override.codes) {
            return Some(SelectedOverride::Mode {
                mode: line_override.mode,
                audit_index: overrides.line_audit_indices.get(index).copied(),
            });
        }
    }
    for (index, (start, end, block_override)) in overrides.block_overrides.iter().enumerate() {
        if line >= *start && line <= *end && override_matches(code, &block_override.codes) {
            return Some(SelectedOverride::Mode {
                mode: block_override.mode,
                audit_index: overrides.block_audit_indices.get(index).copied(),
            });
        }
    }
    match overrides.file_mode.as_ref()? {
        FileOverride::Relaxed => Some(SelectedOverride::Relaxed {
            audit_index: overrides.file_audit_index,
        }),
        FileOverride::Specific { mode, codes } if override_matches(code, codes) => {
            Some(SelectedOverride::Mode {
                mode: *mode,
                audit_index: overrides.file_audit_index,
            })
        }
        FileOverride::Specific { .. } => None,
    }
}

/// Return the ledger entry whose applied effect changes this diagnostic.
pub(crate) fn changing_audit_directive(
    overrides: &SourceOverrides,
    line: usize,
    code: &str,
    severity: Severity,
) -> Option<usize> {
    match selected_override(code, line, overrides)? {
        SelectedOverride::Mode { mode, audit_index } => {
            let changes = match mode {
                RuleMode::Ignore | RuleMode::Disabled => true,
                RuleMode::Warning => severity != Severity::Warning,
                RuleMode::Info => severity != Severity::Info,
                RuleMode::Error => false,
            };
            changes.then_some(audit_index).flatten()
        }
        SelectedOverride::Relaxed { audit_index } => {
            matches!(severity, Severity::Error | Severity::SafetyViolation)
                .then_some(audit_index)
                .flatten()
        }
    }
}

fn apply_mode(mut diagnostic: Diagnostic, mode: RuleMode) -> Option<Diagnostic> {
    match mode {
        RuleMode::Ignore | RuleMode::Disabled => None,
        RuleMode::Warning => {
            diagnostic.severity = Severity::Warning;
            Some(diagnostic)
        }
        RuleMode::Info => {
            diagnostic.severity = Severity::Info;
            Some(diagnostic)
        }
        RuleMode::Error => Some(diagnostic),
    }
}

pub(super) fn override_matches(code: &str, configured: &[String]) -> bool {
    configured.is_empty() || configured.iter().any(|candidate| candidate == code)
}
