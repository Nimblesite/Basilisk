//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Type health computation for the type health panel.

use std::path::Path;

use crate::workspace::WorkspaceIndex;

use super::helpers::{coverage_percent, module_name_from_path};

/// Build the full type health response.
///
/// Implements the server side of [EXTACT-HEALTH-TREE-STRUCTURE] and
/// [EXTACT-HEALTH-ITEM-PROPERTIES]: the workspace `HealthStats` plus the
/// per-module `ModuleHealth` list (path, coverage %, errors/warnings, adopted,
/// unannotated names), sorted worst-first (ascending coverage) per
/// [EXTACT-HEALTH-TOOLBAR]'s default. This is the shared surface for editors
/// without a unified panel (Zed `/health`, Neovim `:BasiliskHealth`).
pub(crate) fn build_type_health(
    idx: &WorkspaceIndex,
    project_root: Option<&Path>,
) -> serde_json::Value {
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

        let health = compute_file_health(
            resolved,
            &file_entry.diagnostics,
            path,
            project_root,
            adoption_store.as_ref(),
        );

        total_symbols += health.total_symbols;
        total_annotated += health.annotated_symbols;
        total_errors += health.errors;
        total_warnings += health.warnings;
        if health.adopted {
            total_adopted += 1;
        }

        module_health.push(serde_json::json!({
            "name": module_name,
            "path": path.display().to_string(),
            "coveragePercent": health.coverage_percent,
            "errors": health.errors,
            "warnings": health.warnings,
            "adopted": health.adopted,
            "unannotated": health.unannotated,
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

/// Per-file type-health figures.
///
/// Single source of truth shared by the `basilisk.typeHealth` command and the
/// health rollup folded into each `basilisk.workspaceModules` node — so the
/// coverage/diagnostic/adoption numbers are computed in exactly one place.
pub(crate) struct FileHealth {
    pub total_symbols: usize,
    pub annotated_symbols: usize,
    pub coverage_percent: u64,
    pub errors: usize,
    pub warnings: usize,
    pub adopted: bool,
    pub unannotated: Vec<String>,
}

/// Compute the type-health figures for a single resolved file.
pub(crate) fn compute_file_health(
    resolved: &basilisk_resolver::ResolvedModule,
    diagnostics: &[basilisk_checker::Diagnostic],
    path: &Path,
    project_root: Option<&Path>,
    adoption_store: Option<&basilisk_config::AdoptionStore>,
) -> FileHealth {
    let (total_symbols, annotated_symbols, unannotated) = count_annotations(resolved);

    let errors = diagnostics
        .iter()
        .filter(|d| d.severity == basilisk_checker::Severity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|d| d.severity == basilisk_checker::Severity::Warning)
        .count();

    let adopted = adoption_store.is_some_and(|store| {
        let relative = path
            .strip_prefix(project_root.unwrap_or(Path::new("")))
            .unwrap_or(path);
        store.demoted_count(relative) > 0
    });

    FileHealth {
        total_symbols,
        annotated_symbols,
        coverage_percent: coverage_percent(annotated_symbols, total_symbols),
        errors,
        warnings,
        adopted,
        unannotated,
    }
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
///
/// Implements the server side of the [EXTACT-MODULES-HEADER] /
/// [EXTACT-HEALTH-HEADER] empty-workspace guarantee: `totalFiles == 0`, so the
/// client branches to "No Python files found" rather than rendering the
/// vacuous `coveragePercent: 100` as a green "perfectly typed" state (#57).
pub(crate) fn empty_health_stats() -> serde_json::Value {
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
    use crate::workspace::WorkspaceIndex;

    fn make_index() -> WorkspaceIndex {
        WorkspaceIndex::new(
            vec![],
            AnalysisMode::WholeModule,
            basilisk_config::BasiliskConfig::default(),
        )
    }

    fn make_index_with_roots(roots: Vec<PathBuf>) -> WorkspaceIndex {
        // Enable the annotation house rules (off by default — the default config
        // is pure PEP conformance) so type-health surfaces their errors/warnings.
        // See [CHKARCH-CONFIGURATION-ONLY].
        WorkspaceIndex::new(
            roots,
            AnalysisMode::WholeModule,
            basilisk_config::BasiliskConfig {
                strict_annotations: true,
                ..Default::default()
            },
        )
    }

    fn make_uri(path: &str) -> tower_lsp::lsp_types::Url {
        tower_lsp::lsp_types::Url::parse(&format!("file://{path}")).unwrap()
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
