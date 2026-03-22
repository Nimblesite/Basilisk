//! Activity panel command handlers for the Basilisk LSP server.
//!
//! Implements `basilisk.workspaceModules` and `basilisk.typeHealth` execute-command
//! handlers that power the Module Explorer and Type Health panels in editor extensions.

use std::path::Path;

use tower_lsp::jsonrpc::Result as LspResult;
use tracing::info;

use crate::workspace::WorkspaceIndex;

use super::LspServer;

/// Handle `basilisk.workspaceModules`.
///
/// Walks the workspace index and builds a hierarchical module tree from the
/// resolved symbol tables. Supports an optional `scope` parameter for prefix
/// filtering (used for lazy child loading).
pub(super) async fn execute_workspace_modules(
    server: &LspServer,
    args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let scope = args
        .first()
        .and_then(|v| v.get("scope"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let modules: Option<Vec<serde_json::Value>> = server
        .with_index(|idx| Some(build_module_tree(idx, scope)))
        .await;

    let modules = modules.unwrap_or_default();
    info!(module_count = modules.len(), scope, "workspaceModules");

    Ok(Some(serde_json::json!({ "modules": modules })))
}

/// Handle `basilisk.typeHealth`.
///
/// Computes type coverage statistics (annotated vs unannotated symbols),
/// error/warning counts, and adoption state for each file in the workspace.
pub(super) async fn execute_type_health(
    server: &LspServer,
    _args: &[serde_json::Value],
) -> LspResult<Option<serde_json::Value>> {
    let roots = server.workspace_roots.read().await;
    let project_root = roots.first().cloned();
    drop(roots);

    let result: Option<serde_json::Value> = server
        .with_index(|idx| Some(build_type_health(idx, project_root.as_deref())))
        .await;

    let result = result.unwrap_or_else(|| {
        serde_json::json!({
            "workspace": empty_health_stats(),
            "modules": []
        })
    });

    info!("typeHealth computed");
    Ok(Some(result))
}

// ── Module tree construction ──────────────────────────────────────────────

/// Build the module tree from the workspace index.
///
/// Each file becomes a module node containing its top-level symbols.
/// Packages are inferred from directory structure.
fn build_module_tree(idx: &WorkspaceIndex, scope: &str) -> Vec<serde_json::Value> {
    let mut modules = Vec::new();

    for entry in &idx.files {
        let path = entry.key();
        let file_entry = entry.value();

        // Compute module name from file path relative to workspace roots.
        let module_name = module_name_from_path(path, &idx.roots);
        if module_name.is_empty() {
            continue;
        }

        // Apply scope filter.
        if !scope.is_empty() && !module_name.starts_with(scope) {
            continue;
        }

        let Some(resolved) = &file_entry.resolved else {
            continue;
        };

        let symbols = build_symbol_list(resolved, &file_entry.text);

        let kind = if path
            .file_name()
            .is_some_and(|n| n == "__init__.py" || n == "__init__.pyi")
        {
            "package"
        } else {
            "module"
        };

        modules.push(serde_json::json!({
            "name": module_name,
            "path": path.display().to_string(),
            "kind": kind,
            "symbols": symbols,
        }));
    }

    modules.sort_by(|a, b| {
        let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        a_name.cmp(b_name)
    });

    modules
}

/// Build the list of top-level symbols from a resolved module.
fn build_symbol_list(
    resolved: &basilisk_resolver::ResolvedModule,
    text: &str,
) -> Vec<serde_json::Value> {
    let mut symbols = Vec::new();

    for func in &resolved.functions {
        // Skip methods (they belong to classes).
        if func.class_name.is_some() {
            continue;
        }
        let line = byte_offset_to_line(text, func.name_span.start);
        let annotated = !matches!(
            func.return_annotation,
            basilisk_resolver::ReturnAnnotationKind::Missing
        ) && func.parameters.iter().all(|p| p.has_annotation);

        symbols.push(serde_json::json!({
            "name": func.name,
            "kind": "function",
            "line": line,
            "annotated": annotated,
            "exported": false,
        }));
    }

    for class in &resolved.classes {
        let line = byte_offset_to_line(text, class.name_span.start);
        let children = build_class_children(resolved, class, text);

        symbols.push(serde_json::json!({
            "name": class.name,
            "kind": "class",
            "line": line,
            "annotated": true,
            "exported": false,
            "children": children,
        }));
    }

    for var in &resolved.module_vars {
        let line = byte_offset_to_line(text, var.name_span.start);
        let kind = if var.name.chars().all(|c| c.is_uppercase() || c == '_') {
            "constant"
        } else {
            "variable"
        };

        symbols.push(serde_json::json!({
            "name": var.name,
            "kind": kind,
            "line": line,
            "annotated": var.has_annotation,
            "exported": false,
        }));
    }

    symbols.sort_by_key(|s| {
        s.get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX)
    });

    symbols
}

/// Build children (methods) for a class.
fn build_class_children(
    resolved: &basilisk_resolver::ResolvedModule,
    class: &basilisk_resolver::ClassInfo,
    text: &str,
) -> Vec<serde_json::Value> {
    let mut children = Vec::new();

    // Methods.
    for func in &resolved.functions {
        if func.class_name.as_deref() != Some(&class.name) {
            continue;
        }
        let line = byte_offset_to_line(text, func.name_span.start);
        let annotated = !matches!(
            func.return_annotation,
            basilisk_resolver::ReturnAnnotationKind::Missing
        ) && func
            .parameters
            .iter()
            .filter(|p| p.name != "self" && p.name != "cls")
            .all(|p| p.has_annotation);

        children.push(serde_json::json!({
            "name": func.name,
            "kind": "function",
            "line": line,
            "annotated": annotated,
            "exported": false,
        }));
    }

    // Attributes.
    for attr in &class.attributes {
        let line = byte_offset_to_line(text, attr.name_span.start);
        children.push(serde_json::json!({
            "name": attr.name,
            "kind": "variable",
            "line": line,
            "annotated": attr.has_annotation,
            "exported": false,
        }));
    }

    children.sort_by_key(|s| {
        s.get("line")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX)
    });

    children
}

