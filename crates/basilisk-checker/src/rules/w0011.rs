//! Implements [BSK-W0011] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! BSK-W0011: Undeclared dependency import.
//!
//! Fires when an import resolves to a package that is only a transitive
//! dependency — present in `uv.lock` but not listed in the project's
//! `[project.dependencies]` in `pyproject.toml`.
//!
//! Transitive dependencies can disappear when a direct dependency drops them,
//! breaking imports that relied on their implicit availability.
//!
//! ```python
//! import urllib3  # W0011: 'urllib3' is a transitive dependency (via requests)
//! ```

use basilisk_resolver::{ImportResolution, PackageDepKind, ResolvedModule};

use crate::diagnostic::{warning_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0011",
    docs_url: "https://www.basilisk-python.dev/warnings/BSK-W0011",
};

/// Emits BSK-W0011 when an import uses a transitive dependency that is not
/// declared in `[project.dependencies]`.
pub(crate) struct UndeclaredDependencyImport;

impl Rule for UndeclaredDependencyImport {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .imports
            .iter()
            .filter(|import| import.resolution != ImportResolution::Unresolved)
            .filter(|import| !basilisk_stubs::is_stdlib_module(&import.module))
            .filter(|import| import.package_dep_kind == Some(PackageDepKind::Transitive))
            .for_each(|import| {
                let root_module = import.module.split('.').next().unwrap_or(&import.module);
                diagnostics.push(warning_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Import `{root_module}` is a transitive dependency, not declared in \
                         [project.dependencies]"
                    ),
                    import.span,
                    &module.path,
                    Some(format!("Add it explicitly: `uv add {root_module}`")),
                    Some(
                        "Transitive dependencies can disappear when direct dependencies change"
                            .to_owned(),
                    ),
                ));
            });
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
    use basilisk_resolver::{ImportInfo, Span};
    use std::path::PathBuf;

    fn make_module(imports: Vec<ImportInfo>) -> ResolvedModule {
        ResolvedModule {
            path: "test.py".to_owned(),
            imports,
            ..ResolvedModule::default()
        }
    }

    /// Build an `ImportInfo` for these tests, only varying the fields that affect the rule.
    fn make_import(
        module: &str,
        span_end: u32,
        resolution: ImportResolution,
        resolved_path: Option<&str>,
        dep_kind: Option<PackageDepKind>,
    ) -> ImportInfo {
        ImportInfo {
            module: module.to_owned(),
            names: vec![],
            span: Span::new(0, span_end),
            kind: ImportKind::Plain,
            resolution,
            resolved_path: resolved_path.map(PathBuf::from),
            package_dep_kind: dep_kind,
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        }
    }

    fn run_check(import: ImportInfo) -> Vec<crate::Diagnostic> {
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        UndeclaredDependencyImport.check(
            &module,
            &crate::context::CheckContext::default(),
            &mut diagnostics,
        );
        diagnostics
    }

    #[test]
    fn fires_for_transitive_dependency() {
        let import = make_import(
            "urllib3",
            14,
            ImportResolution::SourcePy,
            Some("/venv/lib/python3.12/site-packages/urllib3/__init__.py"),
            Some(PackageDepKind::Transitive),
        );
        let diagnostics = run_check(import);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code.code, "BSK-W0011");
        assert!(diagnostics[0].message.contains("transitive"));
    }

    #[test]
    fn skips_direct_dependency() {
        let import = make_import(
            "requests",
            15,
            ImportResolution::SourcePy,
            Some("/venv/lib/python3.12/site-packages/requests/__init__.py"),
            Some(PackageDepKind::Direct),
        );
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn skips_dev_dependency() {
        let import = make_import(
            "pytest",
            13,
            ImportResolution::SourcePy,
            Some("/venv/lib/python3.12/site-packages/pytest/__init__.py"),
            Some(PackageDepKind::Dev),
        );
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn skips_stdlib_modules() {
        let import = make_import("os", 9, ImportResolution::SourcePy, None, None);
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn skips_unresolved_imports() {
        let import = make_import(
            "nonexistent",
            18,
            ImportResolution::Unresolved,
            None,
            Some(PackageDepKind::Transitive),
        );
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn skips_when_no_dep_kind() {
        let import = make_import(
            "mylib",
            12,
            ImportResolution::SourcePy,
            Some("/workspace/mylib/__init__.py"),
            None,
        );
        assert!(run_check(import).is_empty());
    }
}
