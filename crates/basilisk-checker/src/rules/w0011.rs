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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0011",
    docs_url: "https://www.basilisk-python.dev/warnings/BSK-W0011",
};

/// Emits BSK-W0011 when an import uses a transitive dependency that is not
/// declared in `[project.dependencies]`.
pub(crate) struct UndeclaredDependencyImport;

impl Rule for UndeclaredDependencyImport {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .imports
            .iter()
            .filter(|import| import.resolution != ImportResolution::Unresolved)
            .filter(|import| !basilisk_stubs::is_stdlib_module(&import.module))
            .filter(|import| import.package_dep_kind == Some(PackageDepKind::Transitive))
            .for_each(|import| {
                let root_module = import.module.split('.').next().unwrap_or(&import.module);
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Warning,
                    message: format!(
                        "Import `{root_module}` is a transitive dependency, not declared in \
                         [project.dependencies]"
                    ),
                    span: import.span,
                    path: module.path.clone(),
                    help: Some(format!("Add it explicitly: `uv add {root_module}`")),
                    note: Some(
                        "Transitive dependencies can disappear when direct dependencies change"
                            .to_owned(),
                    ),
                    provenance: None,
                });
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

    #[test]
    fn fires_for_transitive_dependency() {
        let import = ImportInfo {
            module: "urllib3".to_owned(),
            names: vec![],
            span: Span::new(0, 14),
            kind: ImportKind::Plain,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(PathBuf::from(
                "/venv/lib/python3.12/site-packages/urllib3/__init__.py",
            )),
            package_dep_kind: Some(PackageDepKind::Transitive),
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        UndeclaredDependencyImport.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code.code, "BSK-W0011");
        assert!(diagnostics[0].message.contains("transitive"));
    }

    #[test]
    fn skips_direct_dependency() {
        let import = ImportInfo {
            module: "requests".to_owned(),
            names: vec![],
            span: Span::new(0, 15),
            kind: ImportKind::Plain,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(PathBuf::from(
                "/venv/lib/python3.12/site-packages/requests/__init__.py",
            )),
            package_dep_kind: Some(PackageDepKind::Direct),
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        UndeclaredDependencyImport.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_dev_dependency() {
        let import = ImportInfo {
            module: "pytest".to_owned(),
            names: vec![],
            span: Span::new(0, 13),
            kind: ImportKind::Plain,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(PathBuf::from(
                "/venv/lib/python3.12/site-packages/pytest/__init__.py",
            )),
            package_dep_kind: Some(PackageDepKind::Dev),
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        UndeclaredDependencyImport.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_stdlib_modules() {
        let import = ImportInfo {
            module: "os".to_owned(),
            names: vec![],
            span: Span::new(0, 9),
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
        UndeclaredDependencyImport.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_unresolved_imports() {
        let import = ImportInfo {
            module: "nonexistent".to_owned(),
            names: vec![],
            span: Span::new(0, 18),
            kind: ImportKind::Plain,
            resolution: ImportResolution::Unresolved,
            resolved_path: None,
            package_dep_kind: Some(PackageDepKind::Transitive),
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        UndeclaredDependencyImport.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_when_no_dep_kind() {
        let import = ImportInfo {
            module: "mylib".to_owned(),
            names: vec![],
            span: Span::new(0, 12),
            kind: ImportKind::Plain,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(PathBuf::from("/workspace/mylib/__init__.py")),
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        UndeclaredDependencyImport.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }
}
