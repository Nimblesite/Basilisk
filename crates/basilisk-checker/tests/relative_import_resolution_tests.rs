//! Tests for [ANALYSIS-INCR-IMPORTS]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INCR-IMPORTS
#![allow(
    clippy::allow_attributes,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    missing_docs
)]
//! Relative-import resolution through the full `resolve_module_imports`
//! pipeline (GitHub #369): `from ..sub import mod` must resolve by walking up
//! from the importing file's package, not by treating `sub` as an absolute
//! module. The standalone `resolve_relative_import` unit tests in
//! `import_resolution_tests.rs` pass in isolation — these tests pin the wiring
//! from the parsed AST (`StmtImportFrom.level`) through `ImportInfo` to the
//! resolver dispatch in `basilisk_checker::imports::resolve_module_imports`.

use std::fs;
use std::path::Path;

use basilisk_resolver::scope::ImportResolution;

mod import_support;
use import_support::{make_search_paths, unique_tmp};

/// The issue #369 package layout:
///
/// ```text
/// <root>/src/pkg/__init__.py
/// <root>/src/pkg/sub/__init__.py
/// <root>/src/pkg/sub/mod.py
/// <root>/src/pkg/dev/__init__.py
/// <root>/src/pkg/dev/other.py
/// ```
///
/// Returns the workspace root; the importing file lives in `src/pkg/dev/`.
fn make_issue_369_layout(prefix: &str) -> std::path::PathBuf {
    let root = unique_tmp(prefix);
    let dev = root.join("src").join("pkg").join("dev");
    let sub = root.join("src").join("pkg").join("sub");
    fs::create_dir_all(&dev).unwrap();
    fs::create_dir_all(&sub).unwrap();
    fs::write(root.join("src").join("pkg").join("__init__.py"), "").unwrap();
    fs::write(dev.join("__init__.py"), "").unwrap();
    fs::write(dev.join("other.py"), "class Bar: pass\n").unwrap();
    fs::write(sub.join("__init__.py"), "").unwrap();
    fs::write(sub.join("mod.py"), "class Foo: pass\n").unwrap();
    root
}

/// Parse `source` as a file at `importing_file` and run the real pipeline:
/// visitor capture (`basilisk_resolver::resolve`) followed by
/// `resolve_module_imports` — the exact path the CLI and the salsa
/// `resolved_module` query share.
fn resolve_imports_of(
    source: &str,
    importing_file: &Path,
    root: &Path,
) -> basilisk_resolver::ResolvedModule {
    let parsed = basilisk_parser::parse_source(
        source.to_owned(),
        importing_file.to_string_lossy().into_owned(),
    )
    .expect("fixture parses");
    let mut resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let paths = make_search_paths(vec![root.to_path_buf()]);
    basilisk_checker::imports::resolve_module_imports(&mut resolved, &paths);
    resolved
}

