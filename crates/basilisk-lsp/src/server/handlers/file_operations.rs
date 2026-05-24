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
/// Looks up files that import the old module path and rewrites their import
/// statements to reference the new module path.
fn collect_import_edits_for_rename(
    idx: &WorkspaceIndex,
    roots: &[PathBuf],
    old_uri_str: &str,
    new_uri_str: &str,
    all_changes: &mut HashMap<Url, Vec<TextEdit>>,
) {
    let Some(old_path) = uri_str_to_path(old_uri_str) else {
        return;
    };
    let Some(new_path) = uri_str_to_path(new_uri_str) else {
        return;
    };

    let Some(old_module) = file_path_to_module(&old_path, roots) else {
        return;
    };
    let Some(new_module) = file_path_to_module(&new_path, roots) else {
        return;
    };

    if old_module == new_module {
        return;
    }

    info!(
        old_module = %old_module,
        new_module = %new_module,
        "will_rename_files: rewriting imports"
    );

    // Find all files that import the old module via the import graph.
    let importers = {
        let Ok(graph) = idx.import_graph.lock() else {
            return;
        };
        graph.importers_of(&old_path)
    };

    for importer_path in &importers {
        collect_edits_for_importer(idx, importer_path, &old_module, &new_module, all_changes);
    }

    // Also check files not yet in the import graph by scanning all files
    // that have the old module name in their source text.
    for entry in &idx.files {
        let path = entry.key().clone();
        if importers.contains(&path) {
            continue;
        }
        collect_edits_for_importer(idx, &path, &old_module, &new_module, all_changes);
    }
}

/// Scan a single importer file for import statements referencing `old_module`
/// and produce text edits to rewrite them to `new_module`.
fn collect_edits_for_importer(
    idx: &WorkspaceIndex,
    importer_path: &Path,
    old_module: &str,
    new_module: &str,
    all_changes: &mut HashMap<Url, Vec<TextEdit>>,
) {
    let Some(entry) = idx.files.get(importer_path) else {
        return;
    };

    let edits = rewrite_imports_in_source(&entry.text, old_module, new_module);
    if edits.is_empty() {
        return;
    }

    let Some(uri) = crate::workspace_scan::path_to_uri(importer_path) else {
        return;
    };

    all_changes.entry(uri).or_default().extend(edits);
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

/// Rewrite import statements in `source` that reference `old_module`, returning
/// text edits that replace them with `new_module`.
///
/// Handles three forms:
/// - `import old.module.path`
/// - `from old.module import name`
/// - `from old.module.path import name`
fn rewrite_imports_in_source(source: &str, old_module: &str, new_module: &str) -> Vec<TextEdit> {
    let mut edits = Vec::new();

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        if let Some(edit) = try_rewrite_import_line(trimmed, line, line_idx, old_module, new_module)
        {
            edits.push(edit);
        }
    }

    edits
}

/// Try to rewrite a single line if it contains an import of `old_module`.
///
/// Returns `Some(TextEdit)` if the line was rewritten, `None` otherwise.
fn try_rewrite_import_line(
    trimmed: &str,
    original_line: &str,
    line_idx: usize,
    old_module: &str,
    new_module: &str,
) -> Option<TextEdit> {
    // Check for `import old_module` or `import old_module.sub`.
    if let Some(edit) =
        try_rewrite_import_stmt(trimmed, original_line, line_idx, old_module, new_module)
    {
        return Some(edit);
    }

    // Check for `from old_module import ...` or `from old_module.sub import ...`.
    try_rewrite_from_import(trimmed, original_line, line_idx, old_module, new_module)
}

/// Try to rewrite an `import X` statement.
fn try_rewrite_import_stmt(
    trimmed: &str,
    original_line: &str,
    line_idx: usize,
    old_module: &str,
    new_module: &str,
) -> Option<TextEdit> {
    let after_import = trimmed.strip_prefix("import ")?;

    // Handle `import old_module` or `import old_module as alias`.
    let module_part = after_import.split_whitespace().next()?;
    let module_part = module_part.trim_end_matches(',');

    if !is_module_match(module_part, old_module) {
        return None;
    }

    let replaced = replace_module_in_str(module_part, old_module, new_module);
    let new_line = original_line.replacen(module_part, &replaced, 1);

    Some(make_line_edit(line_idx, original_line, &new_line))
}

