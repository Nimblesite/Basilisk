//! Implements [PROFILE-PROCESSES-MODEL] and [PROFILE-PROCESSES-LSP].
//! See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-PROCESSES
//!
//! OS process enumeration for the profiler. This is the data source behind the
//! `basilisk.profiler.processes` command and the editor's "Python Processes"
//! panel (#62): instead of asking the user to hand-type a PID, the LSP lists
//! every attachable Python process with the detail an editor needs to render a
//! one-click "Profile" affordance.
//!
//! Per the project's prime directive ("the LSP drives the functionality"),
//! enumeration lives here in Rust so Zed/Neovim can reuse it later. Enumeration
//! only *reads* the process table (via `sysinfo`) and therefore never requires
//! elevation — that is the whole point: discovery must work without `sudo`.
//!
//! Logging discipline ([CLAUDE.md] logging standards): we log the process
//! *count* only. Command lines and user names may contain secrets/PII and are
//! never logged.

use std::collections::HashMap;
use std::ffi::OsString;

use serde::Serialize;
use sysinfo::{get_current_pid, Pid, Process, ProcessesToUpdate, System, Uid, Users};
use tracing::info;

/// Maximum number of distinct interpreter binaries we will invoke with
/// `--version` per enumeration. Bounds the cost of version resolution when many
/// virtualenvs are running; beyond this, versions fall back to the path pattern.
const VERSION_RESOLVE_BUDGET: u32 = 32;

/// Console-script / module launchers that run *on* a Python interpreter. They
/// are surfaced (tagged [`ProcessKind::Launcher`]) so a `uvicorn`/`pytest`
/// process is still offered for profiling rather than hidden.
const LAUNCHERS: &[&str] = &[
    "uvicorn",
    "gunicorn",
    "pytest",
    "celery",
    "flask",
    "hypercorn",
    "daphne",
    "uwsgi",
    "sanic",
];

/// How a Python process presents itself, used by the panel's "group by kind"
/// and to decide whether to show a launcher badge. Implements
/// [PROFILE-PROCESSES-MODEL].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessKind {
    /// A bare interpreter (`python`, `python3.12`, `pypy`).
    Interpreter,
    /// A launcher running on a Python interpreter (`uvicorn`, `pytest`, …).
    Launcher,
}

/// A single attachable Python process. Serialized to the `processes[]` entries
/// of the `basilisk.profiler.processes` response. Implements
/// [PROFILE-PROCESSES-MODEL].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    /// Process id.
    pub pid: u32,
    /// Parent process id (`0` if unknown) — enables "group by parent".
    pub ppid: u32,
    /// Process name, e.g. `python3.12`.
    pub name: String,
    /// Resolved interpreter executable path, if known.
    pub interpreter_path: Option<String>,
    /// Best-effort target script (first positional argument), if any.
    pub script: Option<String>,
    /// Best-effort Python version, e.g. `3.12.13`. `null` ⇒ the editor renders `—`.
    pub python_version: Option<String>,
    /// Instantaneous CPU usage percentage (may exceed 100 across cores).
    pub cpu_percent: f32,
    /// Resident memory in bytes.
    pub memory_bytes: u64,
    /// Seconds elapsed since the process started.
    pub runtime_secs: u64,
    /// Owner login name, if resolvable.
    pub user: Option<String>,
    /// `true` when the process is not owned by the current user, hinting that
    /// attaching the profiler will need elevation (panel shows a lock badge).
    pub requires_elevation: bool,
    /// Whether this is a bare interpreter or a launcher.
    pub kind: ProcessKind,
}

/// Enumerate every running Python process, sorted by CPU usage descending.
///
/// This performs two process refreshes spaced by [`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`]
/// so the reported CPU percentages are meaningful, then resolves interpreter
/// versions (bounded by [`VERSION_RESOLVE_BUDGET`]). It blocks for ~200ms and is
/// intended to be called from a blocking task.
#[must_use]
pub fn enumerate_python_processes() -> Vec<ProcessInfo> {
    let mut system = System::new();
    let _ = system.refresh_processes(ProcessesToUpdate::All, true);
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    let _ = system.refresh_processes(ProcessesToUpdate::All, true);

    let users = Users::new_with_refreshed_list();
    let current_uid = get_current_pid()
        .ok()
        .and_then(|pid| system.process(pid))
        .and_then(Process::user_id);

    let argv_fallback = argv_by_pid();
    let mut cache: HashMap<String, Option<String>> = HashMap::new();
    let mut budget: u32 = VERSION_RESOLVE_BUDGET;

    let mut processes: Vec<ProcessInfo> = system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let context = EnumerationContext {
                users: &users,
                current_uid,
                argv_fallback: &argv_fallback,
            };
            build_process_info(*pid, process, &context, &mut cache, &mut budget)
        })
        .collect();

    processes.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
    info!(count = processes.len(), "enumerated python processes");
    processes
}

