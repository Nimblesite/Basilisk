//! Implements [STUBRES-ENGINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
//! Runtime introspection stub generation.
//!
//! Spawns a Python subprocess to `import` the target module and extract
//! function signatures via `inspect.signature()`.  Returns the result as
//! generated `.pyi` content.

use std::fmt::Write as _;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use super::{GeneratedStub, StubGenError, StubGenMode};

/// Default subprocess timeout.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum bytes retained from each subprocess output stream (1 MiB).
///
/// Readers continue draining after reaching the limit so a noisy child cannot
/// block on a full pipe, but excess bytes are discarded instead of retained.
const CAPTURE_LIMIT_BYTES: usize = 1024 * 1024;

/// Avoid busy-waiting while enforcing the subprocess deadline.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Clone, Copy)]
enum PipeKind {
    Stdout,
    Stderr,
}

#[derive(Debug)]
struct CapturedPipe {
    bytes: Vec<u8>,
    truncated: bool,
}

struct BoundedOutput {
    status: ExitStatus,
    stdout: CapturedPipe,
    stderr: CapturedPipe,
}

/// Python script that imports a module and dumps its public API as JSON.
///
/// Output format (one JSON object per line):
/// ```json
/// {"name": "get", "kind": "function", "signature": "(url: str, **kwargs) -> Response", "return": "Response"}
/// {"name": "Session", "kind": "class", "methods": [...]}
/// ```
const INTROSPECT_SCRIPT: &str = r#"
import sys, json, inspect, types, importlib

module_name = sys.argv[1]
try:
    # `importlib.import_module` returns the named (sub)module itself; the bare
    # `__import__(name)` returns the TOP-LEVEL package for a dotted name, so
    # `import_module("pkg.sub")` was introspecting `pkg` — producing an empty or
    # wrong-module stub (GitHub #336). This mirrors FIND_PACKAGE_SOURCE_SCRIPT.
    module = importlib.import_module(module_name)
except Exception as e:
    print(json.dumps({"error": str(e)}))
    sys.exit(1)

def encode_signature(entry, obj):
    try:
        sig = inspect.signature(obj)
    except (ValueError, TypeError):
        entry["params"] = []
        return
    params = []
    for pname, param in sig.parameters.items():
        p = {"name": pname}
        if param.annotation is not inspect.Parameter.empty:
            # formatannotation renders a class object as a type expression
            # (`datetime.datetime`), where str() would produce the repr
            # (`<class 'datetime.datetime'>`) — not valid stub source
            # (GitHub #336).
            p["annotation"] = inspect.formatannotation(param.annotation)
        if param.default is not inspect.Parameter.empty:
            p["has_default"] = True
        if param.kind == inspect.Parameter.VAR_POSITIONAL:
            p["kind"] = "vararg"
        elif param.kind == inspect.Parameter.VAR_KEYWORD:
            p["kind"] = "kwarg"
        elif param.kind == inspect.Parameter.KEYWORD_ONLY:
            p["kind"] = "keyword_only"
        elif param.kind == inspect.Parameter.POSITIONAL_ONLY:
            p["kind"] = "positional_only"
        params.append(p)
    entry["params"] = params
    ret = sig.return_annotation
    if ret is not inspect.Parameter.empty:
        entry["return"] = inspect.formatannotation(ret)

def encode_class_methods(entry, cls):
    methods = []
    for mname, mobj in inspect.getmembers(cls):
        if mname.startswith('_') or not callable(mobj):
            continue
        mentry = {"name": mname}
        encode_signature(mentry, mobj)
        methods.append(mentry)
    entry["methods"] = methods

results = []
for name in sorted(dir(module)):
    if name.startswith('_'):
        continue
    obj = getattr(module, name)
    entry = {"name": name}
    if callable(obj):
        encode_signature(entry, obj)
        if isinstance(obj, type):
            entry["kind"] = "class"
            encode_class_methods(entry, obj)
        else:
            entry["kind"] = "function"
    elif isinstance(obj, types.ModuleType):
        continue
    else:
        entry["kind"] = "variable"
        ann = type(obj).__name__
        entry["annotation"] = ann
    results.append(entry)

print(json.dumps(results))
"#;

