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
use std::path::{Path, PathBuf};

use serde::Serialize;
use sysinfo::{
    get_current_pid, Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, Uid, UpdateKind,
    Users,
};
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

/// Enumerate the running Python processes that belong to `roots`, sorted by CPU
/// usage descending.
///
/// Only processes whose working directory, target script, or interpreter lives
/// inside one of the workspace `roots` are returned — a developer opening their
/// project should never see an unrelated system Python process they did not
/// start ([PROFILE-PROCESSES-SCOPE]). When `roots` is empty (a single-file or
/// no-folder session) there is nothing to scope by, so every Python process is
/// listed, preserving the original system-wide behaviour.
///
/// This performs two process refreshes spaced by [`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`]
/// so the reported CPU percentages are meaningful, then resolves interpreter
/// versions (bounded by [`VERSION_RESOLVE_BUDGET`]). It blocks for ~200ms and is
/// intended to be called from a blocking task.
#[must_use]
pub fn enumerate_python_processes(roots: &[PathBuf]) -> Vec<ProcessInfo> {
    let mut system = System::new();
    // sysinfo's 2-arg `refresh_processes` does NOT request `cmd`, so argv comes
    // back empty on every platform. macOS recovers it via the `ps` fallback
    // below, but Linux/Windows have none — leaving script labels, launcher
    // detection, and the debugger-infrastructure filter blind (a `debugpy.adapter`
    // would be offered as a target). Refresh the same fields the 2-arg default
    // does, plus `cmd` and `cwd`, so argv and the working directory (which powers
    // workspace scoping) are populated wherever sysinfo can read them.
    let refresh_kind = ProcessRefreshKind::nothing()
        .with_memory()
        .with_cpu()
        .with_disk_usage()
        .with_exe(UpdateKind::OnlyIfNotSet)
        .with_cmd(UpdateKind::OnlyIfNotSet)
        .with_cwd(UpdateKind::OnlyIfNotSet)
        .with_tasks();
    let _ = system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    let _ = system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);

    let users = Users::new_with_refreshed_list();
    let current_uid = get_current_pid()
        .ok()
        .and_then(|pid| system.process(pid))
        .and_then(Process::user_id);

    // Canonicalize roots once so symlinked workspace paths (on macOS a temp dir
    // under `/var/...` resolves to `/private/var/...`) compare against the
    // process cwd, which sysinfo already reports in canonical form.
    let normalized_roots: Vec<PathBuf> = roots.iter().map(|root| normalize_path(root)).collect();

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
                roots: &normalized_roots,
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
    /// Canonicalized workspace roots used to scope enumeration. Empty ⇒ no
    /// scoping (every Python process is in-scope). See [PROFILE-PROCESSES-SCOPE].
    roots: &'ctx [PathBuf],
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
    // A debuggee — debugpy running the developer's *own* program (how VS Code's
    // debugger launches a script) — is precisely the process to surface and
    // profile, so detect it first and label the row with the real program
    // rather than debugpy's bootstrap path.
    let debuggee_program = debugpy_debuggee_program(cmd);
    // Debugger machinery is never a profiling target: offering the adapter
    // invites profiling the debugger instead of the debuggee, and adapters
    // orphaned by a hard-killed editor would linger as phantom rows. The
    // debuggee is exempt — its argv references the bundled `debugpy` package
    // (path contains `/debugpy/`) but it is the user's running program.
    if debuggee_program.is_none() && is_debugger_infrastructure(cmd) {
        return None;
    }
    let script = debuggee_program.or_else(|| extract_script(cmd));
    // Workspace scoping: an interpreter the user never started — a background
    // system Python with nothing to do with the open project — must not appear
    // in the panel. Filter before version resolution so out-of-scope processes
    // never cost a `--version` probe ([PROFILE-PROCESSES-SCOPE]).
    if !is_workspace_relevant(
        process.cwd(),
        script.as_deref(),
        interpreter_path.as_deref(),
        context.roots,
    ) {
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
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
}

/// Whether a process whose working directory is `cwd`, target `script`, and
/// `interpreter` belongs to one of the (already canonicalized) workspace
/// `roots`. A process is relevant when any of those paths resolves to a
/// location inside a root. With no roots — a single-file or no-folder session —
/// every process is relevant, preserving the system-wide list rather than
/// showing nothing. Implements [PROFILE-PROCESSES-SCOPE].
fn is_workspace_relevant(
    cwd: Option<&Path>,
    script: Option<&str>,
    interpreter: Option<&str>,
    roots: &[PathBuf],
) -> bool {
    if roots.is_empty() {
        return true;
    }
    workspace_candidate_paths(cwd, script, interpreter)
        .iter()
        .map(|candidate| normalize_path(candidate))
        .any(|candidate| path_in_roots(&candidate, roots))
}