/// Shared, read-only inputs for building one [`ProcessInfo`].
struct EnumerationContext<'ctx> {
    users: &'ctx Users,
    current_uid: Option<&'ctx Uid>,
    /// macOS argv fallback (sysinfo cannot read other processes' argv there).
    argv_fallback: &'ctx HashMap<u32, Vec<OsString>>,
}

/// Build a [`ProcessInfo`] for `process`, or `None` if it is not Python.
fn build_process_info(
    pid: Pid,
    process: &Process,
    context: &EnumerationContext<'_>,
    cache: &mut HashMap<String, Option<String>>,
    budget: &mut u32,
) -> Option<ProcessInfo> {
    let users = context.users;
    let current_uid = context.current_uid;
    let name = process.name().to_string_lossy().into_owned();
    let exe = process
        .exe()
        .map(|path| path.to_string_lossy().into_owned());
    // sysinfo returns an empty argv for other processes on macOS; fall back
    // to the batched `ps` snapshot so script labels, launcher detection, and
    // the infrastructure filter work there too.
    let cmd: &[OsString] = if process.cmd().is_empty() {
        context
            .argv_fallback
            .get(&pid.as_u32())
            .map_or(&[], Vec::as_slice)
    } else {
        process.cmd()
    };
    let cmd0 = cmd.first().map(|arg| arg.to_string_lossy().into_owned());

    let is_python = is_python_interpreter(file_basename(&name))
        || exe
            .as_deref()
            .map(file_basename)
            .is_some_and(is_python_interpreter)
        || cmd0
            .as_deref()
            .map(file_basename)
            .is_some_and(is_python_interpreter);
    if !is_python {
        return None;
    }

    let interpreter_path = exe.or(cmd0);
    let script = extract_script(cmd);
    // Debugger machinery is never a profiling target: offering the adapter
    // invites profiling the debugger instead of the debuggee, and adapters
    // orphaned by a hard-killed editor would linger as phantom rows.
    if is_debugger_infrastructure(cmd) {
        return None;
    }
    let kind = classify_kind(cmd, script.as_deref());
    let python_version = interpreter_path
        .as_deref()
        .and_then(|exe_path| resolve_python_version(exe_path, cache, budget));

    let process_uid = process.user_id();
    let user = process_uid
        .and_then(|uid| users.get_user_by_id(uid))
        .map(|owner| owner.name().to_owned());
    let requires_elevation = match (process_uid, current_uid) {
        (Some(owner), Some(current)) => owner != current,
        _ => false,
    };

    Some(ProcessInfo {
        pid: pid.as_u32(),
        ppid: process.parent().map_or(0, Pid::as_u32),
        name,
        interpreter_path,
        script,
        python_version,
        cpu_percent: process.cpu_usage(),
        memory_bytes: process.memory(),
        runtime_secs: process.run_time(),
        user,
        requires_elevation,
        kind,
    })
}

