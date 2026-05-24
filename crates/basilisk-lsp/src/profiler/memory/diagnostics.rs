//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Generate LSP diagnostics from memory profiling data.
//!
//! Converts allocation snapshots, snapshot diffs, and gc collection results
//! into `publishDiagnostics` entries. Uses the purple memory palette:
//! - `BSK-MEM-ALLOC` (Hint) for allocation hotspots
//! - `BSK-MEM-GROWTH` (Warning) for memory growth between snapshots
//! - `BSK-MEM-LEAK` (Warning/Error) for suspected/definite leaks
//! - `BSK-MEM-CYCLE` (Error) for uncollectable reference cycles

use std::collections::HashMap;

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Url};
use tracing::info;

use super::diff::MemoryDiff;
use super::leaks::{LeakConfidence, LeakTracker, SuspectedLeak};
use super::{format_bytes, AllocationSite, MemorySnapshot};

/// Source identifier for all memory diagnostics.
const SOURCE: &str = "basilisk-memory";

/// A set of diagnostics grouped by file URI.
pub type DiagnosticsByUri = HashMap<Url, Vec<Diagnostic>>;

/// Configuration for memory allocation hotspot thresholds.
#[derive(Debug, Clone)]
pub struct MemoryHotspotConfig {
    /// Minimum allocation size in bytes to generate a diagnostic.
    pub min_alloc_bytes: u64,
    /// Maximum number of allocation diagnostics per file.
    pub max_per_file: usize,
}

impl Default for MemoryHotspotConfig {
    fn default() -> Self {
        Self {
            min_alloc_bytes: 1_048_576, // 1 MB
            max_per_file: 20,
        }
    }
}

/// Generate allocation diagnostics from a memory snapshot.
///
/// Emits `BSK-MEM-ALLOC` hints for each allocation site above the
/// configured threshold.
#[must_use]
pub fn generate_allocation_diagnostics(
    snapshot: &MemorySnapshot,
    config: &MemoryHotspotConfig,
) -> DiagnosticsByUri {
    let mut result: DiagnosticsByUri = HashMap::new();

    for alloc in &snapshot.top_allocations {
        if alloc.size < config.min_alloc_bytes {
            continue;
        }
        let Ok(uri) = Url::from_file_path(&alloc.file) else {
            continue;
        };
        let entry = result.entry(uri).or_default();
        if entry.len() >= config.max_per_file {
            continue;
        }
        entry.push(build_alloc_diagnostic(alloc));
    }

    info!(
        allocations = snapshot.top_allocations.len(),
        files = result.len(),
        "generated memory allocation diagnostics"
    );

    result
}

/// Generate growth and leak diagnostics from a snapshot diff.
///
/// Feeds growths through the `LeakTracker` for confidence scoring:
/// - `BSK-MEM-GROWTH` warnings for all growing allocations
/// - `BSK-MEM-LEAK` for suspected leaks (Medium+ confidence)
#[must_use]
pub fn generate_diff_diagnostics(
    diff: &MemoryDiff,
    leak_tracker: &mut LeakTracker,
) -> DiagnosticsByUri {
    let mut result: DiagnosticsByUri = HashMap::new();
    let suspected = leak_tracker.process_growths(&diff.grown_allocations);

    for leak in &suspected {
        let Ok(uri) = Url::from_file_path(&leak.file) else {
            continue;
        };
        let diagnostics = result.entry(uri).or_default();
        diagnostics.push(build_growth_diagnostic(leak));

        if leak.confidence >= LeakConfidence::Medium {
            diagnostics.push(build_leak_diagnostic(leak));
        }
    }

    info!(
        growths = diff.grown_allocations.len(),
        suspected_leaks = suspected.len(),
        files = result.len(),
        "generated memory diff diagnostics"
    );

    result
}

/// Parsed uncollectable object from gc collect output.
#[derive(Debug, Clone)]
pub struct UncollectableObject {
    /// Python type name.
    pub type_name: String,
    /// Object `repr` (truncated).
    pub repr: String,
    /// Object size in bytes.
    pub size: u64,
    /// Reason the object is uncollectable.
    pub reason: String,
}

