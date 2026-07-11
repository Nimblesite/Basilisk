//! Implements [PROFILE-HELPER-SOCKET]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-HELPER-SOCKET
//!
//! Elevated-helper launch helpers — the `osascript` elevation command and the
//! Unix socket path the helper connects back on — kept separate from the
//! socket orchestration in the parent module.

use std::path::PathBuf;
use std::time::SystemTime;

/// Build the `osascript` command that runs the helper as administrator.
///
/// The command `cd /`'s first so the elevated shell never inherits a working
/// directory it cannot access — without this, `do shell script ... with
/// administrator privileges` fails with `getcwd: cannot access parent
/// directories` (issue #61, Defect 2). Both paths are single-quoted so spaces
/// survive the shell.
#[must_use]
pub fn build_elevation_script(helper: &str, socket: &str) -> String {
    let helper = escape_elevated_argument(helper);
    let socket = escape_elevated_argument(socket);
    format!("do shell script \"cd / && '{helper}' '{socket}'\" with administrator privileges")
}

/// Quote one shell argument embedded inside an `AppleScript` string literal.
///
/// The argument sits in two nested layers: it is wrapped in shell single quotes
/// inside a command that is itself an `AppleScript` double-quoted string. Escape
/// the INNER shell layer first — a literal `'` closes the quote, emits an escaped
/// `\'`, and reopens (`'\''`) — THEN the OUTER `AppleScript` layer, so the
/// backslash the shell escape introduces is itself doubled for `AppleScript`.
/// Doing the `AppleScript` backslash pass first (as before) left a lone `\'`,
/// which `osascript` rejects as an unknown escape token.
fn escape_elevated_argument(argument: &str) -> String {
    argument
        .replace('\'', "'\\''")
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
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
    fn elevation_script_escapes_shell_and_applescript_metacharacters() {
        let script =
            build_elevation_script("/Apps/O'Brien/\"Basilisk\"\\helper", "/tmp/a'b\"c\\d.sock");
        // The shell single-quote escape `'\''` has its backslash doubled for the
        // outer AppleScript string layer, so the embedded form is `'\\''`. A lone
        // `\'` (the previous, un-doubled output) is an invalid AppleScript escape
        // that osascript rejects with "unknown token".
        assert!(
            script.contains(r"O'\\''Brien"),
            "shell single quotes must be escaped and the backslash doubled for AppleScript: {script}"
        );
        assert!(
            script.contains(r#"\"Basilisk\""#),
            "AppleScript double quotes must be escaped: {script}"
        );
        assert!(
            script.contains(r"\\helper"),
            "AppleScript backslashes must be escaped: {script}"
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
}
