//! Implements [PROFILE-HELPER-SOCKET]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-HELPER-SOCKET
//!
//! Pure conversion / path / command helpers for the elevated-helper path,
//! kept separate from the socket orchestration in the parent module.

use std::path::PathBuf;
use std::time::SystemTime;

use basilisk_profiler_protocol::TraceData;

/// Build the `osascript` command that runs the helper as administrator.
///
/// The command `cd /`'s first so the elevated shell never inherits a working
/// directory it cannot access — without this, `do shell script ... with
/// administrator privileges` fails with `getcwd: cannot access parent
/// directories` (issue #61, Defect 2). Both paths are single-quoted so spaces
/// survive the shell.
#[must_use]
pub fn build_elevation_script(helper: &str, socket: &str) -> String {
    format!("do shell script \"cd / && '{helper}' '{socket}'\" with administrator privileges")
}

/// Convert wire-format traces back into the `py_spy` shapes the aggregator eats.
pub(super) fn to_pyspy_traces(pid: u32, traces: Vec<TraceData>) -> Vec<py_spy::StackTrace> {
    let spy_pid = i32::try_from(pid).unwrap_or_default();
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

/// Generate a unique, short Unix socket path for one helper session.
///
/// Stays under `/tmp` (not the platform temp dir) because macOS Unix socket
/// paths are limited to ~104 bytes and `/var/folders/...` temp paths overflow.
pub(super) fn create_socket_path(pid: u32) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    PathBuf::from(format!("/tmp/basilisk-profiler-{pid}-{nanos}.sock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevation_script_guards_cwd() {
        let script =
            build_elevation_script("/opt/basilisk/basilisk-profiler-helper", "/tmp/x.sock");
        assert!(
            script.contains("cd / &&"),
            "script must cd to / so it cannot fail on an inaccessible cwd: {script}"
        );
        assert!(
            script.contains("with administrator privileges"),
            "script must request elevation: {script}"
        );
        // The cwd guard must come before the helper invocation.
        let cd_pos = script.find("cd /");
        let helper_pos = script.find("basilisk-profiler-helper");
        assert!(
            matches!((cd_pos, helper_pos), (Some(cd), Some(helper)) if cd < helper),
            "cd / must precede the helper: {script}"
        );
    }

    #[test]
    fn elevation_script_quotes_paths_with_spaces() {
        let script =
            build_elevation_script("/Apps/My Tools/basilisk-profiler-helper", "/tmp/s p.sock");
        assert!(
            script.contains("'/Apps/My Tools/basilisk-profiler-helper'"),
            "helper path must be quoted: {script}"
        );
        assert!(
            script.contains("'/tmp/s p.sock'"),
            "socket path must be quoted: {script}"
        );
    }

    #[test]
    fn socket_path_is_short_and_contains_pid() {
        let path = create_socket_path(12345);
        let display = path.display().to_string();
        assert!(
            display.contains("12345"),
            "path must contain the PID: {display}"
        );
        assert!(
            display.starts_with("/tmp/basilisk-profiler-"),
            "path: {display}"
        );
        assert!(
            path.extension().is_some_and(|ext| ext == "sock"),
            "path must use the .sock extension: {display}"
        );
        assert!(
            display.len() < 104,
            "macOS caps socket paths near 104 bytes: {display}"
        );
    }

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
