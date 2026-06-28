//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Generate LSP diagnostics from profiling data.
//!
//! Converts hot lines and hot functions into `publishDiagnostics` entries
//! with severity `Hint`, source `"basilisk-profiler"`, and structured data
//! payloads for editor decorations.

use std::collections::HashMap;

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url};
use tracing::info;

use super::aggregator::{HotspotConfig, ProfileData};

/// Diagnostic code for hot lines ([PROFILE-CONFIG-CODES]: `BSK-PROF-LINE`, Hint).
const CODE_PROF_LINE: &str = "BSK-PROF-LINE";
/// Diagnostic code for hot functions ([PROFILE-CONFIG-CODES]: `BSK-PROF-FUNC`, Hint).
const CODE_PROF_FUNC: &str = "BSK-PROF-FUNC";
/// Source identifier for all profiling diagnostics. Implements
/// [PROFILE-NOTIFICATIONS-DIAG]: profiling diagnostics use a distinct
/// `"basilisk-profiler"` source and severity `Hint` so they never inflate
/// error/warning counts.
const SOURCE: &str = "basilisk-profiler";

/// A set of diagnostics grouped by file URI.
pub type DiagnosticsByUri = HashMap<Url, Vec<Diagnostic>>;

/// Generate profiling diagnostics from profile data.
///
/// Returns diagnostics grouped by file URI, ready to be published via
/// `textDocument/publishDiagnostics`.
#[must_use]
pub fn generate_diagnostics(data: &ProfileData, config: &HotspotConfig) -> DiagnosticsByUri {
    let mut result: DiagnosticsByUri = HashMap::new();

    // Hot lines → BSK-PROF-LINE diagnostics.
    let hot_lines = data.hot_lines(config);
    for line in &hot_lines {
        let Ok(uri) = Url::from_file_path(&line.file) else {
            continue;
        };

        let diagnostic = Diagnostic {
            range: line_range(line.line),
            severity: Some(DiagnosticSeverity::HINT),
            code: Some(NumberOrString::String(CODE_PROF_LINE.to_owned())),
            source: Some(SOURCE.to_owned()),
            message: format!(
                "Hot line: {:.1}% CPU ({}/{} samples)",
                line.percentage, line.samples, data.total_samples
            ),
            data: Some(serde_json::json!({
                "samples": line.samples,
                "totalSamples": data.total_samples,
                "percentage": line.percentage,
            })),
            ..Diagnostic::default()
        };

        result.entry(uri).or_default().push(diagnostic);
    }

    // Hot functions → BSK-PROF-FUNC diagnostics.
    let hot_funcs = data.hot_functions(config);
    for func in &hot_funcs {
        let Ok(uri) = Url::from_file_path(&func.file) else {
            continue;
        };

        let diagnostic = Diagnostic {
            range: line_range(func.line),
            severity: Some(DiagnosticSeverity::HINT),
            code: Some(NumberOrString::String(CODE_PROF_FUNC.to_owned())),
            source: Some(SOURCE.to_owned()),
            message: format!(
                "Hot function: {} \u{2014} {:.1}% CPU ({} samples, {:.1}% self)",
                func.name, func.percentage, func.samples, func.self_percentage
            ),
            data: Some(serde_json::json!({
                "samples": func.samples,
                "selfSamples": func.self_samples,
                "percentage": func.percentage,
                "selfPercentage": func.self_percentage,
            })),
            ..Diagnostic::default()
        };

        result.entry(uri).or_default().push(diagnostic);
    }

    info!(
        hot_lines = hot_lines.len(),
        hot_functions = hot_funcs.len(),
        files = result.len(),
        "generated profiling diagnostics"
    );

    result
}

/// Clear profiling diagnostics for all files by publishing empty diagnostic
/// sets. Call this when the user clears profile results or starts a new session.
#[must_use]
pub fn clear_diagnostics(previous_uris: &[Url]) -> DiagnosticsByUri {
    previous_uris
        .iter()
        .map(|uri| (uri.clone(), Vec::new()))
        .collect()
}

