//! File-system scanning utilities for the workspace index.
//!
//! Provides recursive Python file collection, exclusion filtering, stem
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
        {
            out.push(path);
        }
    }
}

/// Check if a path matches any configured exclude patterns.
#[must_use]
pub fn is_excluded(path: &Path, exclude: &[PathBuf], workspace_root: &Path) -> bool {
    let relative = path.strip_prefix(workspace_root).unwrap_or(path);
    exclude.iter().any(|exc| relative.starts_with(exc))
}

/// Deduplicate a list of Python files by stem, preferring `.pyi` over `.py`.
#[must_use]
pub fn deduplicate_by_stem(files: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut by_stem: HashMap<String, PathBuf> = HashMap::new();
    for path in files {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let ext = path.extension().and_then(|s| s.to_str());
        let key = stem.to_owned();
        match by_stem.get(&key) {
            Some(existing) => {
                if existing.extension().and_then(|s| s.to_str()) == Some("py") && ext == Some("pyi")
                {
                    let _ = by_stem.insert(key, path);
                }
            }
            None => {
                let _ = by_stem.insert(key, path);
            }
        }
    }
    by_stem.into_values().collect()
}

/// Convert a filesystem path to an LSP `Url`.
#[must_use]
pub fn path_to_uri(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}