/// Parsed gc collect result for cycle diagnostics.
#[derive(Debug, Clone)]
pub struct GcCollectResult {
    /// Number of objects collected.
    pub collected: u64,
    /// Number of uncollectable objects.
    pub uncollectable_count: u64,
    /// Memory freed in bytes.
    pub memory_freed: u64,
    /// Uncollectable objects with details.
    pub uncollectable_objects: Vec<UncollectableObject>,
}

/// Parse gc collect JSON output into a structured result.
///
/// # Errors
///
/// Returns an error if the JSON is malformed.
pub fn parse_gc_result(json_str: &str) -> Result<GcCollectResult, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|err| format!("invalid gc JSON: {err}"))?;

    Ok(GcCollectResult {
        collected: json_u64(&value, "collected"),
        uncollectable_count: json_u64(&value, "uncollectable"),
        memory_freed: json_u64(&value, "memoryFreed"),
        uncollectable_objects: value
            .get("uncollectableObjects")
            .and_then(serde_json::Value::as_array)
            .map(|arr| arr.iter().map(parse_uncollectable).collect())
            .unwrap_or_default(),
    })
}

/// Generate cycle diagnostics from gc collect results.
///
/// Emits `BSK-MEM-CYCLE` errors for each uncollectable object reported
/// by Python's garbage collector.
#[must_use]
pub fn generate_cycle_diagnostics(gc_result: &GcCollectResult) -> DiagnosticsByUri {
    let mut result: DiagnosticsByUri = HashMap::new();

    for obj in &gc_result.uncollectable_objects {
        let uri = cycle_diagnostic_uri();
        result
            .entry(uri)
            .or_default()
            .push(build_cycle_diagnostic(obj));
    }

    info!(
        uncollectable = gc_result.uncollectable_count,
        diagnostics = result.values().map(Vec::len).sum::<usize>(),
        "generated memory cycle diagnostics"
    );

    result
}

/// Clear memory diagnostics for the given URIs.
#[must_use]
pub fn clear_diagnostics(previous_uris: &[Url]) -> DiagnosticsByUri {
    previous_uris
        .iter()
        .map(|uri| (uri.clone(), Vec::new()))
        .collect()
}

// ── Private helpers ──────────────────────────────────────────────────────

/// Build a `BSK-MEM-ALLOC` diagnostic for an allocation site.
fn build_alloc_diagnostic(alloc: &AllocationSite) -> Diagnostic {
    Diagnostic {
        range: line_range(alloc.line),
        severity: Some(DiagnosticSeverity::HINT),
        code: Some(NumberOrString::String(
            basilisk_common::memory_diagnostics::ALLOC.to_owned(),
        )),
        source: Some(SOURCE.to_owned()),
        message: format!(
            "Memory hotspot: {} allocated ({} objects)",
            format_bytes(alloc.size),
            alloc.count,
        ),
        data: Some(serde_json::json!({
            "size": alloc.size,
            "count": alloc.count,
        })),
        ..Diagnostic::default()
    }
}

/// Build a `BSK-MEM-GROWTH` diagnostic from a suspected leak.
fn build_growth_diagnostic(leak: &SuspectedLeak) -> Diagnostic {
    let growth_bytes = u64::try_from(leak.size_growth).unwrap_or(0);
    Diagnostic {
        range: line_range(leak.line),
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(
            basilisk_common::memory_diagnostics::GROWTH.to_owned(),
        )),
        source: Some(SOURCE.to_owned()),
        message: format!(
            "Memory growth: +{} ({} new objects)",
            format_bytes(growth_bytes),
            leak.count_growth,
        ),
        data: Some(serde_json::json!({
            "sizeGrowth": leak.size_growth,
            "countGrowth": leak.count_growth,
            "currentSize": leak.current_size,
        })),
        ..Diagnostic::default()
    }
}