/// The set of paths that tie a process to a workspace: its working directory,
/// its target script, and its interpreter. Relative script/interpreter paths
/// are resolved against `cwd` so `python app.py` launched from the project is
/// attributed to it.
fn workspace_candidate_paths(
    cwd: Option<&Path>,
    script: Option<&str>,
    interpreter: Option<&str>,
) -> Vec<PathBuf> {
    let cwd_candidate = cwd.map(Path::to_path_buf);
    let arg_candidates = [script, interpreter]
        .into_iter()
        .flatten()
        .map(|raw| resolve_against(raw, cwd));
    cwd_candidate.into_iter().chain(arg_candidates).collect()
}

/// Resolve `raw` to an absolute path: absolute paths pass through, relative ones
/// join onto `base` (the process working directory) when it is known.
fn resolve_against(raw: &str, base: Option<&Path>) -> PathBuf {
    let path = Path::new(raw);
    match base {
        Some(base) if path.is_relative() => base.join(path),
        _ => path.to_path_buf(),
    }
}

/// Whether `candidate` is equal to, or nested under, any of `roots`.
fn path_in_roots(candidate: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| candidate.starts_with(root))
}

/// Best-effort canonicalization so symlinked roots and `.`/`..` segments compare
/// correctly. Falls back to the input when the path cannot be resolved (e.g. a
/// script that has since been deleted).
fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
/// profilable target: `python -m debugpy.adapter`, `-m pydevd`, or the
/// debugpy/pydevd launcher/adapter living inside the package directory.
/// Implements [PROFILE-PROCESSES-MODEL] (exclusions).
///
/// The caller exempts a *debuggee* ([`debugpy_debuggee_program`]) first, so the
/// user's own program running under the bundled debugpy — whose path also
/// contains `/debugpy/` — is surfaced rather than swallowed by the path check.
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

/// Flags in a debugpy debuggee invocation that consume the following argument,
/// so the user's program is never mistaken for one of their values (e.g. the
/// `127.0.0.1:5679` after `--connect`). Mirrors debugpy's launcher cmdline.
const DEBUGPY_VALUE_FLAGS: &[&str] = &[
    "--connect",
    "--listen",
    "--host",
    "--port",
    "--adapter-access-token",
];

/// If `cmd` runs the developer's own program *under* debugpy
/// (`python <…>/debugpy --connect <addr> … <program>`, the exact shape VS Code's
/// debugger launches), return that program. Such a process is a real debuggee —
/// the thing the user wants to see and profile — not debugger machinery, even
/// though its argv references the bundled `debugpy` package (whose path contains
/// `/debugpy/`). Implements [PROFILE-PROCESSES-MODEL] (debuggee surfacing).
fn debugpy_debuggee_program(cmd: &[OsString]) -> Option<String> {
    let entry = debugpy_entry_index(cmd)?;
    let mut skip_next = false;
    for arg in cmd.iter().skip(entry + 1) {
        let token = arg.to_string_lossy();
        if skip_next {
            skip_next = false;
            continue;
        }
        if token.starts_with('-') {
            if DEBUGPY_VALUE_FLAGS.contains(&token.as_ref()) || token.starts_with("--configure-") {
                skip_next = true;
            }
            continue;
        }
        return Some(token.into_owned());
    }
    None
}