/// Generate stubs by running `inspect.signature()` on each public symbol.
///
/// # Errors
///
/// Returns `StubGenError::Subprocess` if the Python process fails or times out.
/// Returns `StubGenError::Import` if the module cannot be imported.
// Implements [STUBRES-AUTOGEN-MODES] "Runtime introspection" — highest
// accuracy: `inspect.signature()` via a Python subprocess sees actual signatures.
pub fn generate_runtime_stubs(
    module_name: &str,
    python_path: &Path,
) -> Result<GeneratedStub, StubGenError> {
    let output = run_introspection(python_path, module_name, TIMEOUT)?;

    if !output.status.success() {
        let mut stderr = String::from_utf8_lossy(&output.stderr.bytes).into_owned();
        if output.stderr.truncated {
            let _ = write!(
                stderr,
                "\n[stderr truncated after {CAPTURE_LIMIT_BYTES} bytes]"
            );
        }
        return Err(StubGenError::Import(format!(
            "Python exited with {}: {stderr}",
            output.status
        )));
    }

    if output.stdout.truncated {
        return Err(StubGenError::Subprocess(format!(
            "stdout exceeded {CAPTURE_LIMIT_BYTES}-byte capture limit"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout.bytes);
    let entries: Vec<serde_json::Value> = serde_json::from_str(stdout.trim())
        .map_err(|err| StubGenError::Subprocess(format!("invalid JSON output: {err}")))?;

    // Check for import error in output.
    if let Some(first) = entries.first() {
        if let Some(error) = first.get("error").and_then(|v| v.as_str()) {
            return Err(StubGenError::Import(error.to_owned()));
        }
    }

    let pyi_content = entries_to_pyi(module_name, &entries);

    // A stub Basilisk's own parser rejects must never be reported as a
    // success — fail loudly instead of writing a contentless lie (GitHub #336).
    if let Err(err) =
        basilisk_parser::parse_source(pyi_content.clone(), format!("{module_name}.pyi"))
    {
        return Err(StubGenError::Subprocess(format!(
            "generated stub for `{module_name}` does not parse: {err}"
        )));
    }

    Ok(GeneratedStub {
        module_name: module_name.to_owned(),
        pyi_content,
        mode: StubGenMode::Runtime,
    })
}

fn run_introspection(
    python_path: &Path,
    module_name: &str,
    timeout: Duration,
) -> Result<BoundedOutput, StubGenError> {
    let mut child = Command::new(python_path)
        .args(["-c", INTROSPECT_SCRIPT, module_name])
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| StubGenError::Subprocess(format!("failed to spawn Python: {err}")))?;

    let Some(stdout) = child.stdout.take() else {
        terminate_and_reap(&mut child);
        return Err(StubGenError::Subprocess(
            "failed to capture Python stdout".to_owned(),
        ));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_and_reap(&mut child);
        return Err(StubGenError::Subprocess(
            "failed to capture Python stderr".to_owned(),
        ));
    };

    let (sender, receiver) = mpsc::channel();
    if let Err(err) = spawn_bounded_reader(stdout, PipeKind::Stdout, sender.clone()) {
        terminate_and_reap(&mut child);
        return Err(StubGenError::Subprocess(format!(
            "failed to start stdout reader: {err}"
        )));
    }
    if let Err(err) = spawn_bounded_reader(stderr, PipeKind::Stderr, sender) {
        terminate_and_reap(&mut child);
        return Err(StubGenError::Subprocess(format!(
            "failed to start stderr reader: {err}"
        )));
    }

    let deadline = Instant::now() + timeout;
    let status = wait_until(&mut child, deadline, timeout)?;
    let (stdout, stderr) = receive_captured_pipes(&receiver, deadline, timeout)?;

    Ok(BoundedOutput {
        status,
        stdout,
        stderr,
    })
}

fn spawn_bounded_reader<R>(
    reader: R,
    kind: PipeKind,
    sender: Sender<(PipeKind, io::Result<CapturedPipe>)>,
) -> io::Result<()>
where
    R: Read + Send + 'static,
{
    thread::Builder::new()
        .name(
            match kind {
                PipeKind::Stdout => "basilisk-stub-stdout",
                PipeKind::Stderr => "basilisk-stub-stderr",
            }
            .to_owned(),
        )
        .spawn(move || {
            let captured = read_bounded(reader);
            let _ = sender.send((kind, captured));
        })
        .map(|_| ())
}

fn read_bounded(mut reader: impl Read) -> io::Result<CapturedPipe> {
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }

        let remaining = CAPTURE_LIMIT_BYTES.saturating_sub(bytes.len());
        let retain = read.min(remaining);
        let retained = buffer
            .get(..retain)
            .ok_or_else(|| io::Error::other("invalid output capture length"))?;
        bytes.extend_from_slice(retained);
        if retain < read {
            truncated = true;
        }
    }

    Ok(CapturedPipe { bytes, truncated })
}