/// Build a `BSK-MEM-LEAK` diagnostic from a suspected leak.
fn build_leak_diagnostic(leak: &SuspectedLeak) -> Diagnostic {
    Diagnostic {
        range: line_range(leak.line),
        severity: Some(severity_for_confidence(leak.confidence)),
        code: Some(NumberOrString::String(
            basilisk_common::memory_diagnostics::LEAK.to_owned(),
        )),
        source: Some(SOURCE.to_owned()),
        message: format!(
            "Suspected memory leak ({} confidence): {} \u{2014} {}",
            leak.confidence,
            format_bytes(leak.current_size),
            leak.reason,
        ),
        data: Some(serde_json::json!({
            "confidence": leak.confidence.to_string(),
            "currentSize": leak.current_size,
            "reason": leak.reason,
        })),
        ..Diagnostic::default()
    }
}

/// Build a `BSK-MEM-CYCLE` diagnostic for an uncollectable object.
fn build_cycle_diagnostic(obj: &UncollectableObject) -> Diagnostic {
    Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(
            basilisk_common::memory_diagnostics::CYCLE.to_owned(),
        )),
        source: Some(SOURCE.to_owned()),
        message: format!(
            "Uncollectable reference cycle: {} ({}) \u{2014} {}",
            obj.type_name,
            format_bytes(obj.size),
            obj.reason,
        ),
        data: Some(serde_json::json!({
            "typeName": obj.type_name,
            "size": obj.size,
            "repr": obj.repr,
            "reason": obj.reason,
        })),
        ..Diagnostic::default()
    }
}

/// Parse a single uncollectable object from gc JSON.
fn parse_uncollectable(value: &serde_json::Value) -> UncollectableObject {
    UncollectableObject {
        type_name: json_str(value, "type", "<unknown>"),
        repr: json_str(value, "repr", ""),
        size: json_u64(value, "size"),
        reason: json_str(value, "reason", "Uncollectable cycle"),
    }
}

