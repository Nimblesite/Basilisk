//! Implements [LSPPROF]. See docs/specs/LSP-PROFILING-SPEC.md#LSPPROF
//!
//! Sample aggregation for profiling sessions.
//!
//! Accumulates stack traces from py-spy into per-file, per-line hit counts
//! and per-function statistics. Thread-safe: receives samples via channel,
//! queried from the LSP thread for diagnostics and export.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

/// Accumulated profiling data for a single session.
///
/// Implements [PROFILE-AGGREGATION-STRUCTS] — the `line_hits` / `function_stats`
/// maps, total/per-thread counts, and the deduplicated frame list + per-thread
/// stacks/weights that back the speedscope export.
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
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
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

/// Per-function profiling statistics ([PROFILE-AGGREGATION-STRUCTS]).
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
    // Implements [PROFILE-AGGREGATION-THRESHOLD] — line 1% / function 2% of total
    // samples, capped at 20 diagnostics per file (the spec defaults).
    fn default() -> Self {
        Self {
            line_threshold_pct: 1.0,
            function_threshold_pct: 2.0,
            max_diagnostics_per_file: 20,
        }
    }
}

/// Whether a frame is interpreter/debugger scaffolding rather than code the
/// user (or a library they call) wrote: the runpy/debugpy launcher spine that
/// wraps every debug-launched program, pydevd tracer frames, and the injected
/// cooperative sampler's own frames. Anchored matching only — basename prefix
/// or exact path segment, never a full-path substring — so a user file under
/// `debugpy_utils/` is never mistaken for the debugger (mirrors the memory
/// profiler's `_is_runtime_glue`, [PROFILE-MEMORY-FINAL]).
///
/// `<string>` is matched on the FUNCTION, not the filename: `python -c`,
/// `exec`, `eval`, and the REPL all run legitimate user code with
/// `co_filename == "<string>"`, so only the sampler's `__basilisk*`/`_basilisk*`
/// frames there are scaffolding. Blanket-stripping every `<string>` frame
/// discarded real user samples — a `python -c` workload profiled to zero
/// (#251 follow-up).
fn is_runtime_scaffolding(filename: &str, function: &str) -> bool {
    if filename == "<frozen runpy>" {
        return true;
    }
    if filename == "<string>" {
        return function.starts_with("__basilisk") || function.starts_with("_basilisk");
    }
    let normalized = filename.replace('\\', "/");
    let basename = normalized.rsplit('/').next().unwrap_or(&normalized);
    if basename == "runpy.py"
        || basename.starts_with("pydevd")
        || basename.starts_with("debugpy")
        || basename.starts_with("_pydev")
    {
        return true;
    }
    // The stdlib thread-bootstrap spine. Every non-main thread starts inside
    // `Thread._bootstrap`, so keeping it roots each thread's flame chart three
    // rows above the code the user wrote, and — because the debugger's own
    // housekeeping threads are started by the stdlib too — leaves a
    // debugpy-only thread non-empty, so it survives as pure machinery instead
    // of being dropped. Only the bootstrap trio qualifies: a genuine
    // `threading` wait (`join`, `acquire`) is behaviour the user asked about,
    // and a user's own `run` override lives in the user's file, not here.
    if basename == "threading.py" {
        return matches!(function, "_bootstrap" | "_bootstrap_inner" | "run");
    }
    normalized
        .split('/')
        .any(|segment| segment == "debugpy" || segment == "pydevd")
}

impl ProfileData {
    /// Ingest a set of stack traces from a single `get_stack_traces()` call.
    ///
    /// `sample_weight` is `1.0 / sample_rate` (seconds per sample).
    ///
    /// Implements [PROFILE-AGGREGATION-LOGIC] — per trace: skip idle threads
    /// unless `include_idle`; increment `line_hits` + `total_samples` once per
    /// **distinct** line/function in the stack (#251 — a recursive function
    /// occupies several stack levels of one sample but is still *one* sample,
    /// or its total percentage overflows 100%), `self_samples` for the leaf
    /// (py-spy index 0); record the stack as frame indices (reversed to
    /// root-first for speedscope); then bump `total_samples` once per
    /// `get_stack_traces()` call.
    ///
    /// Implements [PROFILE-AGGREGATION-SCAFFOLD] — runpy/debugpy/pydevd
    /// scaffolding frames are stripped before counting, so every export roots
    /// at the user's code; a leaf tracer frame's overhead is attributed to the
    /// user line it was tracing, and a machinery-only thread (debugger
    /// housekeeping, the injected sampler) is dropped entirely.
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

