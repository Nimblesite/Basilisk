//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Module tree construction for the workspace modules panel.

use std::path::Path;

use crate::workspace::WorkspaceIndex;

use super::helpers::{
    byte_offset_to_character, byte_offset_to_line, coverage_percent, module_name_from_path,
};
use super::type_health::compute_file_health;

/// Result of building the workspace module tree: the per-module nodes (each with
/// its folded health rollup) plus the workspace-wide health summary.
pub(crate) struct WorkspaceModulesResult {
    pub modules: Vec<serde_json::Value>,
    pub workspace: serde_json::Value,
}

/// Workspace-wide grading accumulator for the single-pass rollup.
#[derive(Default)]
struct HealthTotals {
    symbols: usize,
    annotated: usize,
    errors: usize,
    warnings: usize,
}

impl HealthTotals {
    fn accumulate(&mut self, health: &super::type_health::FileHealth) {
        self.symbols += health.total_symbols;
        self.annotated += health.annotated_symbols;
        self.errors += health.errors;
        self.warnings += health.warnings;
    }
}

// Implements [LSPARCH-DATAMODEL]
/// Build the module tree from the workspace index.
///
/// Implements the server side of [EXTACT-MODULES-MODULE-ROW] (each node carries
/// the folded coverage % and error/warning counts rendered on the module row)
/// and [EXTACT-MODULES-HEADER] (the `workspace` `HealthStats` summary that
/// drives the view's message + badge).
///
/// Each file becomes a module node containing its top-level symbols and a folded
/// health rollup (coverage %, error/warning counts). The
/// workspace-wide rollup is accumulated in the same single pass, so the merged
/// Modules panel needs no separate `basilisk.typeHealth` round-trip.
///
/// With type checking disabled ([ANALYSIS-ENABLED], GitHub #119) the payload
/// carries NO grading data at all — no coverage % and no error/warning tallies
/// — only the navigation tree plus `typeCheckingEnabled: false`.
/// The grading fields are OMITTED (not zeroed) so no client can render a
/// "NN% typed" header or coverage-tinted rows while the toggle is off.
pub(crate) fn build_module_tree(
    idx: &WorkspaceIndex,
    scope: &str,
    type_checking_enabled: bool,
    scan_complete: bool,
) -> WorkspaceModulesResult {
    let mut modules = Vec::new();
    let mut totals = HealthTotals::default();

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

        let mut node = serde_json::json!({
            "name": module_name,
            "path": path.display().to_string(),
            "kind": module_kind(path),
            "symbols": build_symbol_list(resolved, &file_entry.text),
            // The navigable drill-down behind the row's error/warning tally
            // ([EXTACT-MODULES-DIAGNOSTICS], GitHub #235). EMPTY while type
            // checking is disabled ([ANALYSIS-ENABLED]): possibly-stale
            // diagnostics must not leak through the drill-down either.
            "diagnostics": if type_checking_enabled {
                diagnostic_nodes(&file_entry.diagnostics, &file_entry.text)
            } else {
                Vec::new()
            },
        });

        if type_checking_enabled {
            let health = compute_file_health(resolved, &file_entry.diagnostics);
            totals.accumulate(&health);
            attach_grading(&mut node, &health);
        }

        modules.push(node);
    }

    modules.sort_by(|a, b| {
        let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        a_name.cmp(b_name)
    });

    let workspace = workspace_rollup(
        &totals,
        idx.files.len(),
        type_checking_enabled,
        scan_complete,
    );

    WorkspaceModulesResult { modules, workspace }
}

/// Serialize a file's diagnostics as the spec's `DiagnosticNode` rows —
/// errors before warnings, then ascending line — so every count advertised on
/// the module row is navigable ([EXTACT-MODULES-DIAGNOSTICS], GitHub #235).
///
/// Only exact `Error`/`Warning` severities are serialized — the same filter
/// [`compute_file_health`] counts — so the spec invariant
/// `errors == diagnostics.filter(d => d.severity == "error").length` holds
/// (`Info` and the opt-in `SafetyViolation` are excluded from both).
fn diagnostic_nodes(
    diagnostics: &[basilisk_checker::Diagnostic],
    text: &str,
) -> Vec<serde_json::Value> {
    let mut rows: Vec<(bool, usize, serde_json::Value)> = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let severity = match diagnostic.severity {
                basilisk_checker::Severity::Error => "error",
                basilisk_checker::Severity::Warning => "warning",
                basilisk_checker::Severity::Info | basilisk_checker::Severity::SafetyViolation => {
                    return None;
                }
            };
            let line = byte_offset_to_line(text, diagnostic.span.start);
            let node = serde_json::json!({
                "severity": severity,
                "code": diagnostic.code.code,
                "message": diagnostic.message,
                "line": line,
                "character": byte_offset_to_character(text, diagnostic.span.start),
            });
            Some((severity != "error", line, node))
        })
        .collect();
    rows.sort_by_key(|&(is_warning, line, _)| (is_warning, line));
    rows.into_iter().map(|(_, _, node)| node).collect()
}

