//! Sample aggregation for profiling sessions.
//!
//! Accumulates stack traces from py-spy into per-file, per-line hit counts
//! and per-function statistics. Thread-safe: receives samples via channel,
//! queried from the LSP thread for diagnostics and export.

use std::collections::HashMap;

use serde::Serialize;

/// Accumulated profiling data for a single session.
#[derive(Debug, Clone, Default)]
pub struct ProfileData {
    /// `file path -> line number -> sample count`.
    pub line_hits: HashMap<String, HashMap<i32, u64>>,

    /// `file path -> function name -> FunctionStats`.
    pub function_stats: HashMap<String, HashMap<String, FunctionStats>>,

    /// Total samples collected.
    pub total_samples: u64,

    /// Per-thread sample counts.
    pub thread_samples: HashMap<u64, u64>,

    /// Raw frame list for speedscope export (frame index dedup).
    pub frame_index: HashMap<FrameKey, usize>,

    /// Ordered frames for speedscope shared.frames.
    pub frames: Vec<SpeedscopeFrame>,

    /// Per-thread sample stacks (indices into `frames`).
    pub thread_stacks: HashMap<u64, Vec<Vec<usize>>>,

    /// Per-thread sample weights (seconds per sample).
    pub thread_weights: HashMap<u64, Vec<f64>>,

    /// Per-thread names, keyed by thread ID.
    pub thread_names: HashMap<u64, String>,
}

/// Key for deduplicating frames in the speedscope export.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FrameKey {
    /// Function name.
    pub name: String,
    /// Source file path.
    pub file: String,
    /// Line number.
    pub line: i32,
}

/// A single frame in the speedscope shared frames list.
#[derive(Debug, Clone, Serialize)]
pub struct SpeedscopeFrame {
    /// Function name.
    pub name: String,
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: i32,
}

/// Per-function profiling statistics.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionStats {
    /// Function name.
    pub name: String,
    /// Source file path.
    pub file: String,
    /// Line number of the function definition.
    pub line: i32,
    /// Samples where this function appears anywhere in the stack.
    pub total_samples: u64,
    /// Samples where this function is the leaf (top of stack).
    pub self_samples: u64,
}

/// Configuration thresholds for what counts as "hot".
#[derive(Debug, Clone)]
pub struct HotspotConfig {
    /// Minimum percentage of total samples for a line to be reported.
    pub line_threshold_pct: f64,
    /// Minimum percentage of total samples for a function to be reported.
    pub function_threshold_pct: f64,
    /// Maximum diagnostics generated per file.
    pub max_diagnostics_per_file: usize,
}

impl Default for HotspotConfig {
    fn default() -> Self {
        Self {
            line_threshold_pct: 1.0,
            function_threshold_pct: 2.0,
            max_diagnostics_per_file: 20,
        }
    }
}

