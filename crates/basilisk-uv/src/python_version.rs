//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! `.python-version` file parsing.
//!
//! uv projects typically contain a `.python-version` file at the workspace root
//! that pins the Python version used by the project.

use std::path::Path;
use std::time::{Duration, Instant};

/// Hard ceiling for the one-time interpreter platform probe.
///
/// A valid interpreter answers in milliseconds; this generous ceiling exists
/// only so a genuinely unresponsive interpreter cannot block CLI/LSP startup.
/// It is deliberately well clear of scheduling stalls on a saturated machine:
/// a trivial probe that merely waits for CPU under heavy load must still finish
/// inside the budget, so the ceiling measures interpreter health, not runner
/// load. The timeout is injectable ([`read_python_platform_within`]) so the
/// timeout path is exercised without spending this full budget in tests.
const PLATFORM_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Read `sys.platform` from the interpreter that will execute the project.
///
/// This is target evidence, not a host-platform default: cross-platform
/// analysis can still select `python-platform = "All"`, while an explicitly
/// selected interpreter reports its own concrete runtime platform.
///
/// Reserve this probe for EXPLICITLY selected interpreters
/// (`python-interpreter`, `BASILISK_PYTHON`), which can deliberately point at
/// a shim reporting a non-host target. An auto-discovered interpreter — a
/// workspace venv, a bare `python3` on `PATH` — runs on this host by
/// definition, so its answer is always [`host_sys_platform`]: spawning it buys
/// nothing and costs a full interpreter start-up on every invocation.
/// [STUBRES-TYPESHED-VERSION]
#[must_use]
pub fn read_python_platform(interpreter: &Path) -> Option<String> {
    read_python_platform_within(interpreter, PLATFORM_PROBE_TIMEOUT)
}

/// The host's `sys.platform` value, known at compile time.
///
/// Platform target evidence for projects with no EXPLICITLY selected
/// interpreter: any auto-discovered interpreter (workspace venv, `PATH`
/// fallback) executes on this host, so the constant answers exactly what a
/// probe of it would — without launching anything. Values follow the
/// `sys.platform` vocabulary the stub guards use (`darwin` / `linux` /
/// `win32`). [STUBRES-TYPESHED-VERSION]
#[must_use]
pub fn host_sys_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        // `linux`, `freebsd`, `netbsd`, `aix`, … already match `sys.platform`.
        other => other,
    }
}

/// [`read_python_platform`] with an explicit probe ceiling.
///
/// Splitting the timeout out keeps the production ceiling generous (so honest
/// interpreters never lose a race against runner load) while letting tests
/// drive the timeout path with a short budget instead of the full ceiling.
fn read_python_platform_within(interpreter: &Path, timeout: Duration) -> Option<String> {
    use std::io::Read as _;

    let mut child = std::process::Command::new(interpreter)
        .args(["-I", "-c", "import sys; print(sys.platform)"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) | Err(_) => {
                drop(child.kill());
                drop(child.wait());
                return None;
            }
        }
    };
    if !status.success() {
        return None;
    }

    let mut output = Vec::with_capacity(65);
    let _bytes_read = child
        .stdout
        .take()?
        .take(65)
        .read_to_end(&mut output)
        .ok()?;
    if output.len() > 64 {
        return None;
    }
    let platform = std::str::from_utf8(&output).ok()?.trim();
    (!platform.is_empty()
        && platform.len() <= 64
        && platform
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
    .then(|| platform.to_owned())
}

/// Read the Python version string from a `.python-version` file.
///
/// Returns `None` if the file does not exist or contains no valid version line.
/// A valid line is the first non-empty, non-comment line after stripping
/// whitespace.
pub fn read_python_version(root: &Path) -> Option<String> {
    let path = root.join(".python-version");
    let content = std::fs::read_to_string(&path).ok()?;

    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
}

/// Resolve the project's target Python version from project files, per
/// [LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]:
///
/// 1. `.python-version` file (uv standard)
/// 2. `[project].requires-python` lower bound in `pyproject.toml`
/// 3. `uv.lock` top-level `requires-python` lower bound
///
/// Returns `None` when nothing supplies version evidence; callers preserve an
/// unknown target instead of manufacturing a Python version.
//
// Explicit `python-version` in project config is handled by consumers before
// this metadata cascade. Venv probing (`python3 --version`) is deliberately
// out of scope: resolution reads declared project metadata only, so the target
// stays deterministic across machines and adds no subprocess spawn.
#[must_use]
pub fn resolve_target_python_version(root: &Path) -> Option<String> {
    read_python_version(root)
        .or_else(|| pyproject_requires_python(root))
        .or_else(|| lockfile_requires_python(root))
}