/// Return the final path component of `path`, or `path` itself if it has none.
fn file_basename(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

/// Whether `basename` names a Python (or `PyPy`) interpreter: `python`,
/// `python3`, `pythonX.Y`, `pypy`, with an optional `.exe` suffix.
fn is_python_interpreter(basename: &str) -> bool {
    let trimmed = basename.strip_suffix(".exe").unwrap_or(basename);
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("pypy") {
        return true;
    }
    match lower.strip_prefix("python") {
        Some(rest) => rest.chars().all(|ch| ch.is_ascii_digit() || ch == '.'),
        None => false,
    }
}

/// Best-effort target script: the first positional argument after the
/// interpreter, skipping flags and the values of value-taking flags.
fn extract_script(cmd: &[OsString]) -> Option<String> {
    let mut skip_next = false;
    for arg in cmd.iter().skip(1) {
        let token = arg.to_string_lossy();
        if skip_next {
            skip_next = false;
            continue;
        }
        if token.starts_with('-') {
            if matches!(token.as_ref(), "-c" | "-m" | "-W" | "-X") {
                skip_next = true;
            }
            continue;
        }
        return Some(token.into_owned());
    }
    None
}

/// Classify a Python process as an interpreter or a known launcher, looking at
/// both `-m <module>` invocations and the target script's basename.
fn classify_kind(cmd: &[OsString], script: Option<&str>) -> ProcessKind {
    if let Some(module) = module_arg(cmd) {
        let first_segment = module.split('.').next().unwrap_or(&module);
        if LAUNCHERS.contains(&first_segment) {
            return ProcessKind::Launcher;
        }
    }
    if let Some(path) = script {
        let base = file_basename(path);
        let stem = base.strip_suffix(".py").unwrap_or(base);
        if LAUNCHERS.contains(&stem) {
            return ProcessKind::Launcher;
        }
    }
    ProcessKind::Interpreter
}

/// One batched argv snapshot for the whole process table (macOS only).
///
/// sysinfo cannot read other processes' argv on macOS (`KERN_PROCARGS2` is
/// not exposed through it), so `cmd()` comes back empty and script labels,
/// launcher detection, and the infrastructure filter would all be blind. A
/// single `ps` call per enumeration recovers best-effort argv strings —
/// whitespace-split, so paths containing spaces degrade gracefully to
/// classification-only use.
#[cfg(target_os = "macos")]
fn argv_by_pid() -> HashMap<u32, Vec<OsString>> {
    let Ok(output) = std::process::Command::new("ps")
        .args(["-axo", "pid=,args="])
        .output()
    else {
        return HashMap::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            let pid = tokens.next()?.parse::<u32>().ok()?;
            let argv: Vec<OsString> = tokens.map(OsString::from).collect();
            (!argv.is_empty()).then_some((pid, argv))
        })
        .collect()
}

/// Non-macOS: sysinfo reads argv directly; no fallback needed.
#[cfg(not(target_os = "macos"))]
fn argv_by_pid() -> HashMap<u32, Vec<OsString>> {
    HashMap::new()
}

/// Debugger machinery module prefixes that must never be offered as targets.
const INFRASTRUCTURE_MODULES: &[&str] = &["debugpy", "pydevd"];

/// Whether this Python process is debugger infrastructure rather than a
/// profilable target: `python -m debugpy.adapter`, `-m pydevd`, or a script
/// living inside a `debugpy`/`pydevd` package directory (the launcher).
/// Implements [PROFILE-PROCESSES-MODEL] (exclusions).
fn is_debugger_infrastructure(cmd: &[OsString]) -> bool {
    if let Some(module) = module_arg(cmd) {
        let first_segment = module.split('.').next().unwrap_or(&module);
        if INFRASTRUCTURE_MODULES.contains(&first_segment) {
            return true;
        }
    }
    cmd.iter().skip(1).any(|arg| {
        let token = arg.to_string_lossy();
        INFRASTRUCTURE_MODULES.iter().any(|module| {
            token.contains(&format!("/{module}/")) || token.contains(&format!("\\{module}\\"))
        })
    })
}

/// Extract the module name from a `python -m <module>` invocation, if present.
fn module_arg(cmd: &[OsString]) -> Option<String> {
    let mut expect_module = false;
    for arg in cmd {
        let token = arg.to_string_lossy();
        if expect_module {
            return Some(token.into_owned());
        }
        if token == "-m" {
            expect_module = true;
        }
    }
    None
}

/// Resolve a Python version string for `exe`, caching per executable path.
///
/// Prefers an exact version from `<exe> --version` (e.g. `3.12.13`) within the
/// resolution `budget`, falling back to the `pythonX.Y` path pattern.
fn resolve_python_version(
    exe: &str,
    cache: &mut HashMap<String, Option<String>>,
    budget: &mut u32,
) -> Option<String> {
    if let Some(cached) = cache.get(exe) {
        return cached.clone();
    }

    let resolved = if *budget > 0 {
        *budget -= 1;
        version_via_command(exe).or_else(|| version_from_basename(file_basename(exe)))
    } else {
        version_from_basename(file_basename(exe))
    };

    let _ = cache.insert(exe.to_owned(), resolved.clone());
    resolved
}

