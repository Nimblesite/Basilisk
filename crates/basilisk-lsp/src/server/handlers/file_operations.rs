//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! File operation handlers: `workspace/willRenameFiles`.
//!
//! When a Python file is renamed, rewrites all import statements in files
//! that reference the old module path so they point to the new module path.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{RenameFilesParams, TextEdit, Url, WorkspaceEdit};
use tracing::info;

use crate::server::LspServer;
use crate::workspace::WorkspaceIndex;

// Implements [REFACTOR-RENAMEMOD]
/// Handle `workspace/willRenameFiles`.
///
/// For each renamed `.py` file, computes the old and new Python module paths,
/// finds all files that import the old module, and returns a `WorkspaceEdit`
/// that rewrites those import statements.
pub(in crate::server) async fn will_rename_files(
    server: &LspServer,
    params: RenameFilesParams,
) -> LspResult<Option<WorkspaceEdit>> {
    let roots = server.workspace_roots.read().await;
    let roots_snapshot: Vec<PathBuf> = roots.clone();
    drop(roots);

    let edits = server
        .with_index(|idx| {
            let mut all_changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

            for file_rename in &params.files {
                collect_import_edits_for_rename(
                    idx,
                    &roots_snapshot,
                    &file_rename.old_uri,
                    &file_rename.new_uri,
                    &mut all_changes,
                );
            }

            if all_changes.is_empty() {
                None
            } else {
                Some(all_changes)
            }
        })
        .await;

    let Some(changes) = edits else {
        return Ok(None);
    };

    info!(
        file_count = changes.len(),
        "will_rename_files: returning import edits"
    );

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        ..Default::default()
    }))
}

/// Collect all import-rewriting edits for a single file rename.
///
/// INERT. Rewriting an importer's `import` statements meant walking its source
/// line by line and keeping any line whose characters began with the literal
/// prefix `import ` or `from `, then splicing new text into that line. That is
/// scanning Python source for language vocabulary, which the project's first
/// standing rule forbids: the import statements are typed AST nodes with exact
/// spans, and the scanner instead saw imports inside strings, missed
/// parenthesised and line-continued forms, and rewrote by character offset.
///
/// The scanner is deleted; renaming a file no longer updates its importers
/// until the rewrite is rebuilt on the AST.
fn collect_import_edits_for_rename(
    _idx: &WorkspaceIndex,
    _roots: &[PathBuf],
    _old_uri_str: &str,
    _new_uri_str: &str,
    _all_changes: &mut HashMap<Url, Vec<TextEdit>>,
) {
}

/// Parse a `file://` URI string into a filesystem `PathBuf`.
fn uri_str_to_path(uri_str: &str) -> Option<PathBuf> {
    let url = Url::parse(uri_str).ok()?;
    url.to_file_path().ok()
}

/// Convert a Python file path to a dotted module path relative to a workspace root.
///
/// Examples:
/// - `/workspace/foo/bar.py`    -> `foo.bar`
/// - `/workspace/foo/__init__.py` -> `foo`
/// - `/workspace/main.py`       -> `main`
fn file_path_to_module(file_path: &Path, roots: &[PathBuf]) -> Option<String> {
    let relative = find_relative_to_root(file_path, roots)?;
    relative_path_to_module(&relative)
}

/// Find the relative path of `file_path` under the first matching workspace root.
fn find_relative_to_root(file_path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    for root in roots {
        if let Ok(rel) = file_path.strip_prefix(root) {
            return Some(rel.to_path_buf());
        }
    }
    None
}

/// Convert a relative file path to a dotted Python module path.
///
/// Strips the `.py`/`.pyi` extension, replaces path separators with dots,
/// and handles `__init__.py` (the module is the parent package).
fn relative_path_to_module(relative: &Path) -> Option<String> {
    let file_name = relative.file_name()?.to_str()?;

    let is_init = file_name == "__init__.py" || file_name == "__init__.pyi";

    if is_init {
        // Module path is the parent directory path.
        let parent = relative.parent()?;
        if parent.as_os_str().is_empty() {
            return None;
        }
        return Some(path_components_to_module(parent));
    }

    // Strip .py or .pyi extension.
    let stem = relative.with_extension("").to_str().map(str::to_owned)?;

    // Also strip .py from .pyi (which leaves .py after first with_extension(""))
    let stem = if std::path::Path::new(&stem)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
    {
        // Strip the ".py" suffix (3 chars: dot + extension)
        stem[..stem.len() - 3].to_owned()
    } else {
        stem
    };

    Some(stem.replace(std::path::MAIN_SEPARATOR, "."))
}

