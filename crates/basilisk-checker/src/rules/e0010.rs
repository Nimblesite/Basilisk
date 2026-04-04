//! BSK-E0010: Unresolved import.
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
    code: "BSK-E0010",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0010",
};

/// Emits BSK-E0010 for imports from modules outside the known stdlib/typing
/// ecosystem.
///
/// Uses the compiled typeshed index from `basilisk-stubs` for O(1) module
/// recognition.  Imports that resolved to workspace or stub files are skipped
/// — they already have type information available.
/// Suppression is handled centrally by the `suppression` module.
pub(crate) struct ImportFromUntypedModule;

impl Rule for ImportFromUntypedModule {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
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
        // NoStubs is handled by W0010 — fall through to generic message.
        Some(UnresolvedReason::NoStubs | UnresolvedReason::Unknown) | None => (
            format!(
                "Cannot resolve import `{}` \u{2014} no type information available",
                import.module
            ),
            format!(
                "`{root_module}` is not installed or has no type stubs"
            ),
        ),
    }
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    reason = "test-only code: indexing acceptable in unit tests"
)]
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

    #[test]
    fn generic_message_when_no_reason() {
        let import = make_import("requests", None);
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("no type information available"));
    }

    #[test]
    fn not_installed_message() {
        let import = make_import("requests", Some(UnresolvedReason::NotInstalled));
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("is not a dependency in pyproject.toml"));
        assert!(diagnostics[0]
            .help
            .as_ref()
            .is_some_and(|h| h.contains("not listed in project dependencies")));
    }

    #[test]
    fn not_in_deps_message() {
        let import = make_import("urllib3", Some(UnresolvedReason::NotInDeps));
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("transitive dependency"));
        assert!(diagnostics[0].message.contains("[project.dependencies]"));
    }

    #[test]
    fn needs_sync_message() {
        let import = make_import("flask", Some(UnresolvedReason::NeedsSync));
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("not synced"));
        assert!(diagnostics[0]
            .help
            .as_ref()
            .is_some_and(|h| h.contains("out of sync")));
    }

    #[test]
    fn wrong_python_version_message() {
        let import = make_import(
            "some_versioned_pkg",
            Some(UnresolvedReason::WrongPythonVersion),
        );
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("not available in the target Python version"));
    }

    #[test]
    fn no_stubs_falls_back_to_generic() {
        let import = make_import("requests", Some(UnresolvedReason::NoStubs));
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("no type information available"));
    }

    #[test]
    fn unknown_reason_falls_back_to_generic() {
        let import = make_import("requests", Some(UnresolvedReason::Unknown));
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("no type information available"));
    }

    #[test]
    fn skips_stdlib_imports() {
        let import = make_import("os", None);
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_resolved_imports() {
        let import = ImportInfo {
            module: "requests".to_owned(),
            names: vec![],
            span: Span::new(0, 15),
            kind: ImportKind::Plain,
            resolution: ImportResolution::SourcePy,
            resolved_path: None,
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn diagnostic_has_correct_code() {
        let import = make_import("numpy", None);
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code.code, "BSK-E0010");
    }

    #[test]
    fn diagnostic_has_help_text() {
        let import = make_import("requests", None);
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].help.is_some());
    }
}
