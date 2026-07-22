//! Implements [BSK-0012] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! BSK-0012: Unused dependency.
//!
//! Fires when a package is declared in `[project.dependencies]` but no module
//! in the workspace imports it. This indicates a dependency that can be removed,
//! reducing the project's dependency footprint.
//!
//! This is a **whole-module-only** diagnostic — it requires scanning all files
//! in the workspace to determine which packages are actually imported. The rule
//! currently provides the skeleton; it activates when the workspace layer
//! provides aggregate import data.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

/// Emits BSK-0012 when a declared dependency is never imported across the
/// workspace.
///
/// This rule is a skeleton awaiting workspace-level aggregate import data.
/// It cannot operate per-file because unused-dependency detection requires
/// knowledge of all imports across all workspace files.
pub(crate) struct UnusedDependency;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "skeleton rule: CODE and make_diagnostic are pre-built for when \
                  workspace aggregate import data is wired through"
    )
)]
impl UnusedDependency {
    /// Diagnostic code for BSK-0012.
    pub(crate) const CODE: ErrorCode = ErrorCode {
        code: "BSK-0012",
        docs_url: "https://www.basilisk-python.dev/errors/BSK-0012",
    };

    /// Build the diagnostic for an unused dependency.
    ///
    /// `package_name` is the declared dependency that is never imported.
    pub(crate) fn make_diagnostic(
        package_name: &str,
        path: &str,
        span: basilisk_resolver::Span,
    ) -> Diagnostic {
        warning_diagnostic_owned(
            Self::CODE.clone(),
            format!(
                "Package `{package_name}` is declared in [project.dependencies] but never imported"
            ),
            span,
            path,
            Some(format!("Remove it: `uv remove {package_name}`")),
            Some("Unused dependencies increase install size and lock file complexity".to_owned()),
        )
    }
}

impl Rule for UnusedDependency {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: Self::CODE.code,
            tags: &["dependencies"],
        })
    }

    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Skeleton: this rule requires workspace-level data to determine which
        // declared dependencies are never imported across all files. It will be
        // activated when the workspace layer provides aggregate import sets.
        //
        // Implementation approach:
        // 1. Collect all root import modules across the workspace
        // 2. Compare against PackageRegistry direct deps
        // 3. Emit BSK-0012 for direct deps with no matching import
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
    fn does_not_fire_without_workspace_data() {
        let module = make_module();
        let mut diagnostics = Vec::new();
        UnusedDependency.check(
            &module,
            &crate::context::CheckContext::default(),
            &mut diagnostics,
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn make_diagnostic_produces_correct_code() {
        let diagnostic = UnusedDependency::make_diagnostic("flask", "test.py", Span::new(0, 10));
        assert_eq!(diagnostic.code.code, "BSK-0012");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(diagnostic.message.contains("flask"));
        assert!(diagnostic.message.contains("never imported"));
    }
}