impl ProfileData {
    /// Ingest a set of stack traces from a single `get_stack_traces()` call.
    ///
    /// `sample_weight` is `1.0 / sample_rate` (seconds per sample).
    pub fn ingest_traces(
        &mut self,
        traces: &[py_spy::StackTrace],
        sample_weight: f64,
        include_idle: bool,
    ) {
        for trace in traces {
            if !trace.active && !include_idle {
                continue;
            }

            let thread_id = trace.thread_id;

            // Record thread name if available.
            if let Some(ref name) = trace.thread_name {
                let _ = self
                    .thread_names
                    .entry(thread_id)
                    .or_insert_with(|| name.clone());
            }

            // Increment per-thread sample count.
            *self.thread_samples.entry(thread_id).or_insert(0) += 1;

            // Build frame index stack for speedscope (root-first).
            let mut stack_indices = Vec::with_capacity(trace.frames.len());

            for (frame_idx, frame) in trace.frames.iter().enumerate() {
                // Increment line hits.
                *self
                    .line_hits
                    .entry(frame.filename.clone())
                    .or_default()
                    .entry(i32::try_from(frame.line).unwrap_or(0))
                    .or_insert(0) += 1;

                // Increment function stats.
                let func_stats = self
                    .function_stats
                    .entry(frame.filename.clone())
                    .or_default()
                    .entry(frame.name.clone())
                    .or_insert_with(|| FunctionStats {
                        name: frame.name.clone(),
                        file: frame.filename.clone(),
                        line: i32::try_from(frame.line).unwrap_or(0),
                        total_samples: 0,
                        self_samples: 0,
                    });
                func_stats.total_samples += 1;

                // Leaf frame (index 0 in py-spy = top of stack) gets self_samples.
                if frame_idx == 0 {
                    func_stats.self_samples += 1;
                }

                // Deduplicate frame for speedscope.
                let key = FrameKey {
                    name: frame.name.clone(),
                    file: frame.filename.clone(),
                    line: i32::try_from(frame.line).unwrap_or(0),
                };
                let idx = *self.frame_index.entry(key).or_insert_with(|| {
                    let idx = self.frames.len();
                    self.frames.push(SpeedscopeFrame {
                        name: frame.name.clone(),
                        file: frame.filename.clone(),
                        line: i32::try_from(frame.line).unwrap_or(0),
                    });
                    idx
                });
                stack_indices.push(idx);
            }

            // py-spy returns leaf-first; speedscope wants root-first.
            stack_indices.reverse();

            self.thread_stacks
                .entry(thread_id)
                .or_default()
                .push(stack_indices);
            self.thread_weights
                .entry(thread_id)
                .or_default()
                .push(sample_weight);
        }

        self.total_samples += 1;
    }

    /// Return hot lines above the configured threshold, sorted by sample count.
    #[must_use]
    pub fn hot_lines(&self, config: &HotspotConfig) -> Vec<HotLine> {
        if self.total_samples == 0 {
            return Vec::new();
        }

        // Compute total line samples across all files to get proper percentages.
        let total_line_samples: u64 = self
            .line_hits
            .values()
            .flat_map(HashMap::values)
            .sum();

        let mut result = Vec::new();
        for (file, lines) in &self.line_hits {
            let mut file_lines: Vec<HotLine> = lines
                .iter()
                .filter_map(|(&line, &samples)| {
                    let pct = if total_line_samples > 0 {
                        (samples as f64 / total_line_samples as f64) * 100.0
                    } else {
                        0.0
                    };
                    if pct >= config.line_threshold_pct {
                        Some(HotLine {
                            file: file.clone(),
                            line,
                            samples,
                            percentage: pct,
                        })
                    } else {
                        None
                    }
                })
                .collect();
            file_lines.sort_by(|a, b| b.samples.cmp(&a.samples));
            file_lines.truncate(config.max_diagnostics_per_file);
            result.extend(file_lines);
        }
        result.sort_by(|a, b| b.samples.cmp(&a.samples));
        result
    }

    /// Return hot functions above the configured threshold, sorted by total samples.
    #[must_use]
    pub fn hot_functions(&self, config: &HotspotConfig) -> Vec<HotFunction> {
        if self.total_samples == 0 {
            return Vec::new();
        }

        // Compute total function samples across all files.
        let total_func_samples: u64 = self
            .function_stats
            .values()
            .flat_map(HashMap::values)
            .map(|s| s.self_samples)
            .sum();

        let mut result = Vec::new();
        for funcs in self.function_stats.values() {
            for stats in funcs.values() {
                let pct = if total_func_samples > 0 {
                    (stats.total_samples as f64 / total_func_samples as f64) * 100.0
                } else {
                    0.0
                };
                let self_pct = if total_func_samples > 0 {
                    (stats.self_samples as f64 / total_func_samples as f64) * 100.0
                } else {
                    0.0
                };
                if pct >= config.function_threshold_pct {
                    result.push(HotFunction {
                        name: stats.name.clone(),
                        file: stats.file.clone(),
                        line: stats.line,
                        samples: stats.total_samples,
                        percentage: pct,
                        self_samples: stats.self_samples,
                        self_percentage: self_pct,
                    });
                }
            }
        }
        result.sort_by(|a, b| b.samples.cmp(&a.samples));
        result
    }
}

