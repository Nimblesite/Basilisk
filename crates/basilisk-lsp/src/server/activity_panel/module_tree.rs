//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Module tree construction for the workspace modules panel.

use std::path::Path;

use crate::workspace::WorkspaceIndex;

use super::helpers::{byte_offset_to_line, coverage_percent, module_name_from_path};
use super::type_health::compute_file_health;

/// Result of building the workspace module tree: the per-module nodes (each with
/// its folded health rollup) plus the workspace-wide health summary.
pub(crate) struct WorkspaceModulesResult {
    pub modules: Vec<serde_json::Value>,
    pub workspace: serde_json::Value,
}

// Implements [LSPARCH-DATAMODEL]
/// Build the module tree from the workspace index.
///
/// Implements the server side of [EXTACT-MODULES-MODULE-ROW] (each node carries
/// the folded coverage %, error/warning counts, and adoption state rendered on
/// the module row) and [EXTACT-MODULES-HEADER] (the `workspace` `HealthStats`
/// summary that drives the view's message + badge).
///
/// Each file becomes a module node containing its top-level symbols and a folded
/// health rollup (coverage %, error/warning counts, adoption state). The
/// workspace-wide rollup is accumulated in the same single pass, so the merged
/// Modules panel needs no separate `basilisk.typeHealth` round-trip.
pub(crate) fn build_module_tree(
    idx: &WorkspaceIndex,
    scope: &str,
    project_root: Option<&Path>,
    type_checking_enabled: bool,
) -> WorkspaceModulesResult {
    let adoption_store =
        project_root.and_then(|root| basilisk_config::AdoptionStore::load(root).ok());

    let mut modules = Vec::new();
    let mut total_symbols: usize = 0;
    let mut total_annotated: usize = 0;
    let mut total_errors: usize = 0;
    let mut total_warnings: usize = 0;
    let mut total_adopted: usize = 0;

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
        let health = compute_file_health(
            resolved,
            &file_entry.diagnostics,
            path,
            project_root,
            adoption_store.as_ref(),
        );

        // The module tree is a PULL surface that reads stored diagnostics
        // directly, bypassing the publish gate. When type checking is disabled the
        // error/warning counts must read empty too, mirroring the cleared editor
        // diagnostics ([ANALYSIS-ENABLED], GitHub #119). Coverage and adoption are
        // annotation metrics, not type-check diagnostics, so they remain.
        let errors = if type_checking_enabled {
            health.errors
        } else {
            0
        };
        let warnings = if type_checking_enabled {
            health.warnings
        } else {
            0
        };

        total_symbols += health.total_symbols;
        total_annotated += health.annotated_symbols;
        total_errors += errors;
        total_warnings += warnings;
        if health.adopted {
            total_adopted += 1;
        }

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
            "coveragePercent": health.coverage_percent,
            "errors": errors,
            "warnings": warnings,
            "adopted": health.adopted,
        }));
    }

    modules.sort_by(|a, b| {
        let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        a_name.cmp(b_name)
    });

    let workspace = serde_json::json!({
        "totalSymbols": total_symbols,
        "annotatedSymbols": total_annotated,
        "coveragePercent": coverage_percent(total_annotated, total_symbols),
        "errors": total_errors,
        "warnings": total_warnings,
        "adoptedFiles": total_adopted,
        "totalFiles": idx.files.len(),
    });

    WorkspaceModulesResult { modules, workspace }
}

/// Build the list of top-level symbols from a resolved module.
///
/// Implements the server side of [EXTACT-MODULES-ITEM-PROPERTIES]: each symbol
/// carries its name, kind, source line, and `annotated` flag so the client can
/// render the per-symbol drill-down rows and the "untyped" decoration.
pub(crate) fn build_symbol_list(
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

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test-only code: unwrap and indexing acceptable in unit tests"
)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::config::AnalysisMode;
    use crate::workspace::WorkspaceIndex;

    fn make_index_with_roots(roots: Vec<PathBuf>) -> WorkspaceIndex {
        WorkspaceIndex::new(
            roots,
            AnalysisMode::WholeModule,
            basilisk_config::BasiliskConfig::default(),
        )
    }

    fn make_uri(path: &str) -> tower_lsp::lsp_types::Url {
        tower_lsp::lsp_types::Url::parse(&format!("file://{path}")).unwrap()
    }

    #[test]
    fn test_build_module_tree_two_files() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root.clone()]);
        let uri_a = make_uri("/workspace/alpha.py");
        let uri_b = make_uri("/workspace/beta.py");
        let _ = idx.set_open(&uri_a, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri_b, "y: str = 'hi'\n", 1);

        let tree = build_module_tree(&idx, "", Some(&root), true);
        assert_eq!(tree.modules.len(), 2, "expected 2 modules in the tree");

        let names: Vec<&str> = tree
            .modules
            .iter()
            .filter_map(|m| m.get("name").and_then(|v| v.as_str()))
            .collect();
        assert!(names.contains(&"alpha"), "expected alpha module");
        assert!(names.contains(&"beta"), "expected beta module");

        // Each module should have at least one symbol.
        for module in &tree.modules {
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
        let idx = make_index_with_roots(vec![root.clone()]);
        let uri = make_uri("/workspace/pkg/__init__.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let tree = build_module_tree(&idx, "", Some(&root), true);
        assert_eq!(tree.modules.len(), 1);
        let kind = tree.modules[0]
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(kind, "package");
    }

    #[test]
    fn test_build_module_tree_kind_for_regular_module() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root.clone()]);
        let uri = make_uri("/workspace/mod.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let tree = build_module_tree(&idx, "", Some(&root), true);
        assert_eq!(tree.modules.len(), 1);
        let kind = tree.modules[0]
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(kind, "module");
    }

    #[test]
    fn test_build_module_tree_scope_filter() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root.clone()]);
        let uri_a = make_uri("/workspace/pkg/a.py");
        let uri_b = make_uri("/workspace/other/b.py");
        let _ = idx.set_open(&uri_a, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri_b, "y: int = 2\n", 1);

        let tree = build_module_tree(&idx, "pkg", Some(&root), true);
        assert_eq!(tree.modules.len(), 1, "scope filter should keep only pkg.a");
        let name = tree.modules[0]
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(name, "pkg.a");
    }

    #[test]
    fn test_build_module_tree_folds_health_rollup() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root.clone()]);
        // Fully annotated file (100%) and a half-annotated file.
        let uri_full = make_uri("/workspace/full.py");
        let uri_partial = make_uri("/workspace/partial.py");
        let _ = idx.set_open(&uri_full, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri_partial, "a: int = 1\nb = 2\n", 1);

        let tree = build_module_tree(&idx, "", Some(&root), true);

        // Every module node carries its folded health fields.
        for module in &tree.modules {
            for field in ["coveragePercent", "errors", "warnings", "adopted"] {
                assert!(
                    module.get(field).is_some(),
                    "module node missing folded health field '{field}'"
                );
            }
            let coverage = module
                .get("coveragePercent")
                .and_then(serde_json::Value::as_u64)
                .unwrap();
            assert!(coverage <= 100, "coverage should be <= 100");
        }

        // The workspace rollup is present and consistent.
        let ws = &tree.workspace;
        assert_eq!(
            ws.get("totalFiles").and_then(serde_json::Value::as_u64),
            Some(2),
            "workspace rollup should count both files"
        );
        let ws_coverage = ws
            .get("coveragePercent")
            .and_then(serde_json::Value::as_u64)
            .unwrap();
        assert!(ws_coverage <= 100, "workspace coverage should be <= 100");
    }
}
