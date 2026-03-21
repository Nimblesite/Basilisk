//! Pure logic extracted from the Zed extension glue layer.
//!
//! **Zero `zed_extension_api` imports.** Every function here takes and returns
//! only `serde_json::Value`, `String`, `&str`, or basic Rust types so the
//! module compiles and tests on any native target — no WASM host required.

use basilisk_common::{config_keys, slash_commands};
use serde_json::Value;

// ── Slash commands ───────────────────────────────────────────────────────────

/// Produce the (label, text) pair for a slash command invocation.
///
/// Output is formatted as Markdown for the Zed AI assistant panel.
/// Returns `Err` for unknown command names.
pub fn slash_command_output(command: &str, args: &[String]) -> Result<(String, String), String> {
    match command {
        slash_commands::PROFILE => {
            let target = match args.first() {
                Some(pid) => format!("PID `{pid}`"),
                None => "active Python process".to_string(),
            };
            let text = format!(
                "## Profiling Started\n\n\
                 **Target:** {target}\n\n\
                 Collecting CPU samples via `py-spy`. \
                 Use `/profstop` to stop and view results, \
                 or `/profsnapshot` for a snapshot without stopping.\n\n\
                 Results will appear as:\n\
                 - **LSP diagnostics** — per-line timing hints in the editor\n\
                 - **Speedscope JSON** — opened in browser for flamegraph view"
            );
            Ok(("Profile Started".to_string(), text))
        }
        slash_commands::PROFSTOP => {
            let text = "\
                ## Profile Results\n\n\
                Profiling stopped. Results sent via LSP diagnostics.\n\n\
                | Metric | Value |\n\
                |--------|-------|\n\
                | Status | Stopped |\n\
                | Output | LSP hint diagnostics + speedscope JSON |\n\n\
                > Hot functions and per-line timing are visible as editor hints.\n\
                > Open the speedscope file in your browser for a flamegraph."
                .to_string();
            Ok(("Profile Results".to_string(), text))
        }
        slash_commands::PROFSNAPSHOT => {
            let text = "\
                ## Profile Snapshot\n\n\
                Snapshot captured. **Profiling continues.**\n\n\
                Results sent via LSP diagnostics. \
                Use `/profstop` to stop, or `/profsnapshot` again for another snapshot."
                .to_string();
            Ok(("Profile Snapshot".to_string(), text))
        }
        slash_commands::MEMLEAK => {
            let text = "\
                ## Memory Tracking Started\n\n\
                Tracking object allocations via debug session.\n\n\
                Use `/memstop` to stop and generate a leak report, \
                or `/memrefs <TypeName>` to query retention paths for a specific type."
                .to_string();
            Ok(("Memory Tracking".to_string(), text))
        }
        slash_commands::MEMSTOP => {
            let text = "\
                ## Memory Leak Report\n\n\
                Memory tracking stopped. Leak report sent via LSP diagnostics.\n\n\
                | Metric | Value |\n\
                |--------|-------|\n\
                | Status | Stopped |\n\
                | Output | LSP diagnostics with confidence scores |\n\n\
                > Use `/memrefs <TypeName>` to inspect retention paths for specific types."
                .to_string();
            Ok(("Memory Report".to_string(), text))
        }
        slash_commands::MEMREFS => {
            let type_name = args.first().map_or("(unknown)", String::as_str);
            let text = format!(
                "## Reference Graph: `{type_name}`\n\n\
                 Querying retention paths for `{type_name}`...\n\n\
                 Results will show the reference chain from GC roots \
                 to instances of `{type_name}`, helping identify why objects are not collected."
            );
            Ok(("Reference Graph".to_string(), text))
        }
        _ => Err(format!("Unknown slash command: {command}")),
    }
}

/// Return completion suggestions for a slash command as `(label, new_text, run_command)`.
pub fn slash_completions(command: &str) -> Vec<(String, String, bool)> {
    match command {
        slash_commands::PROFILE => {
            vec![("<pid>".to_string(), String::new(), false)]
        }
        slash_commands::MEMREFS => ["DataFrame", "dict", "list", "set", "ndarray", "Tensor"]
            .iter()
            .map(|t| ((*t).to_string(), (*t).to_string(), true))
            .collect(),
        _ => vec![],
    }
}