// ── Type health computation ───────────────────────────────────────────────

/// Build the full type health response.
fn build_type_health(idx: &WorkspaceIndex, project_root: Option<&Path>) -> serde_json::Value {
    let adoption_store =
        project_root.and_then(|root| basilisk_config::AdoptionStore::load(root).ok());

    let mut total_symbols: usize = 0;
    let mut total_annotated: usize = 0;
    let mut total_errors: usize = 0;
    let mut total_warnings: usize = 0;
    let mut total_adopted: usize = 0;
    let mut module_health: Vec<serde_json::Value> = Vec::new();

    for entry in &idx.files {
        let path = entry.key();
        let file_entry = entry.value();

        let module_name = module_name_from_path(path, &idx.roots);
        if module_name.is_empty() {
            continue;
        }

        let Some(resolved) = &file_entry.resolved else {
            continue;
        };

        let (symbols, annotated, unannotated_names) = count_annotations(resolved);
        let errors = file_entry
            .diagnostics
            .iter()
            .filter(|d| d.severity == basilisk_checker::Severity::Error)
            .count();
        let warnings = file_entry
            .diagnostics
            .iter()
            .filter(|d| d.severity == basilisk_checker::Severity::Warning)
            .count();

        let adopted = adoption_store.as_ref().is_some_and(|store| {
            let relative = path
                .strip_prefix(project_root.unwrap_or(Path::new("")))
                .unwrap_or(path);
            store.demoted_count(relative) > 0
        });

        let coverage = coverage_percent(annotated, symbols);

        total_symbols += symbols;
        total_annotated += annotated;
        total_errors += errors;
        total_warnings += warnings;
        if adopted {
            total_adopted += 1;
        }

        module_health.push(serde_json::json!({
            "name": module_name,
            "path": path.display().to_string(),
            "coveragePercent": coverage,
            "errors": errors,
            "warnings": warnings,
            "adopted": adopted,
            "unannotated": unannotated_names,
        }));
    }

    module_health.sort_by(|a, b| {
        let a_cov = a
            .get("coveragePercent")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(100);
        let b_cov = b
            .get("coveragePercent")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(100);
        a_cov.cmp(&b_cov)
    });

    let workspace_coverage = coverage_percent(total_annotated, total_symbols);

    serde_json::json!({
        "workspace": {
            "totalSymbols": total_symbols,
            "annotatedSymbols": total_annotated,
            "coveragePercent": workspace_coverage,
            "errors": total_errors,
            "warnings": total_warnings,
            "adoptedFiles": total_adopted,
            "totalFiles": idx.files.len(),
        },
        "modules": module_health,
    })
}

