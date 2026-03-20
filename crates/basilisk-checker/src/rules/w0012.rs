//! BSK-W0012: Unused dependency.
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

/// Emits BSK-W0012 when a declared dependency is never imported across the
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
    /// Diagnostic code for BSK-W0012.
    pub(crate) const CODE: ErrorCode = ErrorCode {
        code: "BSK-W0012",
        docs_url: "https://www.basilisk-python.dev/warnings/BSK-W0012",
    };

    /// Build the diagnostic for an unused dependency.
    ///
    /// `package_name` is the declared dependency that is never imported.
    pub(crate) fn make_diagnostic(
        package_name: &str,
        path: &str,
        span: basilisk_resolver::Span,
    ) -> Diagnostic {
        Diagnostic {
            code: Self::CODE.clone(),
            severity: Severity::Info,
            message: format!(
                "Package `{package_name}` is declared in [project.dependencies] but never imported"
            ),
            span,
            path: path.to_owned(),
            help: Some(format!("Remove it: `uv remove {package_name}`")),
            note: Some(
                "Unused dependencies increase install size and lock file complexity".to_owned(),
            ),
        }
    }
}

impl Rule for UnusedDependency {
    fn check(&self, _module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
        // Skeleton: this rule requires workspace-level data to determine which
        // declared dependencies are never imported across all files. It will be
        // activated when the workspace layer provides aggregate import sets.
        //
        // Implementation approach:
        // 1. Collect all root import modules across the workspace
        // 2. Compare against PackageRegistry direct deps
        // 3. Emit W0012 for direct deps with no matching import
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        UnusedDependency.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn make_diagnostic_produces_correct_code() {
        let diagnostic = UnusedDependency::make_diagnostic("flask", "test.py", Span::new(0, 10));
        assert_eq!(diagnostic.code.code, "BSK-W0012");
        assert_eq!(diagnostic.severity, Severity::Info);
        assert!(diagnostic.message.contains("flask"));
        assert!(diagnostic.message.contains("never imported"));
    }
}