// ── DAP config building ──────────────────────────────────────────────────────

/// Build the DAP configuration JSON from an adapter config value.
///
/// Normalises missing keys to sensible defaults so the debug-adapter
/// subcommand always receives a complete configuration.
pub fn build_dap_config(adapter_config: &Value) -> Value {
    serde_json::json!({
        "program": adapter_config.get("program").and_then(Value::as_str).unwrap_or(""),
        "args": adapter_config.get("args").unwrap_or(&serde_json::json!([])),
        "cwd": adapter_config.get("cwd").and_then(Value::as_str).unwrap_or(""),
        "python": adapter_config.get("python").and_then(Value::as_str).unwrap_or("python3"),
        "justMyCode": adapter_config.get("justMyCode").and_then(Value::as_bool).unwrap_or(true),
        "stopOnEntry": adapter_config.get("stopOnEntry").and_then(Value::as_bool).unwrap_or(false),
        "console": adapter_config.get("console").and_then(Value::as_str).unwrap_or("integratedTerminal"),
    })
}

/// Determine whether a DAP config represents an "attach" request.
///
/// Returns `true` when `processId` is present **or** `request` is `"attach"`.
/// Returns `false` for launch (including when `request` is absent).
/// Returns `Err` for unrecognised request kinds.
pub fn is_attach_request(config: &Value) -> Result<bool, String> {
    if config.get("processId").is_some() {
        return Ok(true);
    }
    match config.get("request").and_then(Value::as_str) {
        Some("attach") => Ok(true),
        Some("launch") | None => Ok(false),
        Some(other) => Err(format!("Unknown request kind: {other}")),
    }
}

/// Build a launch-mode scenario config from high-level parameters.
pub fn build_launch_scenario(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    stop_on_entry: bool,
) -> Value {
    serde_json::json!({
        "program": program,
        "args": args,
        "cwd": cwd,
        "stopOnEntry": stop_on_entry,
        "justMyCode": true,
        "console": "integratedTerminal",
    })
}

/// Build an attach-mode scenario config.
pub fn build_attach_scenario(process_id: Option<u32>) -> Value {
    serde_json::json!({
        "processId": process_id,
        "request": "attach",
    })
}

// ── Version check ────────────────────────────────────────────────────────────

/// Compare two semver-ish version strings (e.g. "v0.2.1" vs "0.3.0").
///
/// Returns `true` if `latest` is newer than `current`.
/// Strips a leading 'v' if present.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let parse = |s: &str| -> (u32, u32, u32) {
        let s = s.strip_prefix('v').unwrap_or(s);
        let mut parts = s.split('.');
        let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    };
    parse(latest) > parse(current)
}

// ── Workspace configuration ──────────────────────────────────────────────────

/// Build the default workspace configuration sent when the user has no
/// explicit `basilisk` settings in Zed.
pub fn default_workspace_config() -> Value {
    serde_json::json!({
        config_keys::INLAY_HINTS: {
            config_keys::PARAM_NAMES: true,
            config_keys::VAR_TYPES: true
        },
        config_keys::RUFF: {
            config_keys::RUFF_ENABLED: true
        },
        config_keys::UV: {
            config_keys::UV_ENABLED: true,
            config_keys::UV_EXECUTABLE_PATH: "",
            config_keys::UV_AUTO_SYNC: false,
            config_keys::UV_STUB_SUGGESTIONS: true,
            config_keys::UV_DEPENDENCY_DIAGNOSTICS: true
        }
    })
}

/// Wrap a config value under the `"basilisk"` root key.
pub fn wrap_config(config: &Value) -> Value {
    serde_json::json!({ config_keys::ROOT: config })
}

// ── Binary resolution helpers ────────────────────────────────────────────────

/// Search for a named variable in a list of `(key, value)` pairs.
pub fn find_env_var<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
    env.iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Build the cargo bin path from a home directory.
pub fn cargo_bin_path(home: &str) -> String {
    format!("{home}/.cargo/bin/basilisk")
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