/// Count annotated vs unannotated symbols in a resolved module.
///
/// Returns `(total, annotated, unannotated_names)`.
fn count_annotations(resolved: &basilisk_resolver::ResolvedModule) -> (usize, usize, Vec<String>) {
    let mut total: usize = 0;
    let mut annotated: usize = 0;
    let mut unannotated: Vec<String> = Vec::new();

    // Top-level functions (skip methods).
    for func in &resolved.functions {
        if func.class_name.is_some() {
            continue;
        }
        total += 1;
        let is_annotated = !matches!(
            func.return_annotation,
            basilisk_resolver::ReturnAnnotationKind::Missing
        ) && func.parameters.iter().all(|p| p.has_annotation);
        if is_annotated {
            annotated += 1;
        } else {
            unannotated.push(func.name.clone());
        }
    }

    // Module variables.
    for var in &resolved.module_vars {
        total += 1;
        if var.has_annotation {
            annotated += 1;
        } else {
            unannotated.push(var.name.clone());
        }
    }

    // Class attributes and methods.
    for class in &resolved.classes {
        for attr in &class.attributes {
            total += 1;
            if attr.has_annotation {
                annotated += 1;
            } else {
                unannotated.push(format!("{}.{}", class.name, attr.name));
            }
        }
    }

    (total, annotated, unannotated)
}

/// Empty health stats for when the workspace index is unavailable.
fn empty_health_stats() -> serde_json::Value {
    serde_json::json!({
        "totalSymbols": 0,
        "annotatedSymbols": 0,
        "coveragePercent": 100,
        "errors": 0,
        "warnings": 0,
        "adoptedFiles": 0,
        "totalFiles": 0,
    })
}

// ── Utilities ─────────────────────────────────────────────────────────────

/// Derive a dotted Python module name from a file path relative to workspace roots.
fn module_name_from_path(path: &Path, roots: &[std::path::PathBuf]) -> String {
    let relative = roots
        .iter()
        .find_map(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);

    let mut parts: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Strip the file extension from the last component.
    if let Some(last) = parts.last_mut() {
        if let Some(stem) = last
            .strip_suffix(".py")
            .or_else(|| last.strip_suffix(".pyi"))
        {
            // __init__.py → use the package name (drop __init__).
            if stem == "__init__" {
                let _ = parts.pop();
            } else {
                *last = stem;
            }
        }
    }

    parts.join(".")
}

/// Convert a byte offset to a 0-based line number.
fn byte_offset_to_line(text: &str, offset: u32) -> usize {
    let offset = usize::try_from(offset).unwrap_or(0);
    text[..offset.min(text.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
}

/// Compute coverage percentage without raw `as` casts.
///
/// Symbol counts are far below 2^52 so f64 precision loss is negligible.
/// The result is always 0..=100 so truncation/sign-loss is impossible.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "symbol counts < 2^52 and result is 0..=100"
)]
fn coverage_percent(annotated: usize, total: usize) -> u64 {
    if total == 0 {
        return 100;
    }
    (annotated as f64 / total as f64 * 100.0).round() as u64
}

// ── Module change notification ────────────────────────────────────────────

/// Notification type for `basilisk/moduleChanged`.
pub(crate) struct ModuleChangedNotification;

impl tower_lsp::lsp_types::notification::Notification for ModuleChangedNotification {
    type Params = serde_json::Value;
    const METHOD: &'static str = basilisk_common::notifications::MODULE_CHANGED;
}

