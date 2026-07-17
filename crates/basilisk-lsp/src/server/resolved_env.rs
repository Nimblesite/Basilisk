//! Implements [LSPARCH-RESOLVED-ENV]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-RESOLVED-ENV
//!
//! Resolves the environment the server actually uses — the Python interpreter
//! ([LSPDEBUG-PYRES]), the uv binary ([LSPARCH-UV-BINRES]), and the running
//! server binary itself — and packages it as the
//! `experimental.basilisk.resolvedEnvironment` payload of the `initialize`
//! response. Editors render these resolved values in their Server Info UIs so
//! `auto-detect` is observable instead of a bare placeholder (GitHub #153).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tracing::debug;

use crate::profiler::processes::version_via_command;

/// One resolved tool: where it lives, and the version its `--version` reports
/// (`None` when the probe fails — the path is still worth showing).
struct ResolvedTool {
    path: PathBuf,
    version: Option<String>,
}

/// Build the `experimental` capabilities payload for the `initialize` response
/// by MERGING `resolvedEnvironment` into `base` — every capability already
/// advertised there (e.g. `basilisk.configurationEditor`,
/// [LSPARCH-CONFIG-EDITOR-PROTOCOL]) is preserved, never replaced.
///
/// Shape: `{"basilisk": {…existing keys…, "resolvedEnvironment": {"python": …,
/// "uv": …, "binary": …}}}` where each tool is `{"path", "version"}` or `null`
/// when nothing usable was found — clients surface that as an explicit
/// "none found", never a silent placeholder.
pub(super) fn experimental_payload(
    base: Option<Value>,
    init_options: Option<&Value>,
    roots: &[PathBuf],
) -> Value {
    let mut payload = match base {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    let basilisk = payload.entry("basilisk").or_insert_with(|| json!({}));
    if let Some(map) = basilisk.as_object_mut() {
        drop(map.insert(
            "resolvedEnvironment".to_owned(),
            resolved_environment(init_options, roots),
        ));
    }
    Value::Object(payload)
}

/// Resolve the python/uv/binary triple, honouring the editor's configured
/// overrides from `initializationOptions` (empty strings mean auto-detect,
/// matching the editor settings' defaults).
fn resolved_environment(init_options: Option<&Value>, roots: &[PathBuf]) -> Value {
    let python_override = string_option(init_options, &["basilisk", "python"])
        .or_else(|| string_option(init_options, &["python"]));
    let uv_override = string_option(init_options, &["uv", "executablePath"])
        .or_else(|| string_option(init_options, &["basilisk", "uv", "executablePath"]));

    let payload = json!({
        "python": tool_value(resolve_python_tool(python_override.as_deref(), roots)),
        "uv": tool_value(resolve_uv_tool(uv_override.as_deref())),
        "binary": tool_value(running_binary()),
    });
    debug!(resolved_environment = %payload, "resolved environment for initialize response");
    payload
}

/// Read a non-empty string at `path` inside the init options, `None` when the
/// key is absent, not a string, or blank (the editor sends `""` for unset).
fn string_option(value: Option<&Value>, path: &[&str]) -> Option<String> {
    let mut cursor = value?;
    for key in path {
        cursor = cursor.get(key)?;
    }
    cursor
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// The interpreter the server would launch ([LSPDEBUG-PYRES] cascade via
/// [`crate::debug::effective_python`]), located on disk and version-probed.
fn resolve_python_tool(override_path: Option<&str>, roots: &[PathBuf]) -> Option<ResolvedTool> {
    let workspace = roots
        .first()
        .map_or_else(|| PathBuf::from("."), Clone::clone);
    let candidate = crate::debug::effective_python(override_path, &workspace);
    locate_executable(&candidate).map(probe)
}

/// The uv binary the [LSPARCH-UV-BINRES] cascade resolves, version-probed.
fn resolve_uv_tool(override_path: Option<&str>) -> Option<ResolvedTool> {
    basilisk_uv::find_uv_binary(override_path).map(probe)
}

/// The server binary answering this very request — `current_exe` is
/// authoritative for "which basilisk is actually running" (the blank Binary
/// row of GitHub #153 came from rendering the raw editor setting instead).
fn running_binary() -> Option<ResolvedTool> {
    std::env::current_exe().ok().map(|path| ResolvedTool {
        path,
        version: Some(env!("CARGO_PKG_VERSION").to_owned()),
    })
}

/// Probe `<path> --version` for a display version; keep the path either way.
fn probe(path: PathBuf) -> ResolvedTool {
    let version = version_via_command(&path.to_string_lossy());
    ResolvedTool { path, version }
}

/// JSON for one tool slot: `{"path", "version"}` or `null`.
fn tool_value(tool: Option<ResolvedTool>) -> Value {
    tool.map_or(
        Value::Null,
        |tool| json!({ "path": tool.path.to_string_lossy(), "version": tool.version }),
    )
}

/// A candidate containing a path separator must exist as given; a bare
/// command name (e.g. the `python3` system fallback) is searched on `PATH`.
fn locate_executable(candidate: &str) -> Option<PathBuf> {
    let as_path = Path::new(candidate);
    if candidate.contains(std::path::MAIN_SEPARATOR) || candidate.contains('/') {
        return as_path.is_file().then(|| as_path.to_path_buf());
    }
    find_in_path(candidate)
}

/// First `PATH` directory holding `name` (with the `.exe` suffix on Windows).
fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var).find_map(|dir| {
        let bare = dir.join(name);
        if bare.is_file() {
            return Some(bare);
        }
        if cfg!(windows) {
            let with_exe = dir.join(format!("{name}.exe"));
            if with_exe.is_file() {
                return Some(with_exe);
            }
        }
        None
    })
}