/// Run `<exe> --version` and parse the reported version (stdout or stderr).
fn version_via_command(exe: &str) -> Option<String> {
    let output = std::process::Command::new(exe)
        .arg("--version")
        .output()
        .ok()?;
    let stream = if output.stdout.is_empty() {
        output.stderr
    } else {
        output.stdout
    };
    parse_version_output(&String::from_utf8_lossy(&stream))
}

/// Parse `Python 3.12.13` (and `PyPy`'s `Python 3.9.18 [PyPy …]`) into `3.12.13`.
fn parse_version_output(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix("Python ").unwrap_or(trimmed);
    let token = rest.split_whitespace().next()?;
    let starts_with_digit = token.chars().next().is_some_and(|ch| ch.is_ascii_digit());
    if starts_with_digit && token.contains('.') {
        Some(token.to_owned())
    } else {
        None
    }
}

/// Derive `major.minor` from a `pythonX.Y` interpreter basename, if it encodes
/// a version. Returns `None` for unversioned names like `python` or `python3`.
fn version_from_basename(basename: &str) -> Option<String> {
    let trimmed = basename.strip_suffix(".exe").unwrap_or(basename);
    let rest = trimmed.strip_prefix("python")?;
    let mut parts = rest.split('.');
    let major = numeric_segment(parts.next())?;
    let minor = numeric_segment(parts.next())?;
    Some(format!("{major}.{minor}"))
}

/// Return the segment if it is a non-empty run of ASCII digits.
fn numeric_segment(segment: Option<&str>) -> Option<&str> {
    segment.filter(|value| !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_python_interpreter_names() {
        assert!(is_python_interpreter("python"));
        assert!(is_python_interpreter("python3"));
        assert!(is_python_interpreter("python3.12"));
        assert!(is_python_interpreter("python.exe"));
        assert!(is_python_interpreter("pypy3"));
        assert!(!is_python_interpreter("pythonista"));
        assert!(!is_python_interpreter("node"));
        assert!(!is_python_interpreter("ws_features_tests"));
    }

    #[test]
    fn version_from_basename_needs_major_and_minor() {
        assert_eq!(version_from_basename("python3.12"), Some("3.12".to_owned()));
        assert_eq!(version_from_basename("python3"), None);
        assert_eq!(version_from_basename("python"), None);
    }

    #[test]
    fn parses_version_output_from_either_stream() {
        assert_eq!(
            parse_version_output("Python 3.12.13"),
            Some("3.12.13".to_owned())
        );
        assert_eq!(
            parse_version_output("Python 3.9.18\n[PyPy 7.3.13]"),
            Some("3.9.18".to_owned())
        );
        assert_eq!(parse_version_output("garbage"), None);
    }

    #[test]
    fn extracts_script_and_skips_flag_values() {
        let cmd = vec![
            OsString::from("python3"),
            OsString::from("-X"),
            OsString::from("utf8"),
            OsString::from("app.py"),
        ];
        assert_eq!(extract_script(&cmd), Some("app.py".to_owned()));

        let dash_c = vec![
            OsString::from("python3"),
            OsString::from("-c"),
            OsString::from("print(1)"),
        ];
        assert_eq!(extract_script(&dash_c), None);
    }

    #[test]
    fn classifies_launchers_by_module_and_script() {
        let module = vec![
            OsString::from("python3"),
            OsString::from("-m"),
            OsString::from("uvicorn"),
        ];
        assert_eq!(classify_kind(&module, None), ProcessKind::Launcher);

        let script = vec![OsString::from("python3"), OsString::from("/srv/pytest.py")];
        assert_eq!(
            classify_kind(&script, Some("/srv/pytest.py")),
            ProcessKind::Launcher
        );

        let plain = vec![OsString::from("python3"), OsString::from("main.py")];
        assert_eq!(
            classify_kind(&plain, Some("main.py")),
            ProcessKind::Interpreter
        );
    }

    #[test]
    fn enumeration_runs_and_excludes_non_python() {
        // The test binary itself is Rust, not Python, so it must not appear.
        let processes = enumerate_python_processes();
        let own = std::process::id();
        assert!(
            !processes.iter().any(|p| p.pid == own),
            "the non-Python test process must be excluded"
        );
    }
}
