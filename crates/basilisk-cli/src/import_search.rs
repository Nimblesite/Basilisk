//! CLI import-search setup fast paths.
//!
//! Implements [ANALYSIS-CROSSLSP-IMPORT]. Search-path discovery probes Python,
//! uv metadata, and nested projects. None of those paths can affect a resolved
//! module with no import statements, so a proven no-import batch keeps only its
//! roots and avoids that fixed setup cost.

use std::path::PathBuf;

use basilisk_lsp::import_resolver::ImportSearchPaths;

const IMPORT_KEYWORD: &[u8] = b"import";

/// Return `true` unless every source file proves it contains no Python import
/// keyword.
///
/// The byte search is deliberately conservative. Strings, comments, and names
/// such as `important` can produce a false positive and take the full discovery
/// path. They cannot produce a false negative: Python's `import` keyword is
/// always the literal lowercase ASCII token. An unreadable file also fails open
/// so the ordinary analysis path retains its existing error behaviour.
pub(crate) fn files_might_import(paths: &[String]) -> bool {
    paths.iter().any(|path| match std::fs::read(path) {
        Ok(source) => source_might_import(&source),
        Err(_) => true,
    })
}

fn source_might_import(source: &[u8]) -> bool {
    source
        .windows(IMPORT_KEYWORD.len())
        .any(|window| window == IMPORT_KEYWORD)
}

/// Build the complete search-path value required by the resolver when there
/// are no imports to resolve. Roots are retained for API consistency; all
/// import-only fields are empty by proof from [`files_might_import`].
pub(crate) fn roots_only(roots: Vec<PathBuf>) -> ImportSearchPaths {
    ImportSearchPaths {
        roots,
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_statements_take_full_discovery_path() {
        assert!(source_might_import(b"import package\n"));
        assert!(source_might_import(b"from package import symbol\n"));
    }

    #[test]
    fn import_free_source_takes_roots_only_path() {
        assert!(!source_might_import(b"value: int = 42\n"));
    }

    #[test]
    fn false_positives_are_conservative() {
        assert!(source_might_import(b"important = 'not syntax'\n"));
        assert!(source_might_import(b"# import mentioned in a comment\n"));
    }

    #[test]
    fn unreadable_source_fails_open() {
        assert!(files_might_import(&[format!(
            "/path/that/does/not/exist/{}",
            std::process::id()
        )]));
    }

    #[test]
    fn roots_only_retains_roots_and_empties_import_fields() {
        let root = PathBuf::from("/workspace");
        let paths = roots_only(vec![root.clone()]);

        assert_eq!(paths.roots, vec![root]);
        assert!(paths.extra_paths.is_empty());
        assert!(paths.stub_paths.is_empty());
        assert!(paths.workspace_members.is_empty());
        assert!(paths.site_packages.is_none());
        assert!(paths.registry.is_none());
        assert!(paths.typeshed_snapshot.is_none());
    }
}