// Tests for [LSPARCH-RESOLVED-ENV] — payload shape, override parsing, and
// executable location. Cross-references server/resolved_env.rs (this file)
// and the initialize wiring in server/init.rs.
#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    #[test]
    fn string_option_reads_nested_and_flat_shapes() {
        let options = json!({ "basilisk": { "python": "/usr/bin/python3" }, "uv": { "executablePath": "  " } });
        assert_eq!(
            string_option(Some(&options), &["basilisk", "python"]).as_deref(),
            Some("/usr/bin/python3")
        );
        // Blank means "unset" — the editor sends "" for auto-detect defaults.
        assert_eq!(
            string_option(Some(&options), &["uv", "executablePath"]),
            None
        );
        assert_eq!(string_option(Some(&options), &["missing"]), None);
        assert_eq!(string_option(None, &["basilisk", "python"]), None);
    }

    #[test]
    fn merging_preserves_existing_experimental_capabilities() {
        // Regression: the resolved-environment payload once REPLACED the whole
        // `experimental` object, silently dropping `configurationEditor`
        // ([LSPARCH-CONFIG-EDITOR-PROTOCOL]) and un-gating the editor command.
        let base = json!({ "basilisk": { "configurationEditor": true } });
        let payload = experimental_payload(Some(base), None, &[]);
        assert_eq!(
            payload.pointer("/basilisk/configurationEditor"),
            Some(&Value::Bool(true)),
            "resolvedEnvironment must merge alongside configurationEditor, never replace it: {payload}"
        );
        assert!(
            payload.pointer("/basilisk/resolvedEnvironment").is_some(),
            "merged payload must still carry resolvedEnvironment: {payload}"
        );
    }

    #[test]
    fn payload_always_carries_all_three_slots_and_the_running_binary() {
        let payload = experimental_payload(None, None, &[]);
        let env = payload
            .get("basilisk")
            .and_then(|b| b.get("resolvedEnvironment"))
            .expect("payload must nest basilisk.resolvedEnvironment");
        for slot in ["python", "uv", "binary"] {
            assert!(env.get(slot).is_some(), "missing `{slot}` slot: {env}");
        }
        // The binary slot reports the test executable itself + crate version.
        let binary = env.get("binary").expect("binary slot");
        assert_eq!(
            binary.get("version").and_then(Value::as_str),
            Some(env!("CARGO_PKG_VERSION")),
            "binary version must be the running server's own version"
        );
        let path = binary.get("path").and_then(Value::as_str).expect("path");
        assert!(
            Path::new(path).is_absolute(),
            "current_exe must be absolute, got {path}"
        );
    }

    #[test]
    fn nonexistent_python_override_resolves_to_null() {
        let options = json!({ "basilisk": { "python": "/definitely/not/here/python" } });
        let payload = experimental_payload(None, Some(&options), &[]);
        let python = payload
            .pointer("/basilisk/resolvedEnvironment/python")
            .expect("python slot");
        assert!(
            python.is_null(),
            "an explicit interpreter that does not exist must surface as null (→ \"none found\"), got {python}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn explicit_python_override_is_reported_with_its_probed_version() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("bsk_renv_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("python");
        std::fs::write(&fake, "#!/bin/sh\necho 'Python 9.9.9'\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let options = json!({ "basilisk": { "python": fake.to_string_lossy() } });
        let payload = experimental_payload(None, Some(&options), &[]);
        let python = payload
            .pointer("/basilisk/resolvedEnvironment/python")
            .expect("python slot");
        assert_eq!(
            python.get("path").and_then(Value::as_str),
            Some(fake.to_string_lossy().as_ref())
        );
        assert_eq!(python.get("version").and_then(Value::as_str), Some("9.9.9"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn bare_command_names_are_searched_on_path() {
        // `sh` is guaranteed on any Unix PATH; a made-up name is nowhere.
        let found = locate_executable("sh").expect("sh must be on PATH");
        assert!(found.is_absolute());
        assert_eq!(locate_executable("bsk-no-such-tool-153"), None);
    }

    #[test]
    fn version_probe_failure_keeps_the_path() {
        let dir = std::env::temp_dir().join(format!("bsk_renv_probe_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Present on disk but not executable: the probe fails, the path stays.
        let dud = dir.join("uv");
        std::fs::write(&dud, "not a binary").unwrap();

        let tool = probe(dud.clone());
        assert_eq!(tool.path, dud);
        assert_eq!(tool.version, None);
        let rendered = tool_value(Some(tool));
        assert_eq!(
            rendered.get("version"),
            Some(&Value::Null),
            "a failed probe serialises version as null"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