/// Convert path components to a dotted module string.
fn path_components_to_module(path: &Path) -> String {
    path.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join(".")
}

/// Check if `module_str` matches `old_module` exactly or starts with `old_module.`.
fn is_module_match(module_str: &str, old_module: &str) -> bool {
    module_str == old_module || module_str.starts_with(&format!("{old_module}."))
}

/// Replace the `old_module` prefix in `module_str` with `new_module`.
fn replace_module_in_str(module_str: &str, old_module: &str, new_module: &str) -> String {
    if module_str == old_module {
        new_module.to_owned()
    } else {
        // Replace prefix: `old_module.sub` -> `new_module.sub`.
        format!("{new_module}{}", &module_str[old_module.len()..])
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test-only code: unwrap and indexing acceptable in unit tests"
)]
mod tests {
    use super::*;

    // ── relative_path_to_module ───────────────────────────────────────────

    #[test]
    fn module_from_simple_file() {
        let path = Path::new("foo/bar.py");
        assert_eq!(relative_path_to_module(path).unwrap(), "foo.bar");
    }

    #[test]
    fn module_from_init_file() {
        let path = Path::new("foo/__init__.py");
        assert_eq!(relative_path_to_module(path).unwrap(), "foo");
    }

    #[test]
    fn module_from_top_level_file() {
        let path = Path::new("main.py");
        assert_eq!(relative_path_to_module(path).unwrap(), "main");
    }

    #[test]
    fn module_from_nested_init() {
        let path = Path::new("foo/bar/__init__.py");
        assert_eq!(relative_path_to_module(path).unwrap(), "foo.bar");
    }

    #[test]
    fn module_from_deeply_nested() {
        let path = Path::new("a/b/c/d.py");
        assert_eq!(relative_path_to_module(path).unwrap(), "a.b.c.d");
    }

    // ── is_module_match ──────────────────────────────────────────────────

    #[test]
    fn exact_match() {
        assert!(is_module_match("foo.bar", "foo.bar"));
    }

    #[test]
    fn prefix_match() {
        assert!(is_module_match("foo.bar.baz", "foo.bar"));
    }

    #[test]
    fn no_match_different_module() {
        assert!(!is_module_match("foo.baz", "foo.bar"));
    }

    #[test]
    fn no_match_partial_segment() {
        // "foo.barx" should NOT match "foo.bar" (partial segment).
        assert!(!is_module_match("foo.barx", "foo.bar"));
    }

    // ── replace_module_in_str ────────────────────────────────────────────

    #[test]
    fn replace_exact() {
        assert_eq!(
            replace_module_in_str("foo.bar", "foo.bar", "baz.qux"),
            "baz.qux"
        );
    }

    #[test]
    fn replace_prefix() {
        assert_eq!(
            replace_module_in_str("foo.bar.sub", "foo.bar", "baz.qux"),
            "baz.qux.sub"
        );
    }

    // ── file_path_to_module ──────────────────────────────────────────────

    #[test]
    fn file_path_to_module_simple() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/foo/bar.py");
        assert_eq!(file_path_to_module(&path, &roots).unwrap(), "foo.bar");
    }

    #[test]
    fn file_path_to_module_init() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/foo/__init__.py");
        assert_eq!(file_path_to_module(&path, &roots).unwrap(), "foo");
    }

    #[test]
    fn file_path_to_module_no_matching_root() {
        let roots = vec![PathBuf::from("/other")];
        let path = PathBuf::from("/workspace/foo/bar.py");
        assert!(file_path_to_module(&path, &roots).is_none());
    }

}