fn wait_until(
    child: &mut Child,
    deadline: Instant,
    timeout: Duration,
) -> Result<ExitStatus, StubGenError> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                terminate_and_reap(child);
                return Err(timeout_error(timeout));
            }
            Ok(None) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                thread::sleep(WAIT_POLL_INTERVAL.min(remaining));
            }
            Err(err) => {
                terminate_and_reap(child);
                return Err(StubGenError::Subprocess(format!(
                    "failed while waiting for Python: {err}"
                )));
            }
        }
    }
}

fn receive_captured_pipes(
    receiver: &Receiver<(PipeKind, io::Result<CapturedPipe>)>,
    deadline: Instant,
    timeout: Duration,
) -> Result<(CapturedPipe, CapturedPipe), StubGenError> {
    let mut stdout = None;
    let mut stderr = None;

    while stdout.is_none() || stderr.is_none() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(timeout_error(timeout));
        }

        match receiver.recv_timeout(remaining) {
            Ok((PipeKind::Stdout, captured)) => {
                stdout = Some(captured.map_err(|err| pipe_error(&err))?);
            }
            Ok((PipeKind::Stderr, captured)) => {
                stderr = Some(captured.map_err(|err| pipe_error(&err))?);
            }
            Err(RecvTimeoutError::Timeout) => return Err(timeout_error(timeout)),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(StubGenError::Subprocess(
                    "Python output reader stopped unexpectedly".to_owned(),
                ));
            }
        }
    }

    match (stdout, stderr) {
        (Some(stdout), Some(stderr)) => Ok((stdout, stderr)),
        _ => Err(StubGenError::Subprocess(
            "Python output capture incomplete".to_owned(),
        )),
    }
}

fn pipe_error(err: &io::Error) -> StubGenError {
    StubGenError::Subprocess(format!("failed reading Python output: {err}"))
}

fn timeout_error(timeout: Duration) -> StubGenError {
    StubGenError::Subprocess(format!("timed out after {} seconds", timeout.as_secs()))
}

