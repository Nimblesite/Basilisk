//! Implements [ANALYSIS-INDEX-STRUCT]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INDEX-STRUCT
//! File-system scanning utilities for the workspace index.
//!
//! Provides recursive Python file collection, exclusion filtering, path-aware
//! deduplication (`.pyi` beats `.py`), and `Path → Url` conversion.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;

/// Recursively collect all `.py` / `.pyi` files under `dir`, skipping hidden
/// dirs, common non-source directories, and user-configured exclude paths.
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
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.')
                || basilisk_config::DEFAULT_EXCLUDES
                    .iter()
                    .any(|exc| name_str == *exc)
            {
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
    use super::deduplicate_by_stem;
    use std::path::PathBuf;

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
