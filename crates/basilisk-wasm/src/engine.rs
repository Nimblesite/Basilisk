//! Implements [WASM-PIPELINE] and [WASM-NOFS].
//! See docs/specs/WASM-SPEC.md#WASM-PIPELINE
//!
//! The four checking stages, driven exactly as `basilisk check` drives them
//! (`basilisk-cli/src/pipeline/mod.rs`), with `parse_file` swapped for
//! `parse_source` because there is no file to read. No checking logic lives
//! here: a rule change reaches the browser without touching this crate.

use std::sync::Arc;

use basilisk_checker::imports::{ActiveTypeshed, ImportSearchPaths};

use crate::options::CheckOptions;
use crate::report::Report;

/// Check one Python source string and report every diagnostic.
///
/// Never panics: a parse or scope-resolution failure becomes a diagnostic
/// ([WASM-PIPELINE]), because the caller is an editor that wants to render the
/// problem, not a stack trace.
#[must_use]
pub fn check_source(source: &str, options: &CheckOptions) -> Report {
    let path = options.path();
    let config = options.to_config();
    let target_version =
        basilisk_checker::context::CheckContext::from_config(&config).target_version;

    let parsed = match basilisk_parser::parse_source(source.to_owned(), path.to_owned()) {
        Ok(parsed) => parsed,
        Err(error) => return Report::from_failure(path, &error.to_string()),
    };

    let mut resolved = match target_version {
        Some(target) => basilisk_resolver::resolve_with_target(&parsed, target),
        None => basilisk_resolver::resolve(&parsed),
    };
    let resolved = match resolved.as_mut() {
        Ok(resolved) => resolved,
        Err(error) => return Report::from_failure(path, &error.to_string()),
    };

    match search_paths(target_version) {
        Ok(search_paths) => {
            basilisk_checker::imports::resolve_module_imports(resolved, &search_paths);
        }
        Err(error) => return Report::from_failure(path, &error),
    }

    Report::new(
        &basilisk_checker::check_with_config(resolved, &config),
        source,
    )
}

/// Search paths that reach nothing outside this module ([WASM-NOFS]).
///
/// Every root is empty, so the directory-listing cache backing import
/// resolution is never asked to read a directory, and the only source of
/// stdlib stubs is the typeshed snapshot embedded in the binary. `import
/// typing` therefore resolves in a browser exactly as it does on disk, while
/// `import numpy` correctly reports unresolved — nothing is installed
/// ([WASM-LIMITS]).
fn search_paths(target_version: Option<(u32, u32)>) -> Result<ImportSearchPaths, String> {
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot().map_err(|error| {
        format!("basilisk-wasm: the embedded typeshed bundle could not be decoded: {error}")
    })?;

    Ok(ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: Some(ActiveTypeshed::new(
            Arc::new(snapshot),
            target_version.map(stub_target),
        )),
    })
}

/// Target evidence for version- and platform-guarded `.pyi` declarations.
///
/// The platform is always the cross-platform intersection: a playground has no
/// host to speak for, so a declaration is only offered when it is valid
/// everywhere. That is the conservative choice — it never invents support that
/// a reader's own platform lacks.
fn stub_target(python_version: (u32, u32)) -> basilisk_stubs::types::StubTarget {
    basilisk_stubs::types::StubTarget {
        python_version,
        platform: basilisk_stubs::types::StubTargetPlatform::All,
    }
}