fn terminate_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Convert introspection JSON entries to `.pyi` stub content.
fn entries_to_pyi(module_name: &str, entries: &[serde_json::Value]) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "# Auto-generated stub for `{module_name}` (runtime introspection)"
    ));
    lines.push("# Tier 3: best-effort, may be inaccurate".to_owned());
    lines.push(String::new());
    lines.push("from typing import Any".to_owned());
    // A dotted annotation (`datetime.datetime`) references a module the stub
    // must import to be self-consistent (GitHub #336).
    lines.extend(
        collect_annotation_imports(entries)
            .into_iter()
            .map(|module| format!("import {module}")),
    );
    lines.push(String::new());

    for entry in entries {
        let Some(name) = entry.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let kind = entry
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("variable");

        match kind {
            "function" => {
                let sig = format_function_stub(name, entry);
                lines.push(sig);
            }
            "class" => {
                // Emit method bodies so class members survive stub parsing —
                // hover on inherited members needs them (GitHub #287).
                let methods = entry.get("methods").and_then(|v| v.as_array());
                match methods {
                    Some(methods) if !methods.is_empty() => {
                        lines.push(format!("class {name}:"));
                        for method in methods {
                            let Some(mname) = method.get("name").and_then(|v| v.as_str()) else {
                                continue;
                            };
                            lines.push(format!("    {}", format_function_stub(mname, method)));
                        }
                    }
                    _ => lines.push(format!("class {name}: ...")),
                }
            }
            "variable" => {
                let ann = entry
                    .get("annotation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Any");
                lines.push(format!("{name}: {ann}"));
            }
            _ => {}
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

/// Format a single function as a `.pyi` stub line.
fn format_function_stub(name: &str, entry: &serde_json::Value) -> String {
    let mut params = Vec::new();
    if let Some(param_list) = entry.get("params").and_then(|v| v.as_array()) {
        for param in param_list {
            let pname = param.get("name").and_then(|v| v.as_str()).unwrap_or("arg");
            let ann = param.get("annotation").and_then(|v| v.as_str());
            let kind = param.get("kind").and_then(|v| v.as_str());

            let formatted = match kind {
                Some("vararg") => format!("*{pname}"),
                Some("kwarg") => format!("**{pname}"),
                _ => match ann.map(sanitized_annotation) {
                    Some(annotation) => format!("{pname}: {annotation}"),
                    None => pname.to_owned(),
                },
            };
            params.push(formatted);
        }
    }

    let ret = entry
        .get("return")
        .and_then(|v| v.as_str())
        .map_or_else(|| "Any".to_owned(), sanitized_annotation);

    format!("def {name}({}) -> {ret}: ...", params.join(", "))
}

/// An introspected annotation, degraded to `Any` unless it is usable stub
/// source: a single-line string that parses as the annotation of one
/// `_x: <annotation>` statement. Guards against non-expressions such as the
/// `repr()` of a runtime class object — `<class 'str'>` (GitHub #336).
fn sanitized_annotation(raw: &str) -> String {
    let trimmed = raw.trim();
    let single_line = !trimmed.is_empty() && !trimmed.contains(['\n', '\r', ';', '#']);
    if single_line
        && basilisk_parser::parse_source(format!("_x: {trimmed}"), "annotation.pyi".to_owned())
            .is_ok()
    {
        trimmed.to_owned()
    } else {
        "Any".to_owned()
    }
}

/// The modules referenced by dotted annotations across all entries, in stable
/// (sorted) order. Best-effort: only a plain dotted-name chain counts, and the
/// module is everything up to the final segment (`datetime.datetime` →
/// `datetime`).
fn collect_annotation_imports(entries: &[serde_json::Value]) -> std::collections::BTreeSet<String> {
    let mut imports = std::collections::BTreeSet::new();
    for entry in entries {
        collect_entry_imports(entry, &mut imports);
        if let Some(methods) = entry.get("methods").and_then(|v| v.as_array()) {
            for method in methods {
                collect_entry_imports(method, &mut imports);
            }
        }
    }
    imports
}

/// Collect the dotted-annotation modules of one function/method entry.
fn collect_entry_imports(
    entry: &serde_json::Value,
    imports: &mut std::collections::BTreeSet<String>,
) {
    let param_annotations = entry
        .get("params")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|param| param.get("annotation").and_then(|v| v.as_str()));
    let return_annotation = entry.get("return").and_then(|v| v.as_str());
    for annotation in param_annotations.chain(return_annotation) {
        if let Some(module) = dotted_annotation_module(annotation) {
            let _newly_inserted = imports.insert(module);
        }
    }
}

/// The module portion of a plain dotted-name annotation, if any:
/// `datetime.datetime` → `datetime`, `a.b.C` → `a.b`. Generic or otherwise
/// composite annotations return `None` — imports for those stay best-effort.
fn dotted_annotation_module(annotation: &str) -> Option<String> {
    let (module, class_name) = annotation.trim().rsplit_once('.')?;
    let is_identifier = |segment: &str| {
        let mut chars = segment.chars();
        chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && chars.all(|rest| rest.is_ascii_alphanumeric() || rest == '_')
    };
    (module.split('.').all(&is_identifier) && is_identifier(class_name)).then(|| module.to_owned())
}

/// Timeout constant exposed for configuration.
#[must_use]
pub const fn default_timeout() -> Duration {
    TIMEOUT
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only: unwrap/expect acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn fake_python(script_body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("python");
        std::fs::write(&path, format!("#!/bin/sh\n{script_body}\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
        (dir, path)
    }

    #[cfg(unix)]
    #[test]
    fn runtime_introspection_times_out_and_reaps_python() {
        let (_dir, python) = fake_python(
            r#"printf '%s' "$$" > "$0.pid"
exec sleep 11"#,
        );
        let started = std::time::Instant::now();

        let error = generate_runtime_stubs("ignored", &python).unwrap_err();
        let message = error.to_string();

        assert!(
            message.contains("timed out after 10 seconds"),
            "runtime introspection must report the configured timeout, got: {message}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(11),
            "the subprocess must be terminated at the timeout"
        );

        let pid = std::fs::read_to_string(python.with_extension("pid")).unwrap();
        let status = Command::new("ps")
            .args(["-p", pid.trim()])
            .status()
            .unwrap();
        assert!(
            !status.success(),
            "the timed-out Python subprocess must be killed and reaped"
        );
    }

    #[cfg(unix)]
    #[test]
    fn runtime_introspection_rejects_stdout_over_capture_limit() {
        let (_dir, python) =
            fake_python(r"dd if=/dev/zero bs=1048576 count=2 2>/dev/null | tr '\000' x");

        let error = generate_runtime_stubs("ignored", &python).unwrap_err();
        let message = error.to_string();

        assert!(
            message.contains("stdout exceeded 1048576-byte capture limit"),
            "oversized stdout must be rejected with the documented limit"
        );
    }

    #[cfg(unix)]
    #[test]
    fn runtime_introspection_truncates_stderr_at_capture_limit() {
        let (_dir, python) = fake_python(
            r"dd if=/dev/zero bs=1048576 count=2 2>/dev/null | tr '\000' x >&2
exit 7",
        );

        let error = generate_runtime_stubs("ignored", &python).unwrap_err();
        let message = error.to_string();

        assert!(
            message.contains("stderr truncated after 1048576 bytes"),
            "oversized stderr must carry an explicit truncation marker"
        );
        assert!(
            message.len() <= 1_048_576 + 256,
            "the surfaced error must remain bounded, got {} bytes",
            message.len()
        );
    }

    #[test]
    fn entries_to_pyi_produces_valid_stub() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"name": "get", "kind": "function", "params": [{"name": "url", "annotation": "str"}], "return": "Response"},
                {"name": "Session", "kind": "class"},
                {"name": "VERSION", "kind": "variable", "annotation": "str"}
            ]"#,
        )
        .unwrap();

        let pyi = entries_to_pyi("requests", &entries);
        assert!(pyi.contains("def get(url: str) -> Response: ..."));
        assert!(pyi.contains("class Session: ..."));
        assert!(pyi.contains("VERSION: str"));
        assert!(pyi.contains("Auto-generated stub"));
    }

    /// GitHub #287: class methods must survive into the generated stub body so
    /// hover can resolve inherited members — `class X: ...` loses them all.
    #[test]
    fn entries_to_pyi_emits_class_methods() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"name": "BaseModel", "kind": "class", "methods": [
                    {"name": "model_validate", "params": [{"name": "obj", "annotation": "Any"}], "return": "BaseModel"}
                ]}
            ]"#,
        )
        .unwrap();

        let pyi = entries_to_pyi("pydantic", &entries);
        assert!(pyi.contains("class BaseModel:"), "class header: {pyi}");
        assert!(
            pyi.contains("    def model_validate(obj: Any) -> BaseModel: ..."),
            "indented method body: {pyi}"
        );
    }

    #[test]
    fn format_function_stub_with_varargs() {
        let entry: serde_json::Value = serde_json::from_str(
            r#"{"name": "foo", "kind": "function", "params": [{"name": "a"}, {"name": "args", "kind": "vararg"}, {"name": "kwargs", "kind": "kwarg"}], "return": "None"}"#,
        )
        .unwrap();

        let stub = format_function_stub("foo", &entry);
        assert_eq!(stub, "def foo(a, *args, **kwargs) -> None: ...");
    }

    #[cfg(unix)]
    #[test]
    fn generate_runtime_stubs_parses_valid_introspection_output() {
        let (_dir, python) = fake_python(
            r#"printf '%s' '[{"name": "connect", "kind": "function", "params": [{"name": "dsn", "annotation": "str"}], "return": "Conn"}, {"name": "VERSION", "kind": "variable", "annotation": "str"}]'"#,
        );

        let stub = generate_runtime_stubs("db", &python).unwrap();
        assert_eq!(stub.module_name.as_str(), "db");
        assert!(
            matches!(stub.mode, StubGenMode::Runtime),
            "runtime introspection must be tagged as the runtime tier"
        );
        assert!(
            stub.pyi_content
                .contains("def connect(dsn: str) -> Conn: ..."),
            "valid introspection JSON must render function stubs: {}",
            stub.pyi_content
        );
        assert!(
            stub.pyi_content.contains("VERSION: str"),
            "{}",
            stub.pyi_content
        );
    }

    /// GitHub #336 (bugs 1 & 2): runtime introspection must import the named
    /// (sub)module itself, not its top-level package. `__import__("a.b.c")`
    /// returns `a`, whose only members are submodules — an empty/wrong stub.
    /// This runs the REAL introspection script against a real interpreter, so
    /// it depends on `python3` being on PATH (as it is in CI's Rust job).
    #[test]
    fn runtime_introspection_targets_the_dotted_submodule_not_its_root() {
        let python = std::path::PathBuf::from("python3");
        let stub = generate_runtime_stubs("concurrent.futures", &python)
            .expect("python3 must introspect a dotted stdlib submodule");
        assert!(
            stub.pyi_content.contains("ThreadPoolExecutor"),
            "the dotted submodule's own members must be introspected (not the \
             root package's), got:\n{}",
            stub.pyi_content
        );
    }

    #[cfg(unix)]
    #[test]
    fn generate_runtime_stubs_surfaces_import_error_in_output() {
        let (_dir, python) = fake_python(r#"printf '%s' '[{"error": "No module named ghost"}]'"#);

        let error = generate_runtime_stubs("ghost", &python).unwrap_err();
        assert!(
            matches!(error, StubGenError::Import(ref msg) if msg == "No module named ghost"),
            "an error entry in the output JSON must surface as an import error: {error:?}"
        );
    }

    #[test]
    fn entries_to_pyi_skips_unnamed_entries_and_unknown_kinds() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"kind": "function"},
                {"name": "Widget", "kind": "class", "methods": [
                    {"params": []},
                    {"name": "render", "return": "None"}
                ]},
                {"name": "mystery", "kind": "enigma"}
            ]"#,
        )
        .unwrap();

        let pyi = entries_to_pyi("mod", &entries);
        assert!(
            !pyi.contains("mystery"),
            "an unknown kind emits nothing: {pyi}"
        );
        assert!(pyi.contains("class Widget:"), "class renders: {pyi}");
        assert!(
            pyi.contains("    def render() -> None: ..."),
            "the named method survives while the nameless one is skipped: {pyi}"
        );
    }

    #[test]
    fn entries_to_pyi_defaults_missing_annotations_and_returns_to_any() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"name": "noop", "kind": "function"},
                {"name": "config", "kind": "variable"}
            ]"#,
        )
        .unwrap();

        let pyi = entries_to_pyi("mod", &entries);
        assert!(
            pyi.contains("def noop() -> Any: ..."),
            "a function with no params/return defaults to Any: {pyi}"
        );
        assert!(
            pyi.contains("config: Any"),
            "a variable with no annotation defaults to Any: {pyi}"
        );
    }

    /// GitHub #336 follow-up: `repr()` of a runtime class object
    /// (`<class 'str'>`) is not a type expression. Whatever the interpreter
    /// hands back, every annotation must be emitted as parseable source — an
    /// annotation that isn't a valid expression degrades to `Any`, and the
    /// assembled stub must always pass Basilisk's own parser.
    #[test]
    fn entries_to_pyi_sanitizes_repr_style_annotations_to_a_parseable_stub() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"name": "get_host", "kind": "function",
                 "params": [{"name": "when", "annotation": "<class 'datetime.datetime'>"}],
                 "return": "<class 'str'>"}
            ]"#,
        )
        .unwrap();

        let pyi = entries_to_pyi("m", &entries);
        assert!(
            basilisk_parser::parse_source(pyi.clone(), "m.pyi".to_owned()).is_ok(),
            "the generated stub must parse:\n{pyi}"
        );
        assert!(
            !pyi.contains("<class"),
            "repr-style annotations must never reach the stub:\n{pyi}"
        );
        assert!(
            pyi.contains("def get_host(when: Any) -> Any: ..."),
            "unparseable annotations degrade to Any:\n{pyi}"
        );
    }

    /// GitHub #336 follow-up: a dotted annotation like `datetime.datetime`
    /// references a module the stub never imports — the generator must emit
    /// the matching `import` so the stub is self-consistent.
    #[test]
    fn entries_to_pyi_imports_modules_referenced_by_dotted_annotations() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"name": "stamp", "kind": "function",
                 "params": [{"name": "when", "annotation": "datetime.datetime"}],
                 "return": "zoneinfo.ZoneInfo"}
            ]"#,
        )
        .unwrap();

        let pyi = entries_to_pyi("m", &entries);
        assert!(
            pyi.contains("import datetime"),
            "dotted parameter annotations need their module imported:\n{pyi}"
        );
        assert!(
            pyi.contains("import zoneinfo"),
            "dotted return annotations need their module imported:\n{pyi}"
        );
        assert!(
            pyi.contains("def stamp(when: datetime.datetime) -> zoneinfo.ZoneInfo: ..."),
            "valid dotted annotations must be preserved verbatim:\n{pyi}"
        );
    }

    /// GitHub #336 follow-up: the real introspection script must format
    /// runtime class-object annotations as type expressions
    /// (`datetime.datetime`), never `str(cls)` = `<class '...'>`. Exercises
    /// the REAL script against a real interpreter (like the dotted-submodule
    /// test above, this depends on `python3` being on PATH).
    #[cfg(unix)]
    #[test]
    fn runtime_introspection_formats_class_annotations_as_type_expressions() {
        use std::fmt::Write as _;

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("ann_fixture_336.py"),
            "import datetime\n\n\
             def stamp(when: datetime.datetime) -> str:\n    return \"\"\n",
        )
        .unwrap();
        // Wrapper so the fixture dir is importable without mutating this
        // process's environment (parallel tests share it).
        let mut wrapper = String::from("#!/bin/sh\n");
        let _ = writeln!(
            wrapper,
            "PYTHONPATH={} exec python3 \"$@\"",
            dir.path().display()
        );
        let python = dir.path().join("python");
        std::fs::write(&python, wrapper).unwrap();
        let mut permissions = std::fs::metadata(&python).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o700);
        std::fs::set_permissions(&python, permissions).unwrap();

        let stub = generate_runtime_stubs("ann_fixture_336", &python)
            .expect("python3 must introspect the fixture module");
        assert!(
            !stub.pyi_content.contains("<class"),
            "class-object annotations must not be repr()'d:\n{}",
            stub.pyi_content
        );
        assert!(
            stub.pyi_content
                .contains("def stamp(when: datetime.datetime) -> str: ..."),
            "class-object annotations must render as type expressions:\n{}",
            stub.pyi_content
        );
    }

    /// GitHub #336 follow-up: reporting `✓ Generated` for a stub Basilisk's
    /// own parser rejects is a lie. If the assembled content does not parse,
    /// generation must fail loudly instead of succeeding.
    #[cfg(unix)]
    #[test]
    fn generate_runtime_stubs_fails_when_the_assembled_stub_does_not_parse() {
        // A symbol name that cannot form a valid `def` line — sanitizing
        // annotations alone cannot save this stub.
        let (_dir, python) =
            fake_python(r#"printf '%s' '[{"name": "not a name", "kind": "function"}]'"#);

        let error = generate_runtime_stubs("mod", &python).unwrap_err();
        assert!(
            matches!(error, StubGenError::Subprocess(ref msg) if msg.contains("parse")),
            "an unparseable assembled stub must be an error, not a success: {error:?}"
        );
    }

    #[test]
    fn default_timeout_matches_configured_timeout() {
        assert_eq!(default_timeout(), TIMEOUT);
        assert_eq!(default_timeout().as_secs(), 10);
    }

    #[test]
    fn receive_captured_pipes_reports_reader_disconnect() {
        let (sender, receiver) = mpsc::channel::<(PipeKind, io::Result<CapturedPipe>)>();
        drop(sender); // every reader gone before a pipe arrives
        let timeout = Duration::from_secs(5);
        let deadline = Instant::now() + timeout;

        let error = receive_captured_pipes(&receiver, deadline, timeout).unwrap_err();
        assert!(
            matches!(error, StubGenError::Subprocess(ref msg) if msg.contains("stopped unexpectedly")),
            "a disconnected reader must surface a subprocess error: {error:?}"
        );
    }

    #[test]
    fn receive_captured_pipes_times_out_once_the_deadline_passes() {
        // Keep the sender alive so the channel is not disconnected; the elapsed
        // deadline alone must produce the timeout.
        let (_sender, receiver) = mpsc::channel::<(PipeKind, io::Result<CapturedPipe>)>();
        let timeout = Duration::from_millis(5);
        let deadline = Instant::now();

        let error = receive_captured_pipes(&receiver, deadline, timeout).unwrap_err();
        assert!(
            matches!(error, StubGenError::Subprocess(ref msg) if msg.contains("timed out")),
            "an elapsed deadline must surface a timeout: {error:?}"
        );
    }
}
