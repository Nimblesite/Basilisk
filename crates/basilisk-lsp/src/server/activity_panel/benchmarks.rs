//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Performance benchmarks for the activity panel LSP commands.
//!
//! Tests [EXTACT-PERFORMANCE]: the activity-panel handlers are computed
//! server-side from existing resolver/diagnostic data (no extra file I/O), and
//! these benches guard that they stay fast on large workspaces.
//!
//! Validates that key operations meet latency targets for 1000-file workspaces:
//! - `basilisk/workspaceModules`: <100ms
//! - `basilisk/typeHealth`: <50ms
//! - `basilisk/moduleChanged` data construction: <20ms per file
//!
//! Uses `std::time::Instant` for wall-clock timing in standard `#[test]` functions.

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only code: unwrap/expect acceptable in unit tests"
)]
mod tests {
    use std::path::PathBuf;
    use std::time::Instant;

    use tower_lsp::lsp_types::Url;

    use crate::config::AnalysisMode;
    use crate::workspace::WorkspaceIndex;

    use super::super::helpers::module_name_from_path;
    use super::super::module_tree::{build_module_tree, build_symbol_list};
    use super::super::type_health::build_type_health;

    /// Timing multiplier: debug builds are ~5x slower than release.
    #[cfg(debug_assertions)]
    const TIMING_MULTIPLIER: u128 = 5;
    #[cfg(not(debug_assertions))]
    const TIMING_MULTIPLIER: u128 = 1;

    /// Build a workspace index with `file_count` Python files, each containing
    /// a mix of annotated and unannotated symbols to exercise both the module
    /// tree builder and the type health scanner.
    fn build_large_workspace(file_count: usize) -> (WorkspaceIndex, PathBuf) {
        let root = PathBuf::from("/tmp/bench-workspace");
        let idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            basilisk_config::BasiliskConfig::default(),
        );

        for file_idx in 0..file_count {
            let package = format!("pkg_{:03}", file_idx / 10);
            let module = format!("mod_{:03}", file_idx % 10);
            let path = format!("/tmp/bench-workspace/{package}/{module}.py");
            let uri = Url::parse(&format!("file://{path}")).unwrap();

            // Each file has: 2 annotated functions, 1 unannotated function,
            // 1 annotated variable, 1 unannotated variable, 1 class with
            // 2 attributes — enough to stress both tree building and annotation counting.
            let src = format!(
                concat!(
                    "def func_a_{idx}(x: int) -> int:\n",
                    "    return x\n",
                    "\n",
                    "def func_b_{idx}(y: str) -> str:\n",
                    "    return y\n",
                    "\n",
                    "def func_c_{idx}(z):\n",
                    "    return z\n",
                    "\n",
                    "typed_var_{idx}: int = {idx}\n",
                    "untyped_var_{idx} = {idx}\n",
                    "\n",
                    "class Widget_{idx}:\n",
                    "    name: str = 'w'\n",
                    "    value = 42\n",
                ),
                idx = file_idx,
            );
            let _ = idx.set_open(&uri, &src, 1);
        }

