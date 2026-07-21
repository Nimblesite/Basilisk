//! Implements [ANALYSIS-INDEX-STRUCT]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INDEX-STRUCT
//! File-system scanning utilities for the workspace index.
//!
//! Provides recursive Python file collection, exclusion filtering, path-aware
//! deduplication (`.pyi` beats `.py`), and `Path → Url` conversion.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;

/// Recursively collect all `.py` / `.pyi` files under `dir`, skipping hidden
/// dirs and the caller's exclude patterns.
///
/// `exclude` is the whole truth about what gets skipped. It arrives already
/// resolved to the effective list — [`basilisk_config::DEFAULT_EXCLUDES`] when
/// the workspace configures nothing, and the configured patterns *instead of*
/// those defaults once it does ([CHKARCH-CONFIG-EXCLUDE]). Applying a second,
/// hardcoded default set here would make the editor skip trees that no
/// configuration could re-enable, so `basilisk check` would report on files
/// the editor silently ignored.
pub fn collect_python_files(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    exclude: &[PathBuf],
    workspace_root: &Path,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Hidden directories are always skipped, exactly as the CLI walk
            // does — that rule is not configurable in either surface.
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if is_excluded(&path, exclude, workspace_root) {
                continue;
            }
            collect_python_files(&path, out, exclude, workspace_root);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "py" || ext == "pyi")
            && !is_excluded(&path, exclude, workspace_root)
        {
            // File-level globs (e.g. `*.pb.py`, `**/conftest.py`) are honoured
            // here; directory globs are already pruned above before recursing.
            out.push(path);
        }
    }
}

/// Check if a path matches any configured exclude pattern.
///
/// Patterns are matched against the path relative to the workspace root using
/// gitignore-style globs (see [`basilisk_config::path_matches_pattern`]), so
/// `**/bundled/**`, `vendor/**`, `build`, and `*.pb.py` all work as expected.
#[must_use]
pub fn is_excluded(path: &Path, exclude: &[PathBuf], workspace_root: &Path) -> bool {
    let relative = path.strip_prefix(workspace_root).unwrap_or(path);
    exclude
        .iter()
        .any(|pattern| basilisk_config::path_matches_pattern(relative, &pattern.to_string_lossy()))
}

/// Deduplicate `.py` / `.pyi` files at the same path, preferring `.pyi`.
#[must_use]
pub fn deduplicate_by_stem(files: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut by_path: HashMap<PathBuf, PathBuf> = HashMap::new();
    for path in files {
        let ext = path.extension().and_then(|s| s.to_str());
        let key = path.with_extension("");
        match by_path.get(&key) {
            Some(existing) => {
                if existing.extension().and_then(|s| s.to_str()) == Some("py") && ext == Some("pyi")
                {
                    let _ = by_path.insert(key, path);
                }
            }
            None => {
                let _ = by_path.insert(key, path);
            }
        }
    }
    by_path.into_values().collect()
}

/// Convert a filesystem path to an LSP `Url`.
#[must_use]
pub fn path_to_uri(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}

#[cfg(test)]
mod tests {
    use super::{collect_python_files, deduplicate_by_stem};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_root(prefix: &str) -> PathBuf {
        static CTR: AtomicU64 = AtomicU64::new(0);
        let n = CTR.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "bsk_scan_exclude_{prefix}_{}_{n}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn write(root: &Path, rel: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, "value: int = 1\n");
    }

    fn scan(root: &Path, exclude: &[&str]) -> Vec<String> {
        let patterns: Vec<PathBuf> = exclude.iter().map(PathBuf::from).collect();
        let mut out = Vec::new();
        collect_python_files(root, &mut out, &patterns, root);
        out.iter()
            .filter_map(|path| path.strip_prefix(root).ok())
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    // [CHKARCH-CONFIG-EXCLUDE] The editor scan must apply exactly the exclude
    // list it is handed — the same contract `basilisk check` honours
    // (crates/basilisk-cli/tests/e2e_exclude_config.rs): configuring `exclude`
    // REPLACES `DEFAULT_EXCLUDES` rather than extending them. A second,
    // hardcoded default set inside the scanner cannot be switched off by any
    // configuration, so the editor silently skips files the CLI reports on.
    #[test]
    fn configured_excludes_replace_defaults_instead_of_extending_them() {
        let root = unique_root("replaces");
        write(&root, "build/generated.py");
        write(&root, "vendor/thirdparty.py");
        write(&root, "src/app.py");

        let found = scan(&root, &["vendor"]);

        assert!(
            found.contains(&"build/generated.py".to_owned()),
            "`exclude = [\"vendor\"]` drops the defaults, so `build/` must be scanned \
             exactly as `basilisk check` scans it; got {found:?}"
        );
        assert!(
            !found.contains(&"vendor/thirdparty.py".to_owned()),
            "the configured `vendor` pattern must still be honoured; got {found:?}"
        );
        assert!(
            found.contains(&"src/app.py".to_owned()),
            "ordinary sources must always be scanned; got {found:?}"
        );
    }

    // The complement: with the caller passing the default list (what an
    // unconfigured workspace resolves to), the vendored trees stay skipped —
    // so the fix above removes a hardcoded duplicate, not the protection.
    #[test]
    fn default_excludes_still_skip_vendored_trees() {
        let root = unique_root("defaults");
        write(&root, "build/generated.py");
        write(&root, "node_modules/dep.py");
        write(&root, ".hidden/secret.py");
        write(&root, "src/app.py");

        let found = scan(&root, basilisk_config::DEFAULT_EXCLUDES);

        assert_eq!(
            found,
            vec!["src/app.py".to_owned()],
            "the default exclude list must skip every vendored tree, and hidden \
             directories are always skipped; got {found:?}"
        );
    }

    // [ANALYSIS-INDEX-STRUCT] A basename is not a module identity: packages
    // commonly contain same-named modules in different directories.
    #[test]
    fn dedup_keeps_same_named_modules_in_different_directories() {
        let first_source = PathBuf::from("/workspace/first/models.py");
        let first_stub = PathBuf::from("/workspace/first/models.pyi");
        let second_source = PathBuf::from("/workspace/second/models.py");

        let deduped = deduplicate_by_stem(vec![
            first_source.clone(),
            second_source.clone(),
            first_stub.clone(),
        ]);

        assert_eq!(deduped.len(), 2, "both module paths must be retained");
        assert!(
            deduped.contains(&first_stub),
            "the stub must win over its matching source"
        );
        assert!(
            !deduped.contains(&first_source),
            "only the source with a matching stub must be dropped"
        );
        assert!(
            deduped.contains(&second_source),
            "a same-named module in another directory must remain"
        );
    }
}
