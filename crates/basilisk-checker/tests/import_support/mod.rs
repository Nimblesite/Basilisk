//! Shared fixtures for the import-resolution integration tests.
//! Implements [ANALYSIS-CROSSLSP-IMPORT] / [ANALYSIS-INCR-IMPORTS] test support.
#![allow(
    dead_code,
    clippy::allow_attributes,
    clippy::unwrap_used,
    reason = "each import test binary uses a subset of these shared fixtures"
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use basilisk_checker::imports::{ActiveTypeshed, ImportSearchPaths};
use basilisk_resolver::scope::{ImportKind, ImportResolution};
use basilisk_stubs::typeshed::archive::{Archive, ArchiveEntry, ArchiveVfs};
use basilisk_stubs::typeshed::gittree::FileMode;
use basilisk_stubs::typeshed::snapshot::Snapshot;
use basilisk_stubs::typeshed::source::{LicenseStatus, SourceIdentity, SourceKind, TypeshedStatus};

static TEST_CTR: AtomicU64 = AtomicU64::new(0);

/// Generate a unique temp dir path to avoid races between parallel tests.
pub fn unique_tmp(prefix: &str) -> PathBuf {
    let n = TEST_CTR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{n}_{}", std::process::id()))
}

/// Create a unique tmp dir named `<prefix>_<n>_<pid>` and return its path.
/// The dir is left in place; tests should clean up with `fs::remove_dir_all` at the end.
pub fn make_tmp_dir(prefix: &str) -> PathBuf {
    let dir = unique_tmp(prefix);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Create `<parent>/<pkg_name>/` and populate it with empty files named in
/// `files`. Returns the package directory path.
pub fn make_pkg(parent: &Path, pkg_name: &str, files: &[&str]) -> PathBuf {
    let pkg = parent.join(pkg_name);
    std::fs::create_dir_all(&pkg).unwrap();
    for f in files {
        std::fs::write(pkg.join(f), "").unwrap();
    }
    pkg
}

/// Search paths rooted at `roots`, with every other search location empty.
pub fn make_search_paths(roots: Vec<PathBuf>) -> ImportSearchPaths {
    ImportSearchPaths {
        roots,
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_snapshot: None,
    }
}

/// Build a gate-equivalent immutable custom Typeshed snapshot from raw stdlib
/// paths such as `os.pyi` or `os/__init__.pyi`.
pub fn custom_typeshed_snapshot(files: &[(&str, &str)]) -> ActiveTypeshed {
    let identity = SourceIdentity::Custom {
        digest: "integration-custom".to_owned(),
    };
    let versions = files.iter().fold(String::new(), |mut versions, (path, _)| {
        let module = path
            .trim_end_matches("/__init__.pyi")
            .trim_end_matches(".pyi")
            .replace('/', ".");
        // Writing into a `String` cannot fail, so the `Result` is dropped
        // rather than asserted on.
        let _ = writeln!(&mut versions, "{module}: 3.0-");
        versions
    });
    let mut entries = vec![ArchiveEntry {
        path: "stdlib/VERSIONS".to_owned(),
        mode: FileMode::Regular,
        data: versions.into_bytes(),
    }];
    entries.extend(files.iter().map(|(path, body)| ArchiveEntry {
        path: format!("stdlib/{path}"),
        mode: FileMode::Regular,
        data: body.as_bytes().to_vec(),
    }));
    let status = TypeshedStatus {
        active_source: SourceKind::Custom,
        commit: None,
        tree: None,
        license_status: LicenseStatus::NotSupplied,
        license_reference: None,
        warnings: Vec::new(),
    };
    let uri_identity = identity.uri_component();
    let snapshot = Snapshot::build(
        identity,
        status,
        ArchiveVfs::new(uri_identity, Archive::new(entries)),
        None,
    )
    .unwrap();
    ActiveTypeshed::new(Arc::new(snapshot), None)
}

/// Build a `ResolvedModule` with a single plain `import <module>` statement.
pub fn module_with_plain_import(module: &str) -> basilisk_resolver::ResolvedModule {
    module_with_plain_imports(&[module])
}

/// Build a `ResolvedModule` with one plain `import <m>` statement per `modules`.
pub fn module_with_plain_imports(modules: &[&str]) -> basilisk_resolver::ResolvedModule {
    basilisk_resolver::ResolvedModule {
        path: "test.py".to_owned(),
        imports: modules
            .iter()
            .map(|module| basilisk_resolver::ImportInfo {
                module: (*module).to_owned(),
                names: vec![],
                span: basilisk_resolver::Span::new(0, 0),
                name_spans: Vec::new(),
                kind: ImportKind::Plain,
                resolution: ImportResolution::Unresolved,
                resolved_path: None,
                package_dep_kind: None,
                package_version: None,
                package_name: None,
                stub_distribution: None,
                unresolved_reason: None,
            })
            .collect(),
        ..basilisk_resolver::ResolvedModule::default()
    }
}
