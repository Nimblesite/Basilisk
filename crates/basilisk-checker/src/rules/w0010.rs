//! BSK-W0010: Missing type stubs for installed package.
//!
//! Fires when a package is imported and resolves to a `.py` source file (not
//! `.pyi`) without a `py.typed` marker. This means the package is installed
//! but lacks type information, reducing type safety.
//!
//! ```python
//! import flask  # W0010: Package 'flask' is installed but has no type stubs
//! ```

use basilisk_resolver::{ImportInfo, ImportResolution, ResolvedModule};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-W0010",
    docs_url: "https://www.basilisk-python.dev/warnings/BSK-W0010",
};

/// Emits BSK-W0010 when an imported package resolves to a `.py` source file
/// without a `py.typed` marker, indicating missing type stubs.
pub(crate) struct MissingTypeStubs;

impl Rule for MissingTypeStubs {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .imports
            .iter()
            .filter(|import| import.resolution == ImportResolution::SourcePy)
            .filter(|import| !basilisk_stubs::is_stdlib_module(&import.module))
            .filter(|import| is_site_packages_import(import))
            .for_each(|import| diagnostics.push(make_diagnostic(import, &module.path)));
    }
}

/// Check whether an import resolved to a site-packages path.
///
/// A simple heuristic: the resolved path contains a `site-packages` component.
fn is_site_packages_import(import: &ImportInfo) -> bool {
    import
        .resolved_path
        .as_ref()
        .is_some_and(|path| path.to_string_lossy().contains("site-packages"))
}

/// Build the diagnostic for a missing type stubs warning.
fn make_diagnostic(import: &ImportInfo, path: &str) -> Diagnostic {
    let root_module = import.module.split('.').next().unwrap_or(&import.module);

    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Warning,
        message: format!("Package `{root_module}` is installed but has no type stubs available"),
        span: import.span,
        path: path.to_owned(),
        help: Some(format!(
            "Install type stubs: `uv add --dev types-{root_module}`"
        )),
        note: Some(
            "Packages without type stubs or a `py.typed` marker provide no type information"
                .to_owned(),
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
    use std::path::PathBuf;

    fn make_module(imports: Vec<ImportInfo>) -> ResolvedModule {
        ResolvedModule {
            path: "test.py".to_owned(),
            imports,
            ..ResolvedModule::default()
        }
    }

    #[test]
    fn fires_for_site_packages_source_py() {
        let import = ImportInfo {
            module: "flask".to_owned(),
            names: vec![],
            span: Span::new(0, 12),
            kind: ImportKind::Plain,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(PathBuf::from(
                "/venv/lib/python3.12/site-packages/flask/__init__.py",
            )),
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        MissingTypeStubs.check(&module, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code.code, "BSK-W0010");
    }

    #[test]
    fn skips_workspace_source_py() {
        let import = ImportInfo {
            module: "myapp".to_owned(),
            names: vec![],
            span: Span::new(0, 12),
            kind: ImportKind::Plain,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(PathBuf::from("/workspace/myapp/__init__.py")),
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        MissingTypeStubs.check(&module, &mut diagnostics);
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
            resolved_path: Some(PathBuf::from(
                "/venv/lib/python3.12/site-packages/os/__init__.py",
            )),
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        MissingTypeStubs.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_stub_pyi_resolution() {
        let import = ImportInfo {
            module: "requests".to_owned(),
            names: vec![],
            span: Span::new(0, 15),
            kind: ImportKind::Plain,
            resolution: ImportResolution::StubPyi,
            resolved_path: Some(PathBuf::from(
                "/venv/lib/python3.12/site-packages/requests-stubs/__init__.pyi",
            )),
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        MissingTypeStubs.check(&module, &mut diagnostics);
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
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        MissingTypeStubs.check(&module, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }
}