/// Lower bound of `[project].requires-python` in `pyproject.toml`, if any.
fn pyproject_requires_python(root: &Path) -> Option<String> {
    let content = std::fs::read_to_string(root.join("pyproject.toml")).ok()?;
    let table: toml::Table = content.parse().ok()?;
    let requires = table
        .get("project")?
        .as_table()?
        .get("requires-python")?
        .as_str()?;
    constraint_lower_bound(requires)
}

/// Lower bound of `requires-python` in `uv.lock`, if any.
fn lockfile_requires_python(root: &Path) -> Option<String> {
    let lock = crate::lockfile::parse_lock_file(&root.join("uv.lock")).ok()?;
    constraint_lower_bound(lock.requires_python.as_deref()?)
}

/// Extract a version from the `>=`/`==`/`~=` clause of a PEP 440 constraint,
/// e.g. `">=3.10,<4"` → `"3.10"`. `<`-only constraints carry no usable target.
fn constraint_lower_bound(constraint: &str) -> Option<String> {
    constraint.split(',').map(str::trim).find_map(|clause| {
        let version = clause
            .strip_prefix(">=")
            .or_else(|| clause.strip_prefix("=="))
            .or_else(|| clause.strip_prefix("~="))?;
        let version = version.trim().trim_end_matches(".*");
        version
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
            .then(|| version.to_owned())
    })
}

// Tests for [LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]: these cover
// `read_python_version` (the `.python-version` file, cascade step 2). NOTE:
// `resolve_target_python_version` itself — the pyproject/lock fallback chain
// and `constraint_lower_bound` — has no direct test here. See the conformance
// audit for the partial NO-TEST gap.
#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test-only")]
mod tests {
    use super::*;

    #[test]
    fn reads_simple_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "3.12.1\n").unwrap();

        assert_eq!(read_python_version(dir.path()), Some("3.12.1".to_owned()));
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let dir = tempfile::tempdir().unwrap();
        let content = "# managed by uv\n\n  3.11.0  \n";
        std::fs::write(dir.path().join(".python-version"), content).unwrap();

        assert_eq!(read_python_version(dir.path()), Some("3.11.0".to_owned()));
    }

    #[test]
    fn returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_python_version(dir.path()), None);
    }

    #[test]
    fn returns_none_for_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "").unwrap();

        assert_eq!(read_python_version(dir.path()), None);
    }

    #[test]
    fn returns_none_for_comments_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "# 3.12\n# 3.11\n").unwrap();

        assert_eq!(read_python_version(dir.path()), None);
    }

    #[test]
    fn strips_surrounding_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "  3.13.0  ").unwrap();

        assert_eq!(read_python_version(dir.path()), Some("3.13.0".to_owned()));
    }

    #[cfg(unix)]
    #[test]
    fn reads_platform_from_selected_interpreter() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let interpreter = dir.path().join("python");
        std::fs::write(&interpreter, "#!/bin/sh\nprintf 'fixture-platform\\n'\n").unwrap();
        std::fs::set_permissions(&interpreter, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Spawning a *just-written* executable can transiently fail with ETXTBSY
        // when a sibling test thread's concurrent `fork` briefly holds a duplicate
        // of the writable fd across this write->exec window (the workspace runs
        // every crate's `--all-targets` at once, and several e2e tests spawn
        // subprocesses). Real interpreters are pre-existing binaries nobody holds
        // open for writing, so production `read_python_platform` never races this;
        // retry through the transient window so the parse assertion below — the
        // actual subject of this test — stays deterministic. All attempts failing
        // still fails the test, so a genuine parse regression surfaces.
        let mut platform = None;
        for _ in 0..10 {
            platform = read_python_platform(&interpreter);
            if platform.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert_eq!(platform.as_deref(), Some("fixture-platform"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_untrusted_platform_output() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let interpreter = dir.path().join("python");
        std::fs::write(&interpreter, "#!/bin/sh\nprintf 'not a platform\\n'\n").unwrap();
        std::fs::set_permissions(&interpreter, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(read_python_platform(&interpreter), None);
    }

    #[cfg(unix)]
    #[test]
    fn platform_probe_times_out() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let interpreter = dir.path().join("python");
        // Sleeps far longer than the injected budget so the probe can only
        // return by hitting the timeout, never by the interpreter answering.
        std::fs::write(
            &interpreter,
            "#!/bin/sh\nsleep 30\nprintf 'fixture-platform\\n'\n",
        )
        .unwrap();
        std::fs::set_permissions(&interpreter, std::fs::Permissions::from_mode(0o755)).unwrap();
        let started = Instant::now();

        // Inject a short ceiling: the timeout path is what matters here, not the
        // production budget, so this stays fast and load-insensitive.
        assert_eq!(
            read_python_platform_within(&interpreter, Duration::from_millis(200)),
            None
        );
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "an unresponsive configured interpreter must not hang CLI/LSP startup"
        );
    }
}