/// Extract a `u64` field from JSON, defaulting to 0.
fn json_u64(value: &serde_json::Value, key: &str) -> u64 {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

/// Extract a string field from JSON with a default.
fn json_str(value: &serde_json::Value, key: &str, default: &str) -> String {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
        .to_owned()
}

/// Map leak confidence to diagnostic severity.
fn severity_for_confidence(confidence: LeakConfidence) -> DiagnosticSeverity {
    match confidence {
        LeakConfidence::Definite => DiagnosticSeverity::ERROR,
        LeakConfidence::High | LeakConfidence::Medium => DiagnosticSeverity::WARNING,
        LeakConfidence::Low => DiagnosticSeverity::HINT,
    }
}

/// Build a `Range` covering an entire line (0-based LSP lines).
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

/// Synthetic URI for cycle diagnostics (not file-specific).
///
/// Uses a `basilisk://` scheme URI so editors can display these in a
/// diagnostics panel without a real file path.
fn cycle_diagnostic_uri() -> Url {
    /// Compile-time constant URI for cycle diagnostics.
    const CYCLE_URI: &str = "basilisk://memory/cycles";
    /// Fallback URI if the primary fails to parse.
    const FALLBACK_URI: &str = "file:///dev/null";

    Url::parse(CYCLE_URI)
        .or_else(|_| Url::parse(FALLBACK_URI))
        .unwrap_or_else(|_| {
            // Both are compile-time constant valid URLs; this branch
            // is unreachable. Construct a minimal data URI as last resort.
            let mut url = Url::parse("data:,").unwrap_or_else(|_| {
                // data:, is the simplest valid URL per RFC 2397.
                // All three failing is a url-crate bug, not our bug.
                std::process::abort();
            });
            url.set_path("/cycles");
            url
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiler::memory::diff::{AllocationGrowth, MemoryDiff, TraceFrame};
    use crate::profiler::memory::{AllocationSite, MemorySnapshot};

    fn make_snapshot() -> MemorySnapshot {
        MemorySnapshot {
            snapshot_id: "test-snap".to_owned(),
            current_memory: 100_000_000,
            peak_memory: 120_000_000,
            gc_objects: 5000,
            gc_counts: vec![100, 10, 1],
            top_allocations: vec![
                AllocationSite {
                    file: "/tmp/app.py".to_owned(),
                    line: 42,
                    size: 24_000_000,
                    count: 1500,
                    traceback: vec![TraceFrame {
                        file: "/tmp/app.py".to_owned(),
                        line: 42,
                    }],
                },
                AllocationSite {
                    file: "/tmp/app.py".to_owned(),
                    line: 78,
                    size: 500_000,
                    count: 200,
                    traceback: vec![],
                },
            ],
        }
    }

    fn make_diff() -> MemoryDiff {
        MemoryDiff {
            total_growth: 18_000_000,
            total_freed: 2_000_000,
            net_growth: 16_000_000,
            grown_allocations: vec![AllocationGrowth {
                file: "/tmp/cache.py".to_owned(),
                line: 34,
                size_diff: 18_000_000,
                count_diff: 5000,
                size: 24_000_000,
                count: 8000,
                traceback: vec![TraceFrame {
                    file: "/tmp/cache.py".to_owned(),
                    line: 34,
                }],
            }],
            freed_allocations: vec![],
        }
    }

    fn has_code(diag: &Diagnostic, expected: &str) -> bool {
        diag.code
            .as_ref()
            .is_some_and(|c| matches!(c, NumberOrString::String(s) if s == expected))
    }

    #[test]
    fn allocation_diagnostics_above_threshold() -> Result<(), String> {
        let snapshot = make_snapshot();
        let config = MemoryHotspotConfig::default();
        let diags = generate_allocation_diagnostics(&snapshot, &config);

        let uri = Url::from_file_path("/tmp/app.py").map_err(|()| "bad URI")?;
        let file_diags = diags
            .get(&uri)
            .ok_or("expected diagnostics for /tmp/app.py")?;

        // Only the 24 MB allocation exceeds the 1 MB default threshold.
        assert_eq!(file_diags.len(), 1);
        let diag = file_diags.first().ok_or("expected one diagnostic")?;
        assert_eq!(
            diag.code,
            Some(NumberOrString::String("BSK-MEM-ALLOC".to_owned()))
        );
        assert_eq!(diag.severity, Some(DiagnosticSeverity::HINT));
        assert!(
            diag.message.contains("allocated"),
            "message should mention allocation: {}",
            diag.message
        );
        assert_eq!(diag.range.start.line, 41);
        assert_eq!(diag.source.as_deref(), Some("basilisk-memory"));
        Ok(())
    }

    #[test]
    fn allocation_diagnostics_custom_threshold() -> Result<(), String> {
        let snapshot = make_snapshot();
        let config = MemoryHotspotConfig {
            min_alloc_bytes: 100_000,
            max_per_file: 10,
        };
        let diags = generate_allocation_diagnostics(&snapshot, &config);

        let uri = Url::from_file_path("/tmp/app.py").map_err(|()| "bad URI")?;
        let file_diags = diags.get(&uri).ok_or("expected diagnostics")?;

        // Both allocations exceed 100 KB threshold.
        assert_eq!(file_diags.len(), 2);
        Ok(())
    }

    #[test]
    fn allocation_diagnostics_respects_max_per_file() -> Result<(), String> {
        let mut snapshot = make_snapshot();
        for idx in 0..30 {
            snapshot.top_allocations.push(AllocationSite {
                file: "/tmp/app.py".to_owned(),
                line: 100 + idx,
                size: 2_000_000,
                count: 10,
                traceback: vec![],
            });
        }

        let config = MemoryHotspotConfig {
            min_alloc_bytes: 100_000,
            max_per_file: 5,
        };
        let diags = generate_allocation_diagnostics(&snapshot, &config);

        let uri = Url::from_file_path("/tmp/app.py").map_err(|()| "bad URI")?;
        let file_diags = diags.get(&uri).ok_or("expected diagnostics")?;

        assert_eq!(file_diags.len(), 5, "should cap at max_per_file");
        Ok(())
    }

    #[test]
    fn diff_diagnostics_with_large_growth() -> Result<(), String> {
        let diff = make_diff();
        let mut tracker = LeakTracker::new();
        let diags = generate_diff_diagnostics(&diff, &mut tracker);

        let uri = Url::from_file_path("/tmp/cache.py").map_err(|()| "bad URI")?;
        let file_diags = diags
            .get(&uri)
            .ok_or("expected diagnostics for /tmp/cache.py")?;

        // 18 MB growth => Medium confidence => both GROWTH and LEAK.
        let growth_diags: Vec<_> = file_diags
            .iter()
            .filter(|d| has_code(d, "BSK-MEM-GROWTH"))
            .collect();
        assert_eq!(growth_diags.len(), 1);
        if let Some(first_growth) = growth_diags.first() {
            assert_eq!(first_growth.severity, Some(DiagnosticSeverity::WARNING));
        }

        let leak_diags: Vec<_> = file_diags
            .iter()
            .filter(|d| has_code(d, "BSK-MEM-LEAK"))
            .collect();
        assert_eq!(leak_diags.len(), 1);
        if let Some(first_leak) = leak_diags.first() {
            assert!(
                first_leak.message.contains("MEDIUM"),
                "leak message should contain confidence level"
            );
        }
        Ok(())
    }

    #[test]
    fn diff_diagnostics_escalate_with_repeated_growth() -> Result<(), String> {
        let diff = make_diff();
        let mut tracker = LeakTracker::new();

        let _ = generate_diff_diagnostics(&diff, &mut tracker);
        let _ = generate_diff_diagnostics(&diff, &mut tracker);
        let diags = generate_diff_diagnostics(&diff, &mut tracker);

        let uri = Url::from_file_path("/tmp/cache.py").map_err(|()| "bad URI")?;
        let file_diags = diags.get(&uri).ok_or("expected diagnostics")?;

        let leak_diags: Vec<_> = file_diags
            .iter()
            .filter(|d| has_code(d, "BSK-MEM-LEAK"))
            .collect();
        assert_eq!(leak_diags.len(), 1);
        if let Some(first_leak) = leak_diags.first() {
            assert!(
                first_leak.message.contains("HIGH"),
                "third consecutive growth should be HIGH confidence"
            );
        }
        Ok(())
    }

    #[test]
    fn low_confidence_produces_no_leak_diagnostic() -> Result<(), String> {
        let diff = MemoryDiff {
            total_growth: 1000,
            total_freed: 0,
            net_growth: 1000,
            grown_allocations: vec![AllocationGrowth {
                file: "/tmp/small.py".to_owned(),
                line: 10,
                size_diff: 1000,
                count_diff: 5,
                size: 2000,
                count: 10,
                traceback: vec![],
            }],
            freed_allocations: vec![],
        };

        let mut tracker = LeakTracker::new();
        let diags = generate_diff_diagnostics(&diff, &mut tracker);

        let uri = Url::from_file_path("/tmp/small.py").map_err(|()| "bad URI")?;
        let file_diags = diags.get(&uri).ok_or("expected diagnostics")?;

        let growth_count = file_diags
            .iter()
            .filter(|d| has_code(d, "BSK-MEM-GROWTH"))
            .count();
        let leak_count = file_diags
            .iter()
            .filter(|d| has_code(d, "BSK-MEM-LEAK"))
            .count();

        assert_eq!(growth_count, 1, "should have one growth diagnostic");
        assert_eq!(
            leak_count, 0,
            "low confidence should not produce leak diagnostic"
        );
        Ok(())
    }

    #[test]
    fn cycle_diagnostics_from_gc_collect() -> Result<(), String> {
        let gc_json = r#"{
            "collected": 42,
            "uncollectable": 2,
            "memoryFreed": 8192,
            "uncollectableObjects": [
                {
                    "id": 12345,
                    "type": "CacheNode",
                    "size": 4096,
                    "repr": "<CacheNode key='session'>",
                    "reason": "Instance has __del__ method and is in a reference cycle"
                },
                {
                    "id": 67890,
                    "type": "Connection",
                    "size": 2048,
                    "repr": "<Connection closed>",
                    "reason": "Uncollectable cycle"
                }
            ]
        }"#;

        let gc_result = parse_gc_result(gc_json)?;
        assert_eq!(gc_result.collected, 42);
        assert_eq!(gc_result.uncollectable_count, 2);
        assert_eq!(gc_result.memory_freed, 8192);
        assert_eq!(gc_result.uncollectable_objects.len(), 2);

        let diags = generate_cycle_diagnostics(&gc_result);
        let uri = cycle_diagnostic_uri();
        let cycle_diags = diags.get(&uri).ok_or("expected cycle diagnostics")?;

        assert_eq!(cycle_diags.len(), 2);
        for diag in cycle_diags {
            assert_eq!(
                diag.code,
                Some(NumberOrString::String("BSK-MEM-CYCLE".to_owned()))
            );
            assert_eq!(diag.severity, Some(DiagnosticSeverity::ERROR));
            assert_eq!(diag.source.as_deref(), Some("basilisk-memory"));
        }

        if let Some(first_cycle) = cycle_diags.first() {
            assert!(
                first_cycle.message.contains("CacheNode"),
                "message should contain type name"
            );
            assert!(
                first_cycle.message.contains("__del__"),
                "message should contain reason"
            );
        }
        Ok(())
    }

    #[test]
    fn parse_gc_result_empty_uncollectable() -> Result<(), String> {
        let gc_json = r#"{
            "collected": 100,
            "uncollectable": 0,
            "memoryFreed": 50000,
            "uncollectableObjects": []
        }"#;

        let gc_result = parse_gc_result(gc_json)?;
        assert_eq!(gc_result.collected, 100);
        assert!(gc_result.uncollectable_objects.is_empty());

        let diags = generate_cycle_diagnostics(&gc_result);
        assert!(
            diags.is_empty(),
            "no uncollectable objects = no diagnostics"
        );
        Ok(())
    }

    #[test]
    fn clear_diagnostics_empties_all() -> Result<(), String> {
        let uris = vec![
            Url::from_file_path("/tmp/a.py").map_err(|()| "bad URI")?,
            Url::from_file_path("/tmp/b.py").map_err(|()| "bad URI")?,
        ];
        let cleared = clear_diagnostics(&uris);
        assert_eq!(cleared.len(), 2);
        for diags in cleared.values() {
            assert!(diags.is_empty());
        }
        Ok(())
    }

    #[test]
    fn severity_maps_correctly() {
        assert_eq!(
            severity_for_confidence(LeakConfidence::Definite),
            DiagnosticSeverity::ERROR
        );
        assert_eq!(
            severity_for_confidence(LeakConfidence::High),
            DiagnosticSeverity::WARNING
        );
        assert_eq!(
            severity_for_confidence(LeakConfidence::Medium),
            DiagnosticSeverity::WARNING
        );
        assert_eq!(
            severity_for_confidence(LeakConfidence::Low),
            DiagnosticSeverity::HINT
        );
    }

    #[test]
    fn empty_snapshot_produces_no_diagnostics() {
        let snapshot = MemorySnapshot {
            snapshot_id: "empty".to_owned(),
            current_memory: 0,
            peak_memory: 0,
            gc_objects: 0,
            gc_counts: vec![],
            top_allocations: vec![],
        };
        let config = MemoryHotspotConfig::default();
        let diags = generate_allocation_diagnostics(&snapshot, &config);
        assert!(diags.is_empty());
    }

    #[test]
    fn empty_diff_produces_no_diagnostics() {
        let diff = MemoryDiff {
            total_growth: 0,
            total_freed: 0,
            net_growth: 0,
            grown_allocations: vec![],
            freed_allocations: vec![],
        };
        let mut tracker = LeakTracker::new();
        let diags = generate_diff_diagnostics(&diff, &mut tracker);
        assert!(diags.is_empty());
    }
}