/// Try to rewrite a `from X import ...` statement.
fn try_rewrite_from_import(
    trimmed: &str,
    original_line: &str,
    line_idx: usize,
    old_module: &str,
    new_module: &str,
) -> Option<TextEdit> {
    let after_from = trimmed.strip_prefix("from ")?;

    // Extract the module path (everything before ` import`).
    let import_keyword_pos = after_from.find(" import")?;
    let module_part = after_from.get(..import_keyword_pos)?.trim();

    if !is_module_match(module_part, old_module) {
        return None;
    }

    let replaced = replace_module_in_str(module_part, old_module, new_module);
    let new_line = original_line.replacen(module_part, &replaced, 1);

    Some(make_line_edit(line_idx, original_line, &new_line))
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

/// Create a `TextEdit` that replaces an entire line.
fn make_line_edit(line_idx: usize, old_line: &str, new_line: &str) -> TextEdit {
    let line = u32::try_from(line_idx).unwrap_or(u32::MAX);
    let end_char = u32::try_from(old_line.len()).unwrap_or(u32::MAX);

    TextEdit {
        range: tower_lsp::lsp_types::Range {
            start: tower_lsp::lsp_types::Position { line, character: 0 },
            end: tower_lsp::lsp_types::Position {
                line,
                character: end_char,
            },
        },
        new_text: new_line.to_owned(),
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

    // ── rewrite_imports_in_source ────────────────────────────────────────

    #[test]
    fn rewrite_import_statement() {
        let source = "import foo.bar\n";
        let edits = rewrite_imports_in_source(source, "foo.bar", "baz.qux");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "import baz.qux");
    }

    #[test]
    fn rewrite_from_import() {
        let source = "from foo.bar import MyClass\n";
        let edits = rewrite_imports_in_source(source, "foo.bar", "baz.qux");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "from baz.qux import MyClass");
    }

    #[test]
    fn rewrite_from_submodule_import() {
        let source = "from foo.bar.sub import helper\n";
        let edits = rewrite_imports_in_source(source, "foo.bar", "baz.qux");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "from baz.qux.sub import helper");
    }

    #[test]
    fn rewrite_import_with_alias() {
        let source = "import foo.bar as fb\n";
        let edits = rewrite_imports_in_source(source, "foo.bar", "baz.qux");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "import baz.qux as fb");
    }

    #[test]
    fn no_rewrite_for_unrelated_import() {
        let source = "import something.else\nfrom another import thing\n";
        let edits = rewrite_imports_in_source(source, "foo.bar", "baz.qux");
        assert!(edits.is_empty());
    }

    #[test]
    fn rewrite_preserves_indentation() {
        let source = "    from foo.bar import MyClass\n";
        let edits = rewrite_imports_in_source(source, "foo.bar", "baz.qux");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "    from baz.qux import MyClass");
    }

    #[test]
    fn rewrite_multiple_imports() {
        let source = "import foo.bar\nfrom foo.bar import X\nimport other\n";
        let edits = rewrite_imports_in_source(source, "foo.bar", "baz.qux");
        assert_eq!(edits.len(), 2);
        assert_eq!(edits[0].new_text, "import baz.qux");
        assert_eq!(edits[1].new_text, "from baz.qux import X");
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

    // ── make_line_edit ───────────────────────────────────────────────────

    #[test]
    fn line_edit_range_is_correct() {
        let edit = make_line_edit(5, "import foo.bar", "import baz.qux");
        assert_eq!(edit.range.start.line, 5);
        assert_eq!(edit.range.start.character, 0);
        assert_eq!(edit.range.end.line, 5);
        assert_eq!(edit.range.end.character, 14);
        assert_eq!(edit.new_text, "import baz.qux");
    }
}
