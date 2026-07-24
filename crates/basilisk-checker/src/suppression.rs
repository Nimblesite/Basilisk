//! Implements [CHKARCH-STRICTNESS-SUPPRESSION] / [STUBRES-SUPPRESSION].
//! Inline suppression parsing, shared application, and audit-ledger plumbing.

mod apply;
mod parser;
mod syntax;

#[cfg(test)]
#[expect(
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test-only assertions and compact fixture access"
)]
mod tests;

use crate::diagnostic::RuleMode;
use crate::suppression_audit::model::Directive;

pub use apply::apply_overrides_at_line;
pub(crate) use apply::changing_audit_directive;
pub use parser::parse_source_overrides;
pub(crate) use parser::parse_source_overrides_with_comments;

#[cfg(test)]
use apply::override_matches;

/// All parsed overrides for a source file.
#[derive(Debug)]
pub struct SourceOverrides {
    /// File-level mode: `# basilisk: relaxed` or a file-specific directive.
    pub file_mode: Option<FileOverride>,
    /// Per-line overrides keyed by zero-based line number.
    pub line_overrides: Vec<(usize, LineOverride)>,
    /// Block overrides: `(start line, end line, override data)`.
    pub block_overrides: Vec<(usize, usize, LineOverride)>,
    /// Lossless entries emitted by the same parser, only when auditing is on.
    pub(crate) audit_directives: Vec<Directive>,
    line_audit_indices: Vec<usize>,
    block_audit_indices: Vec<usize>,
    file_audit_index: Option<usize>,
}

/// A file-level override directive.
#[derive(Debug, Clone)]
pub enum FileOverride {
    /// `# basilisk: relaxed` — errors become warnings.
    Relaxed,
    /// `# basilisk: file-<verb>[CODE, ...]`.
    Specific {
        /// Effect to apply to matching rules.
        mode: RuleMode,
        /// Specific codes, or empty for every rule.
        codes: Vec<String>,
    },
}

/// A per-line or per-block override.
#[derive(Debug, Clone)]
pub struct LineOverride {
    /// Effect to apply.
    pub mode: RuleMode,
    /// Specific codes, or empty for every rule.
    pub codes: Vec<String>,
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
            audit_directives: Vec::new(),
            line_audit_indices: Vec::new(),
            block_audit_indices: Vec::new(),
            file_audit_index: None,
        }
    }
}

/// Convert a byte offset to a zero-based line number.
#[must_use]
#[expect(
    clippy::as_conversions,
    reason = "u32 to usize is always safe on 32-bit+ targets"
)]
pub fn byte_offset_to_line_in_source(source: &str, byte_offset: u32) -> usize {
    let offset = (byte_offset as usize).min(source.len());
    source
        .get(..offset)
        .map_or(0, |prefix| prefix.matches('\n').count())
}