/// Build a `Range` covering an entire line (0-based LSP lines).
///
/// py-spy reports 1-based line numbers; LSP uses 0-based.
fn line_range(line_1based: i32) -> Range {
    let line_0based = u32::try_from((line_1based - 1).max(0)).unwrap_or(0);
    Range {
        start: Position {
            line: line_0based,
            character: 0,
        },
        end: Position {
            line: line_0based,
            character: 999,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiler::aggregator::FunctionStats;

    /// Helper to create a file URI, mapping the `()` error to a `String`.
    fn file_uri(path: &str) -> Result<Url, String> {
        Url::from_file_path(path).map_err(|()| format!("failed to create URI from {path}"))
    }

    /// Build profile data with known hot spots for testing.
    fn make_test_profile() -> ProfileData {
        let mut data = ProfileData {
            total_samples: 100,
            ..ProfileData::default()
        };

        // Hot line: /tmp/test.py:42 with 40 hits.
        let _ = data
            .line_hits
            .entry("/tmp/test.py".to_owned())
            .or_default()
            .insert(42, 40);

        // Warm line: /tmp/test.py:50 with 10 hits.
        let _ = data
            .line_hits
            .entry("/tmp/test.py".to_owned())
            .or_default()
            .insert(50, 10);

        // Cold line: /tmp/test.py:60 with 0 hits (below default 1% threshold).
        let _ = data
            .line_hits
            .entry("/tmp/test.py".to_owned())
            .or_default()
            .insert(60, 0);

        // Hot function.
        let _ = data
            .function_stats
            .entry("/tmp/test.py".to_owned())
            .or_default()
            .insert(
                "process_data".to_owned(),
                FunctionStats {
                    name: "process_data".to_owned(),
                    file: "/tmp/test.py".to_owned(),
                    line: 42,
                    total_samples: 40,
                    self_samples: 30,
                },
            );

        data
    }

    // [PROFILE-NOTIFICATIONS-DIAG]/[PROFILE-CONFIG-CODES] Hot lines become
    // `BSK-PROF-LINE` Hint diagnostics with the `basilisk-profiler` source.
    #[test]
    fn generates_line_diagnostics() -> Result<(), String> {
        let data = make_test_profile();
        let config = HotspotConfig::default();
        let diags = generate_diagnostics(&data, &config);

        let uri = file_uri("/tmp/test.py")?;
        let file_diags = diags
            .get(&uri)
            .ok_or("should have diagnostics for /tmp/test.py")?;

        // Should have at least one BSK-PROF-LINE diagnostic.
        let line_diags: Vec<_> = file_diags
            .iter()
            .filter(|d| {
                d.code
                    .as_ref()
                    .is_some_and(|c| matches!(c, NumberOrString::String(s) if s == CODE_PROF_LINE))
            })
            .collect();

        assert!(
            !line_diags.is_empty(),
            "should generate hot line diagnostics"
        );

        // All should be Hint severity.
        for diag in &line_diags {
            assert_eq!(diag.severity, Some(DiagnosticSeverity::HINT));
            assert_eq!(diag.source.as_deref(), Some(SOURCE));
        }

        Ok(())
    }

    #[test]
    fn generates_function_diagnostics() -> Result<(), String> {
        let data = make_test_profile();
        let config = HotspotConfig::default();
        let diags = generate_diagnostics(&data, &config);

        let uri = file_uri("/tmp/test.py")?;
        let file_diags = diags
            .get(&uri)
            .ok_or("should have diagnostics for /tmp/test.py")?;

        let func_diags: Vec<_> = file_diags
            .iter()
            .filter(|d| {
                d.code
                    .as_ref()
                    .is_some_and(|c| matches!(c, NumberOrString::String(s) if s == CODE_PROF_FUNC))
            })
            .collect();

        assert!(
            !func_diags.is_empty(),
            "should generate hot function diagnostics"
        );

        // Check message contains function name.
        let msg = &func_diags
            .first()
            .ok_or("expected at least one function diagnostic")?
            .message;
        assert!(
            msg.contains("process_data"),
            "message should contain function name"
        );

        Ok(())
    }

    #[test]
    fn line_range_converts_1based_to_0based() {
        let range = line_range(42);
        assert_eq!(range.start.line, 41);
        assert_eq!(range.start.character, 0);
    }

    #[test]
    fn clear_diagnostics_empties_all() -> Result<(), String> {
        let uris = vec![file_uri("/tmp/a.py")?, file_uri("/tmp/b.py")?];
        let cleared = clear_diagnostics(&uris);
        assert_eq!(cleared.len(), 2);
        for diags in cleared.values() {
            assert!(diags.is_empty());
        }
        Ok(())
    }

    #[test]
    fn empty_profile_produces_no_diagnostics() {
        let data = ProfileData::default();
        let config = HotspotConfig::default();
        let diags = generate_diagnostics(&data, &config);
        assert!(diags.is_empty());
    }

    #[test]
    fn diagnostic_generation_under_100ms_for_large_profile() {
        // Phase 6 benchmark target: diagnostic generation <100ms for 60K samples.
        let mut data = ProfileData {
            total_samples: 60_000,
            ..ProfileData::default()
        };

        // Simulate 100 files, 50 hot lines each.
        for file_idx in 0..100_u32 {
            let file = format!("/tmp/module_{file_idx:03}.py");
            let line_map = data.line_hits.entry(file.clone()).or_default();
            for line in 1..=50 {
                let _ = line_map.insert(line, u64::from(file_idx + 1) * 12);
            }
            let func_map = data.function_stats.entry(file.clone()).or_default();
            let _ = func_map.insert(
                format!("func_{file_idx}"),
                FunctionStats {
                    name: format!("func_{file_idx}"),
                    file,
                    line: 1,
                    total_samples: u64::from(file_idx + 1) * 50,
                    self_samples: u64::from(file_idx + 1) * 25,
                },
            );
        }

        let config = HotspotConfig::default();
        let start = std::time::Instant::now();
        let diags = generate_diagnostics(&data, &config);
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 100,
            "diagnostic generation took {elapsed:?}, should be <100ms"
        );
        assert!(
            !diags.is_empty(),
            "should generate diagnostics from large profile"
        );
    }
}