/// Index of the `debugpy` package entry in `cmd`: the `<…>/debugpy` directory run
/// directly (basename `debugpy`) or `-m debugpy`. The `launcher`/`adapter`
/// submodules have a different basename and are deliberately *not* matched —
/// those are machinery, caught by [`is_debugger_infrastructure`].
fn debugpy_entry_index(cmd: &[OsString]) -> Option<usize> {
    let mut expect_module = false;
    for (index, arg) in cmd.iter().enumerate().skip(1) {
        let token = arg.to_string_lossy();
        if expect_module {
            return (token == "debugpy").then_some(index);
        }
        if token == "-m" {
            expect_module = true;
        } else if file_basename(&token) == "debugpy" {
            return Some(index);
        }
    }
    None
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
    fn surfaces_debugpy_debuggee_running_user_program() {
        // The exact argv VS Code's debugger produces with the *bundled* debugpy:
        // the package nests at `.../debugpy/debugpy`, so the path contains
        // `/debugpy/` — the over-broad infra filter used to hide the user's own
        // running script. It must now be surfaced and labelled with the program.
        let cmd = vec![
            OsString::from("/usr/bin/python3"),
            OsString::from("/ext/bundled/debugpy/debugpy"),
            OsString::from("--connect"),
            OsString::from("127.0.0.1:5679"),
            OsString::from("--adapter-access-token"),
            OsString::from("deadbeef"),
            OsString::from("/workspace/cpu_demo.py"),
        ];
        assert_eq!(
            debugpy_debuggee_program(&cmd),
            Some("/workspace/cpu_demo.py".to_owned()),
            "the debuggee's user program must be recovered past debugpy's flags"
        );
        assert!(
            !is_debugger_infrastructure(&cmd) || debugpy_debuggee_program(&cmd).is_some(),
            "a debuggee running the user's program must not be hidden as infrastructure"
        );
    }

    #[test]
    fn debuggee_program_handles_listen_and_module_forms() {
        // `--listen` server form (basename `debugpy`).
        let listen = vec![
            OsString::from("python"),
            OsString::from("/ext/bundled/debugpy/debugpy"),
            OsString::from("--listen"),
            OsString::from("127.0.0.1:0"),
            OsString::from("--wait-for-client"),
            OsString::from("/ws/app.py"),
        ];
        assert_eq!(debugpy_debuggee_program(&listen), Some("/ws/app.py".to_owned()));

        // `-m debugpy` module form.
        let module = vec![
            OsString::from("python"),
            OsString::from("-m"),
            OsString::from("debugpy"),
            OsString::from("--connect"),
            OsString::from("127.0.0.1:1"),
            OsString::from("/ws/main.py"),
        ];
        assert_eq!(debugpy_debuggee_program(&module), Some("/ws/main.py".to_owned()));
    }

    #[test]
    fn launcher_and_adapter_remain_infrastructure() {
        // The launcher (`.../debugpy/launcher`) carries no user program and must
        // stay hidden.
        let launcher = vec![
            OsString::from("python"),
            OsString::from("/ext/bundled/debugpy/debugpy/launcher"),
            OsString::from("53412"),
        ];
        assert_eq!(debugpy_debuggee_program(&launcher), None);
        assert!(is_debugger_infrastructure(&launcher));

        // The adapter basilisk itself spawns.
        let adapter = vec![
            OsString::from("python"),
            OsString::from("-m"),
            OsString::from("debugpy.adapter"),
            OsString::from("--port"),
            OsString::from("0"),
        ];
        assert_eq!(debugpy_debuggee_program(&adapter), None);
        assert!(is_debugger_infrastructure(&adapter));
    }

    #[test]
    fn enumeration_runs_and_excludes_non_python() {
        // The test binary itself is Rust, not Python, so it must not appear.
        // No roots ⇒ no workspace scoping (the system-wide list).
        let processes = enumerate_python_processes(&[]);
        let own = std::process::id();
        assert!(
            !processes.iter().any(|p| p.pid == own),
            "the non-Python test process must be excluded"
        );
    }

    #[test]
    fn no_roots_means_every_process_is_relevant() {
        // With no workspace open there is nothing to scope by, so scoping is a
        // no-op rather than an empty panel ([PROFILE-PROCESSES-SCOPE]).
        assert!(is_workspace_relevant(
            Some(Path::new("/anywhere/at/all")),
            Some("/anywhere/at/all/app.py"),
            Some("/usr/bin/python3"),
            &[],
        ));
    }

    #[test]
    fn cwd_inside_a_root_is_relevant() {
        let roots = vec![PathBuf::from("/home/dev/project")];
        assert!(is_workspace_relevant(
            Some(Path::new("/home/dev/project/sub")),
            None,
            None,
            &roots,
        ));
        // A sibling that merely shares a name prefix is NOT inside the root.
        assert!(!is_workspace_relevant(
            Some(Path::new("/home/dev/project-other")),
            None,
            None,
            &roots,
        ));
    }

    #[test]
    fn unrelated_process_outside_every_root_is_filtered() {
        let roots = vec![PathBuf::from("/home/dev/project")];
        assert!(!is_workspace_relevant(
            Some(Path::new("/")),
            None,
            Some("/usr/bin/python3"),
            &roots,
        ));
    }

    #[test]
    fn absolute_script_inside_a_root_is_relevant_even_when_cwd_is_not() {
        let roots = vec![PathBuf::from("/home/dev/project")];
        assert!(is_workspace_relevant(
            Some(Path::new("/home/dev")),
            Some("/home/dev/project/app.py"),
            None,
            &roots,
        ));
    }

    #[test]
    fn relative_script_resolves_against_cwd() {
        let roots = vec![PathBuf::from("/home/dev/project")];
        // `python app.py` launched from the project dir: argv[0]'s script is
        // relative and only lands in the root once joined to the cwd.
        assert!(is_workspace_relevant(
            Some(Path::new("/home/dev/project")),
            Some("app.py"),
            None,
            &roots,
        ));
    }

    #[test]
    fn candidate_paths_resolve_relative_args_against_cwd() {
        let cwd = Path::new("/home/dev/project");
        let candidates =
            workspace_candidate_paths(Some(cwd), Some("app.py"), Some("/usr/bin/python3"));
        assert!(candidates.contains(&PathBuf::from("/home/dev/project")));
        assert!(candidates.contains(&PathBuf::from("/home/dev/project/app.py")));
        assert!(candidates.contains(&PathBuf::from("/usr/bin/python3")));
    }
}
