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
    MissingTypeStubs.check(
        &module,
        &crate::context::CheckContext::default(),
        &mut diagnostics,
    );
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
    assert_eq!(diagnostics[0].code.code, "BSK-E0152");
}

/// When this opt-in rule fires, an untyped third-party import is a hard ERROR,
/// not a warning. A project can soften it (`"BSK-E0152" = "warning"`) to import
/// at its own risk; this asserts the rule's default severity is an error.
#[test]
fn defaults_to_error_severity() {
    let import = make_import(
        "flask",
        12,
        ImportResolution::SourcePy,
        Some("/venv/lib/python3.12/site-packages/flask/__init__.py"),
    );
    let diagnostics = run_check(import);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].severity,
        Severity::Error,
        "missing type stubs must default to a hard error"
    );
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
///
/// It must also tell the developer *how* to fill the gap: name the local-stub
/// escape hatch (`stub-paths` + a `.pyi` file) and link the official authoring
/// guide, so a developer (or an AI in the editor) has self-contained context.
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
    assert!(
        help.contains("stub-paths"),
        "help must name the `stub-paths` config so the user knows where local stubs go, got: {help}"
    );
    assert!(
        help.contains(".pyi"),
        "help must name the `.pyi` stub file the user should create, got: {help}"
    );
    assert!(
        help.contains("https://typing.python.org/en/latest/guides/writing_stubs.html"),
        "help must link the official stub-writing guide, got: {help}"
    );
}

/// The shared note must carry the canonical PEP 561 reference so the editor
/// surfaces *why* the package is untyped and points at the spec online.
#[test]
fn note_links_pep561_and_mentions_py_typed() {
    let import = make_import(
        "acme_private_pkg",
        20,
        ImportResolution::SourcePy,
        Some("/venv/lib/python3.12/site-packages/acme_private_pkg/__init__.py"),
    );
    let diagnostics = run_check(import);
    assert_eq!(diagnostics.len(), 1);
    let note = diagnostics[0].note.as_deref().unwrap_or_default();
    assert!(
        note.contains("py.typed"),
        "note must mention the PEP 561 `py.typed` marker, got: {note}"
    );
    assert!(
        note.contains("https://peps.python.org/pep-0561/"),
        "note must link PEP 561 online, got: {note}"
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
fn skips_site_packages_package_with_py_typed_marker() -> Result<(), Box<dyn std::error::Error>> {
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

    MissingTypeStubs.check(
        &module,
        &crate::context::CheckContext::default(),
        &mut diagnostics,
    );

    assert!(
        diagnostics.is_empty(),
        "PEP 561 inline-typed packages must not emit BSK-E0152"
    );
    Ok(())
}

/// Regression for issue #13: `from sqlalchemy.orm import session` resolves
/// to `.../sqlalchemy/orm/__init__.py`. The `py.typed` marker lives at the
/// package root (`.../sqlalchemy/py.typed`), not next to the resolved
/// submodule. The marker check must walk up to the top-level package.
#[test]
fn skips_nested_submodule_when_root_package_has_py_typed() -> Result<(), Box<dyn std::error::Error>>
{
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

    MissingTypeStubs.check(
        &module,
        &crate::context::CheckContext::default(),
        &mut diagnostics,
    );

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
/// false-positive E0152. The marker check must locate the top-level package
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

    MissingTypeStubs.check(
        &module,
        &crate::context::CheckContext::default(),
        &mut diagnostics,
    );

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

    MissingTypeStubs.check(
        &module,
        &crate::context::CheckContext::default(),
        &mut diagnostics,
    );

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

    MissingTypeStubs.check(
        &module,
        &crate::context::CheckContext::default(),
        &mut diagnostics,
    );

    assert!(
        diagnostics.is_empty(),
        "PEP 561 py.typed must be honored for underscore flat-file submodules; \
         got: {diagnostics:?}"
    );
    Ok(())
}