/// Issue #369 case 2: `from ..sub import mod` — double dot, bare module
/// target. Must resolve to the sibling package's `__init__.py`, one level up
/// from the importing file's package.
#[test]
fn double_dot_bare_module_target_resolves_to_parent_sibling_package() {
    let root = make_issue_369_layout("bsk_rel369_dotdot");
    let importing = root.join("src").join("pkg").join("dev").join("user.py");

    let resolved = resolve_imports_of("from ..sub import mod\n", &importing, &root);

    let import = &resolved.imports[0];
    assert_ne!(
        import.resolution,
        ImportResolution::Unresolved,
        "`from ..sub import mod` must resolve `..sub` relative to the \
         importing package (GitHub #369); got unresolved_reason {:?}",
        import.unresolved_reason
    );
    let path = import.resolved_path.as_ref().unwrap();
    assert!(
        path.ends_with(Path::new("src/pkg/sub/__init__.py")),
        "`..sub` must resolve to src/pkg/sub/__init__.py, got {path:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Issue #369 case 3: `from ..sub.mod import Foo` — double dot, dotted
/// attribute target. Must resolve to `src/pkg/sub/mod.py`.
#[test]
fn double_dot_dotted_module_target_resolves_through_parent_package() {
    let root = make_issue_369_layout("bsk_rel369_dotted");
    let importing = root.join("src").join("pkg").join("dev").join("user.py");

    let resolved = resolve_imports_of("from ..sub.mod import Foo\n", &importing, &root);

    let import = &resolved.imports[0];
    assert_ne!(
        import.resolution,
        ImportResolution::Unresolved,
        "`from ..sub.mod import Foo` must resolve relative to the importing \
         package (GitHub #369); got unresolved_reason {:?}",
        import.unresolved_reason
    );
    let path = import.resolved_path.as_ref().unwrap();
    assert!(
        path.ends_with(Path::new("src/pkg/sub/mod.py")),
        "`..sub.mod` must resolve to src/pkg/sub/mod.py, got {path:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

/// `from . import other` — bare single dot with no module path. The import
/// target is the importing file's own package `__init__.py`; the bound name
/// `other` is a sibling submodule.
#[test]
fn bare_dot_import_resolves_to_own_package_init() {
    let root = make_issue_369_layout("bsk_rel369_bare");
    let importing = root.join("src").join("pkg").join("dev").join("user.py");

    let resolved = resolve_imports_of("from . import other\n", &importing, &root);

    let import = &resolved.imports[0];
    assert_ne!(
        import.resolution,
        ImportResolution::Unresolved,
        "`from . import other` must resolve to the importing file's package \
         __init__.py (GitHub #369); got unresolved_reason {:?}",
        import.unresolved_reason
    );
    let path = import.resolved_path.as_ref().unwrap();
    assert!(
        path.ends_with(Path::new("src/pkg/dev/__init__.py")),
        "`from . import other` must resolve to src/pkg/dev/__init__.py, got {path:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

/// `from .. import sub` — bare double dot binding a subpackage of the parent.
#[test]
fn bare_double_dot_import_resolves_to_parent_package_init() {
    let root = make_issue_369_layout("bsk_rel369_bare2");
    let importing = root.join("src").join("pkg").join("dev").join("user.py");

    let resolved = resolve_imports_of("from .. import sub\n", &importing, &root);

    let import = &resolved.imports[0];
    assert_ne!(
        import.resolution,
        ImportResolution::Unresolved,
        "`from .. import sub` must resolve to the parent package __init__.py \
         (GitHub #369); got unresolved_reason {:?}",
        import.unresolved_reason
    );
    let path = import.resolved_path.as_ref().unwrap();
    assert!(
        path.ends_with(Path::new("src/pkg/__init__.py")),
        "`from .. import sub` must resolve to src/pkg/__init__.py, got {path:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

/// `from ..sub import *` — the star-import form must dispatch through the same
/// relative resolution as the named form.
#[test]
fn double_dot_star_import_resolves_to_parent_sibling_package() {
    let root = make_issue_369_layout("bsk_rel369_star");
    let importing = root.join("src").join("pkg").join("dev").join("user.py");

    let resolved = resolve_imports_of("from ..sub import *\n", &importing, &root);

    let import = &resolved.imports[0];
    assert_ne!(
        import.resolution,
        ImportResolution::Unresolved,
        "`from ..sub import *` must resolve `..sub` relative to the importing \
         package (GitHub #369); got unresolved_reason {:?}",
        import.unresolved_reason
    );
    let path = import.resolved_path.as_ref().unwrap();
    assert!(
        path.ends_with(Path::new("src/pkg/sub/__init__.py")),
        "`..sub` must resolve to src/pkg/sub/__init__.py, got {path:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

/// Issue #369 case 1: `from .other import Bar` — single dot, sibling module.
/// Passes today only by accident (the absolute resolver's importer-directory
/// fallback); pinned here so the relative dispatch keeps it working on purpose.
#[test]
fn single_dot_sibling_module_resolves() {
    let root = make_issue_369_layout("bsk_rel369_dot");
    let importing = root.join("src").join("pkg").join("dev").join("user.py");

    let resolved = resolve_imports_of("from .other import Bar\n", &importing, &root);

    let import = &resolved.imports[0];
    assert_ne!(import.resolution, ImportResolution::Unresolved);
    let path = import.resolved_path.as_ref().unwrap();
    assert!(
        path.ends_with(Path::new("src/pkg/dev/other.py")),
        "`.other` must resolve to src/pkg/dev/other.py, got {path:?}"
    );

    let _ = fs::remove_dir_all(&root);
}

/// A relative import whose target genuinely does not exist stays unresolved —
/// the fix must add resolution, not blanket-accept every nonzero level.
#[test]
fn missing_relative_target_stays_unresolved() {
    let root = make_issue_369_layout("bsk_rel369_missing");
    let importing = root.join("src").join("pkg").join("dev").join("user.py");

    let resolved = resolve_imports_of("from ..nonexistent import x\n", &importing, &root);

    let import = &resolved.imports[0];
    assert_eq!(
        import.resolution,
        ImportResolution::Unresolved,
        "a relative import of a module that does not exist must stay unresolved"
    );

    let _ = fs::remove_dir_all(&root);
}

/// An absolute import is untouched by the relative dispatch: level 0 keeps the
/// full search-path walk (here: workspace-root resolution of `src`-less
/// absolute names still fails, exactly as before).
#[test]
fn absolute_import_still_walks_search_paths() {
    let root = make_issue_369_layout("bsk_rel369_abs");
    let importing = root.join("src").join("pkg").join("dev").join("user.py");
    // `src` is not on the search path as a package root, so the absolute name
    // `pkg.sub` must NOT resolve — only the relative form reaches it.
    let resolved = resolve_imports_of("from pkg.sub import mod\n", &importing, &root);
    assert_eq!(resolved.imports[0].resolution, ImportResolution::Unresolved);

    let _ = fs::remove_dir_all(&root);
}