/// A hot line exceeding the threshold.
#[derive(Debug, Clone, Serialize)]
pub struct HotLine {
    /// Source file path.
    pub file: String,
    /// Line number (1-based).
    pub line: i32,
    /// Number of samples hitting this line.
    pub samples: u64,
    /// Percentage of total samples.
    pub percentage: f64,
}

/// A hot function exceeding the threshold.
#[derive(Debug, Clone, Serialize)]
pub struct HotFunction {
    /// Function name.
    pub name: String,
    /// Source file path.
    pub file: String,
    /// Line number of the function definition.
    pub line: i32,
    /// Samples where this function appears in the stack.
    pub samples: u64,
    /// Percentage of total samples.
    pub percentage: f64,
    /// Self samples (function is the leaf).
    pub self_samples: u64,
    /// Self percentage.
    pub self_percentage: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trace(
        thread_id: u64,
        active: bool,
        frames: Vec<(&str, &str, usize)>,
    ) -> py_spy::StackTrace {
        py_spy::StackTrace {
            thread_id,
            thread_name: Some(format!("Thread-{thread_id}")),
            owns_gil: false,
            active,
            frames: frames
                .into_iter()
                .map(|(name, file, line)| py_spy::Frame {
                    name: name.to_owned(),
                    filename: file.to_owned(),
                    line,
                    short_filename: None,
                    module: None,
                    locals: None,
                    is_entry: false,
                })
                .collect(),
            os_thread_id: None,
            process_id: 0,
        }
    }

    #[test]
    fn ingest_single_trace_counts_correctly() {
        let mut data = ProfileData::default();
        let traces = vec![make_trace(
            1,
            true,
            vec![
                ("leaf_fn", "src/a.py", 10),
                ("caller_fn", "src/a.py", 5),
                ("main", "main.py", 1),
            ],
        )];

        data.ingest_traces(&traces, 0.01, false);

        assert_eq!(data.total_samples, 1);
        assert_eq!(
            *data
                .line_hits
                .get("src/a.py")
                .and_then(|m| m.get(&10))
                .unwrap_or(&0),
            1
        );

        let leaf_stats = data
            .function_stats
            .get("src/a.py")
            .and_then(|m| m.get("leaf_fn"));
        assert!(leaf_stats.is_some());
        let leaf = leaf_stats.expect("leaf_fn must exist");
        assert_eq!(leaf.total_samples, 1);
        assert_eq!(leaf.self_samples, 1);

        let caller_stats = data
            .function_stats
            .get("src/a.py")
            .and_then(|m| m.get("caller_fn"));
        assert!(caller_stats.is_some());
        let caller = caller_stats.expect("caller_fn must exist");
        assert_eq!(caller.total_samples, 1);
        assert_eq!(caller.self_samples, 0);
    }

    #[test]
    fn idle_traces_skipped_when_not_included() {
        let mut data = ProfileData::default();
        let traces = vec![make_trace(1, false, vec![("idle_fn", "src/a.py", 10)])];

        data.ingest_traces(&traces, 0.01, false);
        assert_eq!(data.total_samples, 1);
        assert!(data.line_hits.is_empty());
    }

    #[test]
    fn hot_lines_filters_by_threshold() {
        let mut data = ProfileData::default();
        // Simulate 100 samples, 50 on line 10, 1 on line 20.
        for _ in 0..50 {
            let traces = vec![make_trace(1, true, vec![("fn_a", "src/a.py", 10)])];
            data.ingest_traces(&traces, 0.01, false);
        }
        let traces = vec![make_trace(1, true, vec![("fn_b", "src/a.py", 20)])];
        data.ingest_traces(&traces, 0.01, false);

        let config = HotspotConfig {
            line_threshold_pct: 5.0,
            ..HotspotConfig::default()
        };
        let hot = data.hot_lines(&config);
        assert_eq!(hot.len(), 1);
        assert_eq!(hot.first().map(|h| h.line), Some(10));
    }
}
