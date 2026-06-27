//! Implements [`imports_unresolved`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! `imports_unresolved`: Unresolved import.
//!
//! Fires when an import cannot be resolved and the module is not part of the
//! Python standard library.  When uv package-registry context is available the
//! diagnostic message explains *why* the import failed (not installed,
//! transitive-only, needs sync, wrong Python version).  Without that context a
//! generic fallback message is used.

use basilisk_resolver::{ImportInfo, ImportResolution, ResolvedModule, UnresolvedReason};
use basilisk_stubs::TypeProvenance;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "imports_unresolved",
    docs_url: "https://www.basilisk-python.dev/errors/imports_unresolved",
};

/// Emits `imports_unresolved` for imports from modules outside the known stdlib/typing
/// ecosystem.
///
/// Uses the compiled typeshed index from `basilisk-stubs` for O(1) module
/// recognition.  Imports that resolved to workspace or stub files are skipped
/// — they already have type information available.
/// Suppression is handled centrally by the `suppression` module.
pub(crate) struct ImportFromUntypedModule;

impl Rule for ImportFromUntypedModule {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .imports
            .iter()
            .filter(|import| !basilisk_stubs::is_stdlib_module(&import.module))
            .filter(|import| import.resolution == ImportResolution::Unresolved)
            .for_each(|import| diagnostics.push(make_diagnostic(import, &module.path)));
    }
}

/// Build a context-aware diagnostic message for an unresolved import.
///
/// When `unresolved_reason` is populated (uv project with registry), the
/// message explains the specific cause.  Otherwise falls back to a generic
/// "no type information available" message.
fn make_diagnostic(import: &ImportInfo, path: &str) -> Diagnostic {
    let root_module = import.module.split('.').next().unwrap_or(&import.module);

    let (message, help) = format_reason(import, root_module);

    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span: import.span,
        path: path.to_owned(),
        help: Some(help),
        note: Some(
            "Basilisk requires complete type information for all imported modules".to_owned(),
        ),
        provenance: Some(TypeProvenance::Untyped),
    }
}

/// Produce the diagnostic message and help text based on the unresolved reason.
///
/// Help text describes the problem — code actions (in the LSP layer) handle the
/// fix. Users should never be told to run CLI commands manually.
fn format_reason(import: &ImportInfo, root_module: &str) -> (String, String) {
    match &import.unresolved_reason {
        Some(UnresolvedReason::NotInstalled) => (
            format!(
                "Cannot resolve import `{}` \u{2014} `{root_module}` is not a dependency in \
                 pyproject.toml",
                import.module
            ),
            format!("`{root_module}` is not listed in project dependencies"),
        ),
        Some(UnresolvedReason::NotInDeps) => (
            format!(
                "Cannot resolve import `{}` \u{2014} `{root_module}` is only a transitive \
                 dependency; add it to [project.dependencies]",
                import.module
            ),
            format!(
                "`{root_module}` is available transitively but should be declared as a direct \
                 dependency"
            ),
        ),
        Some(UnresolvedReason::NeedsSync) => (
            format!(
                "Cannot resolve import `{}` \u{2014} `{root_module}` is declared but the \
                 environment is not synced",
                import.module
            ),
            "Environment is out of sync with declared dependencies".to_owned(),
        ),
        Some(UnresolvedReason::WrongPythonVersion) => (
            format!(
                "Cannot resolve import `{}` \u{2014} not available in the target Python version",
                import.module
            ),
            "Check the `requires-python` setting in pyproject.toml".to_owned(),
        ),
        // NoStubs is handled by E0152 — fall through to generic message.
        Some(UnresolvedReason::NoStubs | UnresolvedReason::Unknown) | None => (
            format!(
                "Cannot resolve import `{}` \u{2014} no type information available",
                import.module
            ),
            format!("`{root_module}` is not installed or has no type stubs"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use basilisk_resolver::scope::ImportKind;
    use basilisk_resolver::Span;

    fn make_module(imports: Vec<ImportInfo>) -> ResolvedModule {
        ResolvedModule {
            path: "test.py".to_owned(),
            imports,
            ..ResolvedModule::default()
        }
    }

    fn make_import(module: &str, reason: Option<UnresolvedReason>) -> ImportInfo {
        ImportInfo {
            module: module.to_owned(),
            names: vec![],
            span: Span::new(0, 15),
            kind: ImportKind::Plain,
            resolution: ImportResolution::Unresolved,
            resolved_path: None,
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            unresolved_reason: reason,
        }
    }

    fn run_check(import: ImportInfo) -> Vec<crate::Diagnostic> {
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(
            &module,
            &crate::context::CheckContext::default(),
            &mut diagnostics,
        );
        diagnostics
    }

    /// Run the rule on a single import with `reason` and assert a single diagnostic was emitted.
    /// Returns that diagnostic for further inspection.
    fn check_single(name: &str, reason: Option<UnresolvedReason>) -> crate::Diagnostic {
        let mut diagnostics = run_check(make_import(name, reason));
        assert_eq!(diagnostics.len(), 1);
        diagnostics.remove(0)
    }

    #[test]
    fn generic_message_when_no_reason() {
        let diag = check_single("requests", None);
        assert!(diag.message.contains("no type information available"));
    }

    #[test]
    fn not_installed_message() {
        let diag = check_single("requests", Some(UnresolvedReason::NotInstalled));
        assert!(diag
            .message
            .contains("is not a dependency in pyproject.toml"));
        assert!(diag
            .help
            .as_ref()
            .is_some_and(|h| h.contains("not listed in project dependencies")));
    }

    #[test]
    fn not_in_deps_message() {
        let diag = check_single("urllib3", Some(UnresolvedReason::NotInDeps));
        assert!(diag.message.contains("transitive dependency"));
        assert!(diag.message.contains("[project.dependencies]"));
    }

    #[test]
    fn needs_sync_message() {
        let diag = check_single("flask", Some(UnresolvedReason::NeedsSync));
        assert!(diag.message.contains("not synced"));
        assert!(diag
            .help
            .as_ref()
            .is_some_and(|h| h.contains("out of sync")));
    }

    #[test]
    fn wrong_python_version_message() {
        let diag = check_single(
            "some_versioned_pkg",
            Some(UnresolvedReason::WrongPythonVersion),
        );
        assert!(diag
            .message
            .contains("not available in the target Python version"));
    }

    #[test]
    fn no_stubs_falls_back_to_generic() {
        let diag = check_single("requests", Some(UnresolvedReason::NoStubs));
        assert!(diag.message.contains("no type information available"));
    }

    #[test]
    fn unknown_reason_falls_back_to_generic() {
        let diag = check_single("requests", Some(UnresolvedReason::Unknown));
        assert!(diag.message.contains("no type information available"));
    }

    #[test]
    fn skips_stdlib_imports() {
        assert!(run_check(make_import("os", None)).is_empty());
    }

    #[test]
    fn skips_resolved_imports() {
        let import = ImportInfo {
            resolution: ImportResolution::SourcePy,
            ..make_import("requests", None)
        };
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn diagnostic_has_correct_code() {
        let diag = check_single("numpy", None);
        assert_eq!(diag.code.code, "imports_unresolved");
    }

    #[test]
    fn diagnostic_has_help_text() {
        let diag = check_single("requests", None);
        assert!(diag.help.is_some());
    }
}