/// Send a debounced `basilisk/moduleChanged` notification for a file that was
/// just re-analysed. Waits 300 ms after the last save before sending, so rapid
/// saves don't flood the client.
pub(crate) async fn send_module_changed(server: &LspServer, uri: &tower_lsp::lsp_types::Url) {
    let uri = uri.clone();
    let index_lock = std::sync::Arc::clone(&server.index);
    let client = server.client.clone();

    let task = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(
            super::MODULE_CHANGED_DEBOUNCE_MS,
        ))
        .await;

        let module_data: Option<serde_json::Value> = {
            let guard = index_lock.read().await;
            guard.as_ref().and_then(|idx| {
                let path = uri.to_file_path().ok()?;
                let entry = idx.files.get(&path)?;
                let resolved = entry.resolved.as_ref()?;
                let module_name = module_name_from_path(&path, &idx.roots);
                if module_name.is_empty() {
                    return None;
                }

                let symbols = build_symbol_list(resolved, &entry.text);
                let kind = if path
                    .file_name()
                    .is_some_and(|n| n == "__init__.py" || n == "__init__.pyi")
                {
                    "package"
                } else {
                    "module"
                };

                Some(serde_json::json!({
                    "module": {
                        "name": module_name,
                        "path": path.display().to_string(),
                        "kind": kind,
                        "symbols": symbols,
                    }
                }))
            })
        };

        if let Some(data) = module_data {
            client
                .send_notification::<ModuleChangedNotification>(data)
                .await;
        }
    });

    // Abort any pending notification and replace with this new one.
    let abort_handle = task.abort_handle();
    let mut debounce = server.module_changed_debounce.lock().await;
    if let Some(old) = debounce.take() {
        old.abort();
    }
    *debounce = Some(abort_handle);
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::redundant_closure_for_method_calls,
    reason = "test-only code: unwrap, indexing, and closures acceptable in unit tests"
)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::config::AnalysisMode;

    fn make_index() -> WorkspaceIndex {
        WorkspaceIndex::new(vec![], AnalysisMode::WholeModule)
    }

    fn make_index_with_roots(roots: Vec<PathBuf>) -> WorkspaceIndex {
        WorkspaceIndex::new(roots, AnalysisMode::WholeModule)
    }

    fn make_uri(path: &str) -> tower_lsp::lsp_types::Url {
        tower_lsp::lsp_types::Url::parse(&format!("file://{path}")).unwrap()
    }

    // ── module_name_from_path ─────────────────────────────────────────────

    #[test]
    fn test_module_name_regular_py_file() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/pkg/sub/module.py");
        assert_eq!(module_name_from_path(&path, &roots), "pkg.sub.module");
    }

    #[test]
    fn test_module_name_init_py_becomes_package() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/pkg/sub/__init__.py");
        assert_eq!(module_name_from_path(&path, &roots), "pkg.sub");
    }

    #[test]
    fn test_module_name_pyi_stub_file() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/pkg/types.pyi");
        assert_eq!(module_name_from_path(&path, &roots), "pkg.types");
    }

    #[test]
    fn test_module_name_outside_roots_uses_full_components() {
        let roots = vec![PathBuf::from("/other")];
        let path = PathBuf::from("/workspace/pkg/module.py");
        // Path cannot be stripped from any root, so all path components
        // (including the root `/`) are joined with dots.
        let name = module_name_from_path(&path, &roots);
        assert!(
            name.contains("workspace"),
            "expected 'workspace' in module name, got: {name}"
        );
        assert!(
            name.contains("pkg"),
            "expected 'pkg' in module name, got: {name}"
        );
        assert!(
            name.ends_with("module"),
            "expected module name to end with 'module', got: {name}"
        );
    }

    #[test]
    fn test_module_name_init_pyi_becomes_package() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/pkg/__init__.pyi");
        assert_eq!(module_name_from_path(&path, &roots), "pkg");
    }

    #[test]
    fn test_module_name_top_level_file() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/main.py");
        assert_eq!(module_name_from_path(&path, &roots), "main");
    }

    // ── byte_offset_to_line ───────────────────────────────────────────────

    #[test]
    fn test_byte_offset_zero_is_line_zero() {
        assert_eq!(byte_offset_to_line("hello\nworld\n", 0), 0);
    }

    #[test]
    fn test_byte_offset_past_first_newline_is_line_one() {
        assert_eq!(byte_offset_to_line("hello\nworld\n", 6), 1);
    }

    #[test]
    fn test_byte_offset_at_end_of_file() {
        let text = "line0\nline1\nline2\n";
        let offset = u32::try_from(text.len()).unwrap();
        assert_eq!(byte_offset_to_line(text, offset), 3);
    }

    #[test]
    fn test_byte_offset_within_second_line() {
        assert_eq!(byte_offset_to_line("ab\ncd\nef\n", 4), 1);
    }

    // ── coverage_percent ──────────────────────────────────────────────────

    #[test]
    fn test_coverage_zero_total_is_100() {
        assert_eq!(coverage_percent(0, 0), 100);
    }

    #[test]
    fn test_coverage_half() {
        assert_eq!(coverage_percent(5, 10), 50);
    }

    #[test]
    fn test_coverage_full() {
        assert_eq!(coverage_percent(10, 10), 100);
    }

    #[test]
    fn test_coverage_thirty_percent() {
        assert_eq!(coverage_percent(3, 10), 30);
    }

    #[test]
    fn test_coverage_zero_annotated() {
        assert_eq!(coverage_percent(0, 10), 0);
    }

    // ── count_annotations ─────────────────────────────────────────────────

    #[test]
    fn test_count_annotations_mixed_symbols() {
        let idx = make_index();
        let uri = make_uri("/tmp/annotations.py");
        // `greet` has annotation on return + param → annotated.
        // `bare` has no annotation → unannotated.
        // `x` has annotation → annotated.
        // `y` has no annotation → unannotated.
        let src = concat!(
            "def greet(name: str) -> str:\n",
            "    return name\n",
            "\n",
            "def bare(a):\n",
            "    return a\n",
            "\n",
            "x: int = 1\n",
            "y = 2\n",
        );
        let _ = idx.set_open(&uri, src, 1);
        let path = uri.to_file_path().unwrap();
        let entry = idx.files.get(&path).unwrap();
        let resolved = entry.resolved.as_ref().unwrap();

        let (total, annotated, unannotated) = count_annotations(resolved);
        assert_eq!(total, 4, "expected 4 symbols (2 functions + 2 vars)");
        assert_eq!(annotated, 2, "expected 2 annotated (greet + x)");
        assert_eq!(unannotated.len(), 2, "expected 2 unannotated names");
        assert!(unannotated.contains(&"bare".to_string()));
        assert!(unannotated.contains(&"y".to_string()));
    }

    #[test]
    fn test_count_annotations_class_attributes() {
        let idx = make_index();
        let uri = make_uri("/tmp/cls_attrs.py");
        let src = concat!("class Foo:\n", "    x: int = 1\n", "    y = 2\n",);
        let _ = idx.set_open(&uri, src, 1);
        let path = uri.to_file_path().unwrap();
        let entry = idx.files.get(&path).unwrap();
        let resolved = entry.resolved.as_ref().unwrap();

        let (total, annotated, unannotated) = count_annotations(resolved);
        // Class attributes count towards the total.
        assert!(total >= 2, "expected at least 2 symbols from class attrs");
        assert!(annotated >= 1, "x: int should be annotated");
        assert!(
            unannotated.iter().any(|n| n.contains('y')),
            "y should be unannotated"
        );
    }

    // ── build_module_tree ─────────────────────────────────────────────────

    #[test]
    fn test_build_module_tree_two_files() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        let uri_a = make_uri("/workspace/alpha.py");
        let uri_b = make_uri("/workspace/beta.py");
        let _ = idx.set_open(&uri_a, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri_b, "y: str = 'hi'\n", 1);

        let tree = build_module_tree(&idx, "");
        assert_eq!(tree.len(), 2, "expected 2 modules in the tree");

        let names: Vec<&str> = tree
            .iter()
            .filter_map(|m| m.get("name").and_then(|v| v.as_str()))
            .collect();
        assert!(names.contains(&"alpha"), "expected alpha module");
        assert!(names.contains(&"beta"), "expected beta module");

        // Each module should have at least one symbol.
        for module in &tree {
            let symbols = module.get("symbols").and_then(|v| v.as_array()).unwrap();
            assert!(
                !symbols.is_empty(),
                "each module should have at least one symbol"
            );
        }
    }

    #[test]
    fn test_build_module_tree_kind_for_init_py() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        let uri = make_uri("/workspace/pkg/__init__.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let tree = build_module_tree(&idx, "");
        assert_eq!(tree.len(), 1);
        let kind = tree[0].get("kind").and_then(|v| v.as_str()).unwrap();
        assert_eq!(kind, "package");
    }

    #[test]
    fn test_build_module_tree_kind_for_regular_module() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        let uri = make_uri("/workspace/mod.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let tree = build_module_tree(&idx, "");
        assert_eq!(tree.len(), 1);
        let kind = tree[0].get("kind").and_then(|v| v.as_str()).unwrap();
        assert_eq!(kind, "module");
    }

    #[test]
    fn test_build_module_tree_scope_filter() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        let uri_a = make_uri("/workspace/pkg/a.py");
        let uri_b = make_uri("/workspace/other/b.py");
        let _ = idx.set_open(&uri_a, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri_b, "y: int = 2\n", 1);

        let tree = build_module_tree(&idx, "pkg");
        assert_eq!(tree.len(), 1, "scope filter should keep only pkg.a");
        let name = tree[0].get("name").and_then(|v| v.as_str()).unwrap();
        assert_eq!(name, "pkg.a");
    }

    // ── build_type_health ─────────────────────────────────────────────────

    #[test]
    fn test_build_type_health_coverage_and_counts() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root.clone()]);

        // Fully annotated file.
        let uri_full = make_uri("/workspace/full.py");
        let _ = idx.set_open(&uri_full, "x: int = 1\ny: str = 'hi'\n", 1);

        // Partially annotated file.
        let uri_partial = make_uri("/workspace/partial.py");
        let _ = idx.set_open(&uri_partial, "a: int = 1\nb = 2\n", 1);

        let health = build_type_health(&idx, Some(&root));

        // Workspace-level stats.
        let ws = health.get("workspace").unwrap();
        let total_symbols = ws.get("totalSymbols").and_then(|v| v.as_u64()).unwrap();
        let annotated_symbols = ws.get("annotatedSymbols").and_then(|v| v.as_u64()).unwrap();
        assert!(
            total_symbols >= 4,
            "expected at least 4 total symbols, got {total_symbols}"
        );
        assert!(
            annotated_symbols >= 3,
            "expected at least 3 annotated symbols, got {annotated_symbols}"
        );
        let total_files = ws.get("totalFiles").and_then(|v| v.as_u64()).unwrap();
        assert_eq!(total_files, 2, "expected 2 files in health");

        // Module-level entries.
        let modules = health.get("modules").and_then(|v| v.as_array()).unwrap();
        assert_eq!(modules.len(), 2, "expected 2 module health entries");

        // Verify coverage percentages are present and sane.
        for module in modules {
            let coverage = module
                .get("coveragePercent")
                .and_then(|v| v.as_u64())
                .unwrap();
            assert!(coverage <= 100, "coverage should be <= 100");
        }
    }

    #[test]
    fn test_build_type_health_sorted_by_coverage() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root.clone()]);

        // File with 0% coverage (no annotations).
        let uri_none = make_uri("/workspace/none.py");
        let _ = idx.set_open(&uri_none, "a = 1\nb = 2\n", 1);

        // File with 100% coverage.
        let uri_full = make_uri("/workspace/full.py");
        let _ = idx.set_open(&uri_full, "x: int = 1\n", 1);

        let health = build_type_health(&idx, Some(&root));
        let modules = health.get("modules").and_then(|v| v.as_array()).unwrap();

        // Modules should be sorted ascending by coveragePercent.
        let coverages: Vec<u64> = modules
            .iter()
            .filter_map(|m| m.get("coveragePercent").and_then(|v| v.as_u64()))
            .collect();
        for pair in coverages.windows(2) {
            assert!(
                pair[0] <= pair[1],
                "modules should be sorted ascending by coverage: {coverages:?}"
            );
        }
    }

    #[test]
    fn test_build_type_health_errors_and_warnings() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root.clone()]);

        // File with a type error (missing return annotation triggers diagnostics).
        let uri = make_uri("/workspace/errs.py");
        let _ = idx.set_open(&uri, "def foo(x: int):\n    return x\n", 1);

        let health = build_type_health(&idx, Some(&root));
        let ws = health.get("workspace").unwrap();

        // The checker should produce at least one error or warning for the
        // missing return annotation.
        let errors = ws.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
        let warnings = ws.get("warnings").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(
            errors + warnings > 0,
            "expected at least one diagnostic for missing return annotation"
        );
    }

    #[test]
    fn test_empty_health_stats() {
        let stats = empty_health_stats();
        assert_eq!(
            stats.get("totalSymbols").and_then(|v| v.as_u64()).unwrap(),
            0
        );
        assert_eq!(
            stats
                .get("coveragePercent")
                .and_then(|v| v.as_u64())
                .unwrap(),
            100
        );
    }
}
