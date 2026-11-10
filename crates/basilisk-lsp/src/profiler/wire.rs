//! Implements [PROFILE-COOPERATIVE] and [PROFILE-HELPER-SOCKET].
//! See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-COOPERATIVE
//! and docs/specs/LSP-PROFILING-SPEC.md#PROFILE-HELPER-SOCKET
//!
//! Shared wire-format conversion. Both sampling paths that stream samples to
//! the LSP — the cooperative in-process sampler (all platforms) and the
//! elevated helper over a Unix socket (macOS) — emit the same
//! [`basilisk_profiler_protocol::TraceData`] wire shape, so the conversion
//! into the `py_spy` types the aggregator consumes lives here, platform-
//! agnostic, rather than inside the Unix-only helper module.

use basilisk_profiler_protocol::TraceData;

/// Convert wire-format traces back into the `py_spy` shapes the aggregator eats.
/// Shared by the cooperative sampler and the elevated helper, which produce the
/// same wire shape.
pub(crate) fn to_pyspy_traces(pid: u32, traces: Vec<TraceData>) -> Vec<py_spy::StackTrace> {
    // `py_spy::StackTrace.pid` is `remoteprocess::Pid` — `i32` on Unix, `u32`
    // on Windows — so convert our `u32` PID to whichever the platform expects.
    #[cfg(unix)]
    let spy_pid = i32::try_from(pid).unwrap_or_default();
    #[cfg(windows)]
    let spy_pid = pid;
    traces
        .into_iter()
        .map(|trace| py_spy::StackTrace {
            pid: spy_pid,
            thread_id: trace.thread_id,
            thread_name: trace.thread_name,
            owns_gil: trace.owns_gil,
            active: trace.active,
            frames: trace
                .frames
                .into_iter()
                .map(|frame| py_spy::Frame {
                    name: frame.name,
                    filename: frame.filename,
                    line: frame.line,
                    short_filename: None,
                    module: None,
                    locals: None,
                    is_entry: false,
                    is_shim_entry: false,
                })
                .collect(),
            os_thread_id: None,
            process_info: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_wire_traces_to_pyspy() {
        let wire = vec![TraceData {
            thread_id: 9,
            thread_name: Some("worker".to_owned()),
            active: true,
            owns_gil: true,
            frames: vec![wire_frame("hot_function", "/app/x.py", 42)],
        }];
        let converted = to_pyspy_traces(99, wire);
        assert_eq!(converted.len(), 1);
        let frame_names: Vec<&str> = converted
            .iter()
            .flat_map(|trace| trace.frames.iter().map(|frame| frame.name.as_str()))
            .collect();
        assert_eq!(frame_names, vec!["hot_function"]);
        let preserved = converted.iter().any(|trace| {
            trace.thread_id == 9
                && trace.thread_name.as_deref() == Some("worker")
                && trace.active
                && trace.owns_gil
                && trace
                    .frames
                    .iter()
                    .any(|frame| frame.filename == "/app/x.py" && frame.line == 42)
        });
        assert!(preserved, "conversion must preserve every wire field");
    }

    /// Build a wire frame for the conversion test.
    fn wire_frame(name: &str, file: &str, line: i32) -> basilisk_profiler_protocol::FrameData {
        basilisk_profiler_protocol::FrameData {
            name: name.to_owned(),
            filename: file.to_owned(),
            line,
        }
    }
}