/// Node kind: `__init__.py(i)` files are packages, everything else a module.
fn module_kind(path: &Path) -> &'static str {
    if path
        .file_name()
        .is_some_and(|n| n == "__init__.py" || n == "__init__.pyi")
    {
        "package"
    } else {
        "module"
    }
}

/// Fold the per-file grading rollup into a module node — enabled path only
/// ([ANALYSIS-ENABLED]): while disabled these fields are absent by construction.
/// The raw symbol counts ride along so clients can roll folder/package coverage
/// up symbol-weighted — matching the workspace header — instead of averaging
/// pre-divided percentages ([EXTACT-MODULES-TREE-STRUCTURE]).
fn attach_grading(node: &mut serde_json::Value, health: &super::type_health::FileHealth) {
    if let Some(obj) = node.as_object_mut() {
        let _ = obj.insert("coveragePercent".into(), health.coverage_percent.into());
        let _ = obj.insert("totalSymbols".into(), health.total_symbols.into());
        let _ = obj.insert("annotatedSymbols".into(), health.annotated_symbols.into());
        let _ = obj.insert("errors".into(), health.errors.into());
        let _ = obj.insert("warnings".into(), health.warnings.into());
    }
}

/// The `workspace` `HealthStats` summary. Every payload declares the toggle
/// state; the grading rollup is only present while type checking is enabled
/// ([ANALYSIS-ENABLED], #119). Every payload also declares `scanComplete`, so
/// a client can tell a genuinely empty workspace apart from one whose initial
/// scan hasn't finished ([EXTACT-MODULES-HEADER-LOADING], GitHub #144).
fn workspace_rollup(
    totals: &HealthTotals,
    total_files: usize,
    type_checking_enabled: bool,
    scan_complete: bool,
) -> serde_json::Value {
    if !type_checking_enabled {
        return serde_json::json!({
            "typeCheckingEnabled": false,
            "totalFiles": total_files,
            "scanComplete": scan_complete,
        });
    }
    serde_json::json!({
        "typeCheckingEnabled": true,
        "totalSymbols": totals.symbols,
        "annotatedSymbols": totals.annotated,
        "coveragePercent": coverage_percent(totals.annotated, totals.symbols),
        "errors": totals.errors,
        "warnings": totals.warnings,
        "totalFiles": total_files,
        "scanComplete": scan_complete,
    })
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
        let idx = make_index_with_roots(vec![root]);
        let uri_a = make_uri("/workspace/alpha.py");
        let uri_b = make_uri("/workspace/beta.py");
        let _ = idx.set_open(&uri_a, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri_b, "y: str = 'hi'\n", 1);

        let tree = build_module_tree(&idx, "", true, true);
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
        let idx = make_index_with_roots(vec![root]);
        let uri = make_uri("/workspace/pkg/__init__.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let tree = build_module_tree(&idx, "", true, true);
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
        let idx = make_index_with_roots(vec![root]);
        let uri = make_uri("/workspace/mod.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let tree = build_module_tree(&idx, "", true, true);
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
        let idx = make_index_with_roots(vec![root]);
        let uri_a = make_uri("/workspace/pkg/a.py");
        let uri_b = make_uri("/workspace/other/b.py");
        let _ = idx.set_open(&uri_a, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri_b, "y: int = 2\n", 1);

        let tree = build_module_tree(&idx, "pkg", true, true);
        assert_eq!(tree.modules.len(), 1, "scope filter should keep only pkg.a");
        let name = tree.modules[0]
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(name, "pkg.a");
    }

    // [ANALYSIS-ENABLED] (GitHub #119, showstopper reopen): with type checking
    // disabled the server must serve NO grading data at all — no coverage %, no
    // error/warning tallies, no adoption state — so no client can render
    // "NN% typed" or red rows while the user has switched checking off.
    #[test]
    fn test_disabled_toggle_serves_no_grading_data() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        // Unannotated symbols → low coverage that would tint rows red if served.
        let uri = make_uri("/workspace/untyped.py");
        let _ = idx.set_open(&uri, "def bare(a):\n    return a\n\nb = 2\n", 1);

        let tree = build_module_tree(&idx, "", false, true);

        assert_eq!(tree.modules.len(), 1, "module list stays for navigation");
        let module = &tree.modules[0];
        for field in ["coveragePercent", "errors", "warnings"] {
            assert!(
                module.get(field).is_none(),
                "disabled toggle must omit grading field '{field}' from module nodes, got {module}"
            );
        }
        // Navigation payload survives — the panel is still a module browser.
        assert!(
            module.get("symbols").is_some(),
            "symbols stay for navigation"
        );

        let ws = &tree.workspace;
        assert_eq!(
            ws.get("typeCheckingEnabled")
                .and_then(serde_json::Value::as_bool),
            Some(false),
            "workspace rollup must carry typeCheckingEnabled=false, got {ws}"
        );
        for field in ["coveragePercent", "errors", "warnings", "totalSymbols"] {
            assert!(
                ws.get(field).is_none(),
                "disabled toggle must omit workspace grading field '{field}', got {ws}"
            );
        }
    }

    // [ANALYSIS-ENABLED]: while enabled the payload declares it, so clients can
    // branch on the flag instead of guessing from missing fields.
    #[test]
    fn test_enabled_toggle_stamps_flag_and_serves_grading() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        let uri = make_uri("/workspace/mod.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let tree = build_module_tree(&idx, "", true, true);
        assert_eq!(
            tree.workspace
                .get("typeCheckingEnabled")
                .and_then(serde_json::Value::as_bool),
            Some(true),
            "enabled payload must stamp typeCheckingEnabled=true"
        );
        assert!(
            tree.modules[0].get("coveragePercent").is_some(),
            "enabled payload keeps the grading rollup"
        );
    }

    // Server side of [EXTACT-MODULES-TREE-STRUCTURE] coverage rollup: each
    // module node must carry its symbol counts (`totalSymbols` /
    // `annotatedSymbols`) so the client can roll folder/package coverage up
    // symbol-weighted — matching the workspace header — instead of having only
    // the pre-divided per-file percentage with no weights.
    #[test]
    fn test_module_nodes_carry_symbol_counts_for_folder_rollup() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        // Half-annotated file: 1 of 2 symbols annotated.
        let uri = make_uri("/workspace/pkg/partial.py");
        let _ = idx.set_open(&uri, "a: int = 1\nb = 2\n", 1);

        let tree = build_module_tree(&idx, "", true, true);
        assert_eq!(tree.modules.len(), 1);
        let module = &tree.modules[0];
        assert_eq!(
            module
                .get("totalSymbols")
                .and_then(serde_json::Value::as_u64),
            Some(2),
            "module node must carry totalSymbols as the client's rollup weight, got {module}"
        );
        assert_eq!(
            module
                .get("annotatedSymbols")
                .and_then(serde_json::Value::as_u64),
            Some(1),
            "module node must carry annotatedSymbols for the client's rollup, got {module}"
        );

        // Disabled toggle omits the counts like every other grading field
        // ([ANALYSIS-ENABLED], #119).
        let disabled = build_module_tree(&idx, "", false, true);
        for field in ["totalSymbols", "annotatedSymbols"] {
            assert!(
                disabled.modules[0].get(field).is_none(),
                "disabled toggle must omit '{field}' from module nodes"
            );
        }
    }

    // Tests [EXTACT-MODULES-DIAGNOSTICS] (GitHub #235): every module node must
    // carry its diagnostics as a navigable list so the `errors`/`warnings`
    // tallies rendered on the row are reachable, not dead. The wire shape is the
    // spec's DiagnosticNode: severity/code/message/line/character, with the
    // count invariant `errors == diagnostics.filter(severity == "error").len()`.
    #[test]
    fn test_module_nodes_carry_navigable_diagnostics() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        let uri = make_uri("/workspace/broken.py");
        let _ = idx.set_open(&uri, "x: int = \"not an int\"\n", 1);

        let tree = build_module_tree(&idx, "", true, true);
        assert_eq!(tree.modules.len(), 1);
        let module = &tree.modules[0];

        // Precondition: the fixture really produces a type error, so the test
        // exercises a non-empty drill-down (not a vacuously-empty list).
        let errors = module
            .get("errors")
            .and_then(serde_json::Value::as_u64)
            .unwrap();
        assert!(
            errors > 0,
            "fixture must produce a type error, got {module}"
        );

        assert!(
            module.get("diagnostics").is_some(),
            "module node must carry a `diagnostics` array so the `errors` tally \
             is navigable ([EXTACT-MODULES-DIAGNOSTICS], #235), got {module}"
        );
        let diagnostics = module
            .get("diagnostics")
            .and_then(serde_json::Value::as_array)
            .unwrap();

        // Count invariant from [EXTACT-DATA-MODEL].
        let error_rows = diagnostics
            .iter()
            .filter(|d| d.get("severity").and_then(serde_json::Value::as_str) == Some("error"))
            .count();
        assert_eq!(
            u64::try_from(error_rows).unwrap(),
            errors,
            "errors tally must equal the number of error-severity diagnostic rows"
        );

        // Each row is the spec's DiagnosticNode shape.
        for entry in diagnostics {
            for field in ["severity", "code", "message", "line", "character"] {
                assert!(
                    entry.get(field).is_some(),
                    "diagnostic row missing `{field}`: {entry}"
                );
            }
        }

        // Single-line fixture: the error anchors to line 0 (zero-based).
        assert_eq!(
            diagnostics[0]
                .get("line")
                .and_then(serde_json::Value::as_u64),
            Some(0),
            "diagnostic line must be the zero-based source line"
        );
    }

    // [ANALYSIS-ENABLED] × [EXTACT-MODULES-DIAGNOSTICS]: with type checking
    // disabled the drill-down carries nothing — an empty array, mirroring the
    // omitted count fields, so no client can render stale diagnostics while
    // the toggle is off.
    #[test]
    fn test_disabled_toggle_serves_empty_diagnostics() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        let uri = make_uri("/workspace/broken.py");
        let _ = idx.set_open(&uri, "x: int = \"not an int\"\n", 1);

        let tree = build_module_tree(&idx, "", false, true);
        assert_eq!(tree.modules.len(), 1);
        assert_eq!(
            tree.modules[0].get("diagnostics"),
            Some(&serde_json::json!([])),
            "disabled toggle must serve an EMPTY diagnostics list, got {}",
            tree.modules[0]
        );
    }

    /// Hand-built diagnostic for the ordering/filtering tests below.
    fn make_diag(
        severity: basilisk_checker::Severity,
        start: u32,
        message: &str,
    ) -> basilisk_checker::Diagnostic {
        basilisk_checker::Diagnostic {
            code: basilisk_checker::ErrorCode {
                code: "test_rule",
                docs_url: "https://example.invalid/test_rule",
            },
            severity,
            message: message.to_owned(),
            span: basilisk_resolver::Span::new(start, start + 1),
            path: "/workspace/x.py".to_owned(),
            help: None,
            note: None,
            provenance: None,
        }
    }

    // Tests the ordering + severity-filter rules of [EXTACT-MODULES-DIAGNOSTICS]:
    // errors before warnings, then ascending line; Info and the opt-in
    // SafetyViolation stay off the wire so the count invariant against
    // compute_file_health holds exactly.
    #[test]
    fn test_diagnostic_nodes_sorts_errors_first_then_line_and_filters_severities() {
        use basilisk_checker::Severity;
        // Offsets land on lines 0..4 of this 5-line text (6 bytes per line).
        let text = "aaaaa\nbbbbb\nccccc\nddddd\neeeee\n";
        let diagnostics = vec![
            make_diag(Severity::Warning, 0, "warning on line 0"),
            make_diag(Severity::Error, 18, "error on line 3"),
            make_diag(Severity::Info, 6, "info stays off the wire"),
            make_diag(Severity::Error, 6, "error on line 1"),
            make_diag(Severity::SafetyViolation, 12, "safety stays off the wire"),
        ];

        let rows = diagnostic_nodes(&diagnostics, text);

        let rendered: Vec<(&str, u64)> = rows
            .iter()
            .map(|row| {
                (
                    row.get("message")
                        .and_then(serde_json::Value::as_str)
                        .unwrap(),
                    row.get("line").and_then(serde_json::Value::as_u64).unwrap(),
                )
            })
            .collect();
        assert_eq!(
            rendered,
            vec![
                ("error on line 1", 1),
                ("error on line 3", 3),
                ("warning on line 0", 0),
            ],
            "errors before warnings, then ascending line; Info/SafetyViolation excluded"
        );
        // Every row carries the full DiagnosticNode shape.
        for row in &rows {
            for field in ["severity", "code", "message", "line", "character"] {
                assert!(row.get(field).is_some(), "row missing `{field}`: {row}");
            }
        }
    }

    #[test]
    fn test_build_module_tree_folds_health_rollup() {
        let root = PathBuf::from("/workspace");
        let idx = make_index_with_roots(vec![root]);
        // Fully annotated file (100%) and a half-annotated file.
        let uri_full = make_uri("/workspace/full.py");
        let uri_partial = make_uri("/workspace/partial.py");
        let _ = idx.set_open(&uri_full, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri_partial, "a: int = 1\nb = 2\n", 1);

        let tree = build_module_tree(&idx, "", true, true);

        // Every module node carries its folded health fields.
        for module in &tree.modules {
            for field in ["coveragePercent", "errors", "warnings"] {
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
