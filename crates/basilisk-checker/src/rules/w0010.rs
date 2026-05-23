//! Implements [BSK-W0010] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
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
use basilisk_stubs::TypeProvenance;

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
            .filter(|import| !has_py_typed_marker(import))
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

/// Check whether the resolved package has a `py.typed` marker (PEP 561).
///
/// Per PEP 561 the marker is placed at the **top-level package** and applies
/// to every submodule. For a `from sqlalchemy.orm import ...` import the
/// resolved path is `.../sqlalchemy/orm/__init__.py`, but the marker lives
/// at `.../sqlalchemy/py.typed`. We walk up `depth(module)` levels from the
/// resolved file's parent to reach the top-level package directory.
fn has_py_typed_marker(import: &ImportInfo) -> bool {
    let Some(resolved) = import.resolved_path.as_ref() else {
        return false;
    };
    let Some(mut pkg_dir) = resolved.parent() else {
        return false;
    };
    // For nested imports like `sqlalchemy.orm.session` we need to climb up
    // one directory per dot in the dotted module name to reach the top
    // package directory. Single-file modules (`flask.py`) and top-level
    // packages have zero dots and need no climbing.
    let depth = import.module.matches('.').count();
    for _ in 0..depth {
        let Some(parent) = pkg_dir.parent() else {
            return false;
        };
        pkg_dir = parent;
    }
    pkg_dir.join("py.typed").is_file()
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
            "Type stubs available as `types-{root_module}` — use quick fix to install"
        )),
        note: Some(
            "Packages without type stubs or a `py.typed` marker provide no type information"
                .to_owned(),
        ),
        provenance: Some(TypeProvenance::Untyped),
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
    use std::fs;
    use std::path::PathBuf;

    fn make_module(imports: Vec<ImportInfo>) -> ResolvedModule {
        ResolvedModule {
            path: "test.py".to_owned(),
            imports,
            ..ResolvedModule::default()
        }
    }

    /// Build an `ImportInfo` with the given module name, resolution, and resolved path.
    /// All other fields default to safe blanks for these tests.
    fn make_import(
        module: &str,
        span_end: u32,
        resolution: ImportResolution,
        resolved_path: Option<&str>,
    ) -> ImportInfo {
        ImportInfo {
            module: module.to_owned(),
            names: vec![],
            span: Span::new(0, span_end),
            kind: ImportKind::Plain,
            resolution,
            resolved_path: resolved_path.map(PathBuf::from),
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        }
    }

    fn run_check(import: ImportInfo) -> Vec<crate::Diagnostic> {
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();
        MissingTypeStubs.check(&module, &mut diagnostics);
        diagnostics
    }

    #[test]
    fn fires_for_site_packages_source_py() {
        let import = make_import(
            "flask",
            12,
            ImportResolution::SourcePy,
            Some("/venv/lib/python3.12/site-packages/flask/__init__.py"),
        );
        let diagnostics = run_check(import);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code.code, "BSK-W0010");
    }

    #[test]
    fn skips_workspace_source_py() {
        let import = make_import(
            "myapp",
            12,
            ImportResolution::SourcePy,
            Some("/workspace/myapp/__init__.py"),
        );
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn skips_stdlib_modules() {
        let import = make_import(
            "os",
            9,
            ImportResolution::SourcePy,
            Some("/venv/lib/python3.12/site-packages/os/__init__.py"),
        );
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn skips_stub_pyi_resolution() {
        let import = make_import(
            "requests",
            15,
            ImportResolution::StubPyi,
            Some("/venv/lib/python3.12/site-packages/requests-stubs/__init__.pyi"),
        );
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn skips_unresolved_imports() {
        let import = make_import("nonexistent", 18, ImportResolution::Unresolved, None);
        assert!(run_check(import).is_empty());
    }

    #[test]
    fn skips_site_packages_package_with_py_typed_marker() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let package = dir
            .path()
            .join("lib")
            .join("python3.12")
            .join("site-packages")
            .join("httpx_fake");
        fs::create_dir_all(&package)?;
        fs::write(package.join("py.typed"), "")?;
        let init_path = package.join("__init__.py");
        fs::write(&init_path, "def get(url: str) -> str: ...\n")?;

        let import = ImportInfo {
            module: "httpx_fake".to_owned(),
            names: vec![],
            span: Span::new(0, 16),
            kind: ImportKind::Plain,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(init_path),
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();

        MissingTypeStubs.check(&module, &mut diagnostics);

        assert!(
            diagnostics.is_empty(),
            "PEP 561 inline-typed packages must not emit BSK-W0010"
        );
        Ok(())
    }

    /// Regression for issue #13: `from sqlalchemy.orm import session` resolves
    /// to `.../sqlalchemy/orm/__init__.py`. The `py.typed` marker lives at the
    /// package root (`.../sqlalchemy/py.typed`), not next to the resolved
    /// submodule. The marker check must walk up to the top-level package.
    #[test]
    fn skips_nested_submodule_when_root_package_has_py_typed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root_pkg = dir
            .path()
            .join("lib")
            .join("python3.12")
            .join("site-packages")
            .join("sqlalchemy_fake");
        let sub_pkg = root_pkg.join("orm");
        fs::create_dir_all(&sub_pkg)?;
        // py.typed lives at the top-level package only — per PEP 561 it
        // applies to the entire package and all its submodules.
        fs::write(root_pkg.join("py.typed"), "")?;
        fs::write(root_pkg.join("__init__.py"), "")?;
        let sub_init = sub_pkg.join("__init__.py");
        fs::write(&sub_init, "class Session: ...\n")?;

        let import = ImportInfo {
            module: "sqlalchemy_fake.orm".to_owned(),
            names: vec!["Session".to_owned()],
            span: Span::new(0, 32),
            kind: ImportKind::From,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(sub_init),
            package_dep_kind: None,
            package_version: None,
            package_name: None,
            unresolved_reason: None,
        };
        let module = make_module(vec![import]);
        let mut diagnostics = Vec::new();

        MissingTypeStubs.check(&module, &mut diagnostics);

        assert!(
            diagnostics.is_empty(),
            "PEP 561 py.typed at the top-level package must apply to all submodules; \
             got: {diagnostics:?}"
        );
        Ok(())
    }
}