            // Strip scaffolding before ANY bookkeeping; a thread with nothing
            // left is pure machinery and never registers.
            let kept_frames: Vec<&py_spy::Frame> = trace
                .frames
                .iter()
                .filter(|frame| !is_runtime_scaffolding(&frame.filename, &frame.name))
                .collect();
            if kept_frames.is_empty() {
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
            let mut stack_indices = Vec::with_capacity(kept_frames.len());

            // Inclusive counters are per SAMPLE, not per frame (#251): a
            // recursive function (or its call-site line) appears at several
            // depths of one stack, but that is still one sample — credit each
            // distinct line/function once, or recursion inflates totals past
            // the number of samples and the reported percentage past 100%.
            let mut counted_lines: HashSet<(&str, i32)> = HashSet::new();
            let mut counted_functions: HashSet<(&str, &str)> = HashSet::new();

            for (frame_idx, frame) in kept_frames.into_iter().enumerate() {
                // Increment line hits (once per sample touching the line).
                if counted_lines.insert((frame.filename.as_str(), frame.line)) {
                    *self
                        .line_hits
                        .entry(frame.filename.clone())
                        .or_default()
                        .entry(frame.line)
                        .or_insert(0) += 1;
                }

                // Increment function stats (inclusive total once per sample).
                let newly_counted =
                    counted_functions.insert((frame.filename.as_str(), frame.name.as_str()));
                let func_stats = self
                    .function_stats
                    .entry(frame.filename.clone())
                    .or_default()
                    .entry(frame.name.clone())
                    .or_insert_with(|| FunctionStats {
                        name: frame.name.clone(),
                        file: frame.filename.clone(),
                        line: frame.line,
                        total_samples: 0,
                        self_samples: 0,
                    });
                if newly_counted {
                    func_stats.total_samples += 1;
                }

                // Leaf frame (index 0 in py-spy = top of stack) gets self_samples.
                if frame_idx == 0 {
                    func_stats.self_samples += 1;
                }

                // Deduplicate frame for speedscope.
                let key = FrameKey {
                    name: frame.name.clone(),
                    file: frame.filename.clone(),
                    line: frame.line,
                };
                let idx = *self.frame_index.entry(key).or_insert_with(|| {
                    let idx = self.frames.len();
                    self.frames.push(SpeedscopeFrame {
                        name: frame.name.clone(),
                        file: frame.filename.clone(),
                        line: frame.line,
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

    /// The one denominator every hot-list percentage divides by: the number of
    /// aggregated traces ([PROFILE-AGGREGATION-LOGIC], #251). Equal to
    /// `Σ self_samples` since every kept trace has exactly one leaf, so line
    /// and function percentages are commensurable and bounded by 100%.
    /// `pub(crate)` so diagnostics report "N of M samples" against the same M.
    pub(crate) fn sample_count(&self) -> u64 {
        self.thread_samples.values().sum()
    }

    /// Return hot lines above the configured threshold, sorted by sample count.
    ///
    /// Implements [PROFILE-AGGREGATION-THRESHOLD] (line side): only lines at or
    /// above `line_threshold_pct` of the samples survive, and each file is
    /// truncated to `max_diagnostics_per_file`.
    #[must_use]
    pub fn hot_lines(&self, config: &HotspotConfig) -> Vec<HotLine> {
        if self.total_samples == 0 {
            return Vec::new();
        }

        // Percentage of SAMPLES touching the line ([PROFILE-AGGREGATION-LOGIC]):
        // the same denominator as hot_functions, so the two lists agree (#251).
        let sample_count = self.sample_count();

        let mut result = Vec::new();
        for (file, lines) in &self.line_hits {
            let mut file_lines: Vec<HotLine> = lines
                .iter()
                .filter_map(|(&line, &samples)| {
                    let pct = if sample_count > 0 {
                        pct_of(samples, sample_count)
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
            file_lines.sort_by_key(|line| std::cmp::Reverse(line.samples));
            file_lines.truncate(config.max_diagnostics_per_file);
            result.extend(file_lines);
        }
        result.sort_by_key(|line| std::cmp::Reverse(line.samples));
        result
    }

    /// Return hot functions above the configured threshold, sorted by total samples.
    ///
    /// Implements [PROFILE-AGGREGATION-THRESHOLD] (function side): only functions
    /// at or above `function_threshold_pct` of the samples survive.
    #[must_use]
    pub fn hot_functions(&self, config: &HotspotConfig) -> Vec<HotFunction> {
        if self.total_samples == 0 {
            return Vec::new();
        }

        // Percentage of SAMPLES ([PROFILE-AGGREGATION-LOGIC]): total% is "in how
        // many samples does this function appear anywhere on the stack" and
        // self% "in how many is it the leaf" — both bounded by 100% (#251).
        let sample_count = self.sample_count();

        let mut result = Vec::new();
        for funcs in self.function_stats.values() {
            for stats in funcs.values() {
                let pct = if sample_count > 0 {
                    pct_of(stats.total_samples, sample_count)
                } else {
                    0.0
                };
                let self_pct = if sample_count > 0 {
                    pct_of(stats.self_samples, sample_count)
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
        result.sort_by_key(|func| std::cmp::Reverse(func.samples));
        result
    }
}

/// Calculate the percentage of `part` in `total` (as u64 → f64 safely).
///
/// Uses intermediate `u32` conversion with saturation to avoid `as` casts.
/// For profiling data, `u32::MAX` (4 billion samples) is more than sufficient.
fn pct_of(part: u64, total: u64) -> f64 {
    let part_f = f64::from(u32::try_from(part).unwrap_or(u32::MAX));
    let total_f = f64::from(u32::try_from(total).unwrap_or(u32::MAX));
    (part_f / total_f) * 100.0
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
        frames: Vec<(&str, &str, i32)>,
    ) -> py_spy::StackTrace {
        py_spy::StackTrace {
            pid: 0,
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
                    is_shim_entry: false,
                })
                .collect(),
            os_thread_id: None,
            process_info: None,
        }
    }

    // [PROFILE-AGGREGATION-LOGIC] Leaf gets self_samples, every frame gets
    // total_samples and a line hit.
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

        let leaf = data
            .function_stats
            .get("src/a.py")
            .and_then(|m| m.get("leaf_fn"));
        assert!(leaf.is_some(), "leaf_fn must exist in function_stats");
        if let Some(leaf) = leaf {
            assert_eq!(leaf.total_samples, 1);
            assert_eq!(leaf.self_samples, 1);
        }

        let caller = data
            .function_stats
            .get("src/a.py")
            .and_then(|m| m.get("caller_fn"));
        assert!(caller.is_some(), "caller_fn must exist in function_stats");
        if let Some(caller) = caller {
            assert_eq!(caller.total_samples, 1);
            assert_eq!(caller.self_samples, 0);
        }
    }

    // [PROFILE-AGGREGATION-LOGIC] Idle threads are skipped unless include_idle.
    #[test]
    fn idle_traces_skipped_when_not_included() {
        let mut data = ProfileData::default();
        let traces = vec![make_trace(1, false, vec![("idle_fn", "src/a.py", 10)])];

        data.ingest_traces(&traces, 0.01, false);
        assert_eq!(data.total_samples, 1);
        assert!(data.line_hits.is_empty());
    }

    // [PROFILE-AGGREGATION-LOGIC] Inclusive totals are per SAMPLE, not per
    // frame (#251): a recursive function occupying several stack levels of one
    // sample is counted once, so its reported total percentage is bounded by
    // 100% — never the impossible `build_topo — 1074.1% CPU` of the report.
    // The same dedup applies to line hits (a recursive call-site line must not
    // be over-weighted by recursion depth).
    #[test]
    fn recursive_stack_counts_inclusive_totals_once_per_sample() {
        let mut data = ProfileData::default();
        // ONE sample: build_topo recursing three levels deep (leaf on line 98,
        // two recursive call-sites on line 101), rooted at main.
        let traces = vec![make_trace(
            1,
            true,
            vec![
                ("build_topo", "src/a.py", 98),
                ("build_topo", "src/a.py", 101),
                ("build_topo", "src/a.py", 101),
                ("main", "main.py", 1),
            ],
        )];

        data.ingest_traces(&traces, 0.01, false);

        let stats = data
            .function_stats
            .get("src/a.py")
            .and_then(|m| m.get("build_topo"));
        assert!(stats.is_some(), "build_topo must exist in function_stats");
        if let Some(stats) = stats {
            assert_eq!(
                stats.total_samples, 1,
                "one sample must credit an inclusive total of 1, even with the function at 3 stack depths (#251)"
            );
            assert_eq!(stats.self_samples, 1, "exactly one leaf per sample");
        }

        // The recursive call-site line appears twice in the stack but was hit
        // by only one sample.
        assert_eq!(
            *data
                .line_hits
                .get("src/a.py")
                .and_then(|m| m.get(&101))
                .unwrap_or(&0),
            1,
            "a line hit at several recursion depths of one sample counts once (#251)"
        );

        // And the user-facing number: total CPU % can never exceed 100%.
        let config = HotspotConfig {
            function_threshold_pct: 0.0,
            ..HotspotConfig::default()
        };
        let hot = data.hot_functions(&config);
        let build_topo = hot.iter().find(|f| f.name == "build_topo");
        assert!(build_topo.is_some(), "build_topo must be reported");
        if let Some(func) = build_topo {
            assert!(
                func.percentage <= 100.0,
                "total-time percentage is bounded by 100% by definition; got {}% (#251)",
                func.percentage
            );
        }

        // Line percentages divide by the SAME per-sample denominator as the
        // function list ([PROFILE-AGGREGATION-LOGIC]): the one sample touches
        // line 101, so it reads exactly 100%.
        let hot_lines = data.hot_lines(&HotspotConfig::default());
        let call_site = hot_lines.iter().find(|l| l.line == 101);
        assert!(
            call_site.is_some(),
            "the recursive call-site line must be reported"
        );
        if let Some(line) = call_site {
            let delta = (line.percentage - 100.0).abs();
            assert!(
                delta < f64::EPSILON,
                "1 of 1 samples touch line 101, so its share is 100%; got {}%",
                line.percentage
            );
        }
    }

    // [PROFILE-AGGREGATION-THRESHOLD] Lines below the configured percentage are
    // dropped from the hot-line set.
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
