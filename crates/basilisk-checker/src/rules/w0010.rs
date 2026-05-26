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
/// to every submodule. A submodule may resolve either to a flat-file module
/// (`.../pydantic_ai/direct.py`) or to a nested subpackage
/// (`.../sqlalchemy/orm/__init__.py`); in both cases the marker lives at the
/// top-level package root (`.../pydantic_ai/py.typed`, `.../sqlalchemy/py.typed`).
///
/// We walk the filesystem upward from the resolved file toward the
/// `site-packages` root, returning `true` at the first directory that contains
/// a `py.typed` marker. Walking the directory tree (rather than counting dots
/// in the dotted module name) honors flat-file and subpackage submodules
/// uniformly and never over-climbs past the package into `site-packages`.
fn has_py_typed_marker(import: &ImportInfo) -> bool {
    import
        .resolved_path
        .as_ref()
        .is_some_and(|resolved| basilisk_stubs::has_py_typed_marker(resolved))
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
        help: Some(stub_help_text(root_module)),
        note: Some(
            "Packages without type stubs or a `py.typed` marker provide no type information"
                .to_owned(),
        ),
        provenance: Some(TypeProvenance::Untyped),
    }
}

/// Build the `help` line for a missing-stubs diagnostic.
///
/// When typeshed publishes a stub distribution for the package, point the user
/// at the real distribution name (e.g. `yaml` → `types-PyYAML`) so the quick fix
/// resolves. Otherwise say so plainly — there is no installable stub to suggest.
fn stub_help_text(root_module: &str) -> String {
    match basilisk_stubs::typeshed_stub_distribution(root_module) {
        Some(distribution) => {
            format!("Type stubs available as `{distribution}` — use quick fix to install")
        }
        None => format!(
            "No published type stubs for `{root_module}` — add inline types (`py.typed`) upstream \
             or provide a local stub"
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

    /// Regression for issue #46: the help text must name the *real* typeshed
    /// distribution (`yaml` → `types-PyYAML`), not a fabricated `types-yaml`.
    #[test]
    fn help_text_uses_real_typeshed_distribution_name() {
        let import = make_import(
            "yaml",
            11,
            ImportResolution::SourcePy,
            Some("/venv/lib/python3.12/site-packages/yaml/__init__.py"),
        );
        let diagnostics = run_check(import);
        assert_eq!(diagnostics.len(), 1);
        let help = diagnostics[0].help.as_deref().unwrap_or_default();
        assert!(
            help.contains("types-PyYAML"),
            "help must name the real typeshed distribution, got: {help}"
        );
    }

    /// Regression for issue #46: when no stub distribution exists, the help must
    /// say so plainly rather than inviting a quick fix that is never offered.
    #[test]
    fn help_text_states_no_stubs_for_unknown_package() {
        let import = make_import(
            "acme_private_pkg",
            20,
            ImportResolution::SourcePy,
            Some("/venv/lib/python3.12/site-packages/acme_private_pkg/__init__.py"),
        );
        let diagnostics = run_check(import);
        assert_eq!(diagnostics.len(), 1);
        let help = diagnostics[0].help.as_deref().unwrap_or_default();
        assert!(
            help.contains("No published type stubs"),
            "help must state that no stubs exist, got: {help}"
        );
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

    /// Regression for issue #46: `from pydantic_ai.direct import model_request`
    /// resolves to a **flat-file** submodule `.../pydantic_ai/direct.py` (NOT a
    /// subpackage `__init__.py`). The resolved file's parent is *already* the
    /// top-level package directory, so climbing `depth` (one dot) levels
    /// over-climbs into `site-packages` and misses the marker, producing a
    /// false-positive W0010. The marker check must locate the top-level package
    /// regardless of whether the submodule is a flat file or a subpackage.
    #[test]
    fn skips_flat_file_submodule_when_root_package_has_py_typed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root_pkg = dir
            .path()
            .join("lib")
            .join("python3.12")
            .join("site-packages")
            .join("pydantic_ai_fake");
        fs::create_dir_all(&root_pkg)?;
        // py.typed lives at the top-level package; `direct` is a flat module.
        fs::write(root_pkg.join("py.typed"), "")?;
        fs::write(root_pkg.join("__init__.py"), "")?;
        let flat_module = root_pkg.join("direct.py");
        fs::write(&flat_module, "def model_request() -> None: ...\n")?;

        let import = ImportInfo {
            module: "pydantic_ai_fake.direct".to_owned(),
            names: vec!["model_request".to_owned()],
            span: Span::new(0, 40),
            kind: ImportKind::From,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(flat_module),
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
            "PEP 561 py.typed must be honored for flat-file submodules; got: {diagnostics:?}"
        );
        Ok(())
    }

    /// Regression for issue #46 (acceptance: the `<pkg>.<subpkg>.<deeper>` shape):
    /// `from pkg.sub.deep import x` resolves to `.../pkg/sub/deep/__init__.py`,
    /// three segments below the top-level package. The marker at the package
    /// root must still be honored no matter how deep the submodule nests.
    #[test]
    fn skips_deeper_nested_submodule_when_root_package_has_py_typed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root_pkg = dir
            .path()
            .join("lib")
            .join("python3.12")
            .join("site-packages")
            .join("deeppkg_fake");
        let deep = root_pkg.join("sub").join("deep");
        fs::create_dir_all(&deep)?;
        fs::write(root_pkg.join("py.typed"), "")?;
        fs::write(root_pkg.join("__init__.py"), "")?;
        fs::write(root_pkg.join("sub").join("__init__.py"), "")?;
        let deep_init = deep.join("__init__.py");
        fs::write(&deep_init, "def helper() -> int: ...\n")?;

        let import = ImportInfo {
            module: "deeppkg_fake.sub.deep".to_owned(),
            names: vec!["helper".to_owned()],
            span: Span::new(0, 40),
            kind: ImportKind::From,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(deep_init),
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
            "PEP 561 py.typed at the root must apply to deeply nested submodules; \
             got: {diagnostics:?}"
        );
        Ok(())
    }

    /// Regression for issue #46 (acceptance: `from httpx._client import Client`):
    /// the underscore-prefixed flat-file submodule pattern. `httpx/_client.py` is
    /// a flat file whose parent is already the top-level package, so the marker
    /// at `httpx/py.typed` must be found without over-climbing.
    #[test]
    fn skips_httpx_underscore_flat_submodule_when_root_has_py_typed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root_pkg = dir
            .path()
            .join("lib")
            .join("python3.12")
            .join("site-packages")
            .join("httpx_fake");
        fs::create_dir_all(&root_pkg)?;
        fs::write(root_pkg.join("py.typed"), "")?;
        fs::write(root_pkg.join("__init__.py"), "")?;
        let client = root_pkg.join("_client.py");
        fs::write(&client, "class Client: ...\n")?;

        let import = ImportInfo {
            module: "httpx_fake._client".to_owned(),
            names: vec!["Client".to_owned()],
            span: Span::new(0, 34),
            kind: ImportKind::From,
            resolution: ImportResolution::SourcePy,
            resolved_path: Some(client),
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
            "PEP 561 py.typed must be honored for underscore flat-file submodules; \
             got: {diagnostics:?}"
        );
        Ok(())
    }
}