        (idx, root)
    }

    #[test]
    fn bench_workspace_modules_under_100ms_for_1000_files() {
        let (idx, _root) = build_large_workspace(1000);

        let start = Instant::now();
        let result = build_module_tree(&idx, "", true, true);
        let elapsed = start.elapsed();

        let target_ms = 100 * TIMING_MULTIPLIER;
        assert!(
            elapsed.as_millis() < target_ms,
            "workspaceModules took {elapsed:?} for 1000 files (incl. folded health), target <{target_ms}ms"
        );
        assert!(
            result.modules.len() >= 1000,
            "expected at least 1000 modules in tree, got {}",
            result.modules.len()
        );

        // Verify structure: each module should have symbols and a folded health rollup.
        for module in result.modules.iter().take(5) {
            let symbols = module.get("symbols").and_then(serde_json::Value::as_array);
            assert!(
                symbols.is_some_and(|s| !s.is_empty()),
                "each module should have symbols"
            );
            assert!(
                module
                    .get("coveragePercent")
                    .and_then(serde_json::Value::as_u64)
                    .is_some(),
                "each module should carry a folded coveragePercent"
            );
        }

        // The workspace rollup is accumulated in the same single pass.
        let coverage = result
            .workspace
            .get("coveragePercent")
            .and_then(serde_json::Value::as_u64)
            .expect("workspace rollup coveragePercent missing");
        assert!(coverage <= 100, "coverage should be <= 100");
    }

    #[test]
    fn bench_type_health_under_50ms_for_1000_files() {
        let (idx, _root) = build_large_workspace(1000);

        let start = Instant::now();
        let health = build_type_health(&idx, true);
        let elapsed = start.elapsed();

        let target_ms = 50 * TIMING_MULTIPLIER;
        assert!(
            elapsed.as_millis() < target_ms,
            "typeHealth took {elapsed:?} for 1000 files, target <{target_ms}ms"
        );

        // Verify workspace-level stats are sane.
        let ws = health.get("workspace").expect("workspace stats missing");
        let total_symbols = ws
            .get("totalSymbols")
            .and_then(serde_json::Value::as_u64)
            .expect("totalSymbols missing");
        assert!(
            total_symbols >= 5000,
            "expected at least 5000 symbols for 1000 files, got {total_symbols}"
        );

        let coverage = ws
            .get("coveragePercent")
            .and_then(serde_json::Value::as_u64)
            .expect("coveragePercent missing");
        assert!(coverage <= 100, "coverage should be <= 100");

        // Verify module-level entries exist.
        let modules = health
            .get("modules")
            .and_then(serde_json::Value::as_array)
            .expect("modules array missing");
        assert!(
            modules.len() >= 1000,
            "expected at least 1000 module health entries, got {}",
            modules.len()
        );
    }

    #[test]
    fn bench_module_changed_data_under_20ms_per_file() {
        let root = PathBuf::from("/tmp/bench-workspace-changed");
        let idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            basilisk_config::BasiliskConfig::default(),
        );

        // Pre-populate a workspace with 100 files so the index is non-trivial.
        for file_idx in 0..100_usize {
            let path = format!("/tmp/bench-workspace-changed/mod_{file_idx:03}.py");
            let uri = Url::parse(&format!("file://{path}")).unwrap();
            let src = format!(
                "def func_{file_idx}(x: int) -> int:\n    return x\nvar_{file_idx}: int = {file_idx}\n"
            );
            let _ = idx.set_open(&uri, &src, 1);
        }

        // Measure the per-file cost of building module changed data
        // (the synchronous part of send_module_changed — reading index + building symbols).
        let mut total_elapsed = std::time::Duration::ZERO;
        let iterations = 50;

        for file_idx in 0..iterations {
            let path_str = format!("/tmp/bench-workspace-changed/mod_{file_idx:03}.py");
            let path = PathBuf::from(&path_str);

            let start = Instant::now();

            // Simulate the synchronous index lookup + symbol construction
            // from send_module_changed.
            let entry = idx.files.get(&path);
            if let Some(entry) = entry {
                let resolved = entry.resolved.as_ref();
                if let Some(resolved) = resolved {
                    let _module_name = module_name_from_path(&path, &idx.roots);
                    let _symbols = build_symbol_list(resolved, &entry.text);
                }
            }

            total_elapsed += start.elapsed();
        }

        let avg_ms = total_elapsed.as_millis() / u128::try_from(iterations).unwrap_or(1);
        let target_ms = 20 * TIMING_MULTIPLIER;
        assert!(
            avg_ms < target_ms,
            "moduleChanged data construction averaged {avg_ms}ms per file, target <{target_ms}ms \
             (total {total_elapsed:?} for {iterations} files)"
        );
    }
}
