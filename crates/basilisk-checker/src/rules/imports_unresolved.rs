//! Implements [`imports_unresolved`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! `imports_unresolved`: Unresolved import.
//!
//! Fires when an import cannot be resolved and the module is not part of the
//! Python standard library.  When uv package-registry context is available the
//! diagnostic message explains *why* the import failed (not installed,
//! transitive-only, needs sync, wrong Python version).  Without that context a
//! generic fallback message is used.
//!
//! This is where the static resolution model surfaces its terminal state
//! ([STUBRES-STATIC-MODEL]): an import the static filesystem search could not
//! follow — a missing dependency, but equally a computed/dynamic import or a
//! module only a runtime `sys.meta_path` hook could supply — carries an implicit
//! `Any`, and default-strict reports it here rather than silently accepting it.

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
///
/// The stdlib skip is gated on
/// [`crate::imports::bundled_stdlib_recognized`], not the raw name-set: when a
/// custom typeshed (`typeshed-path`) is canonical for step 3, a stdlib module
/// absent from it is surfaced as unresolved rather than silently skipped
/// ([STUBRES-CUSTOM-TYPESHED]).
pub(crate) struct ImportFromUntypedModule;

impl Rule for ImportFromUntypedModule {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .imports
            .iter()
            // Skip modules the bundled stdlib name-set still vouches for. When a
            // custom typeshed is configured it is canonical for step 3, so that
            // name-set no longer rescues a stdlib module absent from it — the
            // import must surface as unresolved instead of being silently
            // swallowed here ([STUBRES-CUSTOM-TYPESHED]). Gating on
            // `bundled_stdlib_recognized` keeps this decision identical to the
            // resolver and cascade-suppression sites.
            .filter(|import| {
                !crate::imports::bundled_stdlib_recognized(
                    &import.module,
                    ctx.custom_typeshed_configured,
                )
            })
            // Terminal state of the static search ([STUBRES-STATIC-MODEL]):
            // whatever the search could not follow to a `.py`/`.pyi` is an
            // implicit `Any` we surface here instead of silently accepting.
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
        help: Some(help.into()),
        note: Some("Basilisk requires complete type information for all imported modules".into()),
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

    /// Run the rule with a custom typeshed configured in the context.
    fn run_check_custom_typeshed(import: ImportInfo) -> Vec<crate::Diagnostic> {
        let module = make_module(vec![import]);
        let ctx = crate::context::CheckContext {
            custom_typeshed_configured: true,
            ..crate::context::CheckContext::default()
        };
        let mut diagnostics = Vec::new();
        ImportFromUntypedModule.check(&module, &ctx, &mut diagnostics);
        diagnostics
    }

    /// [STUBRES-CUSTOM-TYPESHED]: with a custom typeshed configured the bundled
    /// name-set is no longer canonical, so a stdlib module that failed to resolve
    /// against it MUST surface as unresolved instead of being silently skipped.
    /// Regression guard for the second suppression site the resolver fix missed.
    #[test]
    fn custom_typeshed_surfaces_absent_stdlib() {
        let mut diags =
            run_check_custom_typeshed(make_import("fractions", Some(UnresolvedReason::Unknown)));
        assert_eq!(
            diags.len(),
            1,
            "absent stdlib module must be reported under a custom typeshed"
        );
        assert_eq!(diags.remove(0).code.code, "imports_unresolved");
    }

    /// A stdlib module the custom typeshed *does* supply resolves to a `.pyi`, so
    /// its resolution is not `Unresolved` and no diagnostic fires.
    #[test]
    fn custom_typeshed_keeps_resolved_stdlib_quiet() {
        let import = ImportInfo {
            resolution: ImportResolution::StubPyi,
            ..make_import("os", None)
        };
        assert!(
            run_check_custom_typeshed(import).is_empty(),
            "a stdlib module resolved via the custom typeshed must not be reported"
        );
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

    /// [STUBRES-STATIC-MODEL]: a module the static filesystem search cannot follow
    /// — the terminal state a computed/dynamic import or a `sys.meta_path`-only
    /// module lands in — surfaces its implicit `Any` as `imports_unresolved`
    /// rather than being silently accepted. Whatever the resolver could not reach
    /// on disk arrives here as `ImportResolution::Unresolved`.
    #[test]
    fn static_model_surfaces_unresolvable_module() {
        let diag = check_single("_runtime_only_module", None);
        assert_eq!(diag.code.code, "imports_unresolved");
        assert!(diag.message.contains("no type information available"));
    }

    #[test]
    fn diagnostic_has_help_text() {
        let diag = check_single("requests", None);
        assert!(diag.help.is_some());
    }
}
