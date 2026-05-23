//! BSK-W0013: Stale uv lock file.
//!
//! Fires when the `uv.lock` file is older than `pyproject.toml`, indicating
//! that dependencies may have changed without re-locking. This can cause
//! import resolution to use stale package versions.
//!
//! This rule currently provides the skeleton; it activates when the workspace
//! provides lock-file staleness information via the resolver context.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

/// Emits BSK-W0013 when the uv lock file appears stale relative to the
/// project configuration.
///
/// This rule is a skeleton awaiting lock-file mtime comparison data from the
/// workspace layer. It currently does not fire.
pub(crate) struct StaleLockFile;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "skeleton rule: CODE and make_diagnostic are pre-built for when \
                  workspace staleness data is wired through"
    )
)]
impl StaleLockFile {
    /// Diagnostic code for BSK-W0013.
    pub(crate) const CODE: ErrorCode = ErrorCode {
        code: "BSK-W0013",
        docs_url: "https://www.basilisk-python.dev/warnings/BSK-W0013",
    };

    /// Build the diagnostic for a stale lock file warning.
    ///
    /// Available for use when the workspace layer provides lock-file staleness
    /// data.
    pub(crate) fn make_diagnostic(path: &str, span: basilisk_resolver::Span) -> Diagnostic {
        warning_diagnostic_owned(
            Self::CODE.clone(),
            "uv.lock is older than pyproject.toml — dependencies may be stale".to_owned(),
            span,
            path,
            Some("Lock file is out of date with pyproject.toml".to_owned()),
            Some(
                "Stale lock files can cause incorrect import resolution and missing packages"
                    .to_owned(),
            ),
        )
    }
}

impl Rule for StaleLockFile {
    fn check(&self, _module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
        // Skeleton: once the workspace layer provides lock-file staleness
        // information (e.g. via a field on ResolvedModule or a shared context),
        // this rule will compare mtimes and emit a diagnostic when uv.lock is
        // older than pyproject.toml.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;
    use basilisk_resolver::Span;

    fn make_module() -> ResolvedModule {
        ResolvedModule {
            path: "test.py".to_owned(),
            ..ResolvedModule::default()
        }
    }

    #[test]
    fn does_not_fire_without_staleness_data() {
        let module = make_module();
        let mut diagnostics = Vec::new();
        StaleLockFile.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn make_diagnostic_produces_correct_code() {
        let diagnostic = StaleLockFile::make_diagnostic("test.py", Span::new(0, 10));
        assert_eq!(diagnostic.code.code, "BSK-W0013");
        assert_eq!(diagnostic.severity, Severity::Warning);
    }
}
