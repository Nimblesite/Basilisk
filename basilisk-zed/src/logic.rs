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
        slash_commands::PROFILE => Ok(slash_profile(args)),
        slash_commands::PROFSTOP => Ok(slash_profstop()),
        slash_commands::PROFSNAPSHOT => Ok(slash_profsnapshot()),
        slash_commands::MEMLEAK => Ok(slash_memleak()),
        slash_commands::MEMSTOP => Ok(slash_memstop()),
        slash_commands::MEMREFS => Ok(slash_memrefs(args)),
        slash_commands::MODULES => Ok(slash_modules(args)),
        slash_commands::SYMBOLS => Ok(slash_symbols(args)),
        slash_commands::HEALTH => Ok(slash_health()),
        slash_commands::BASILISK => Ok(slash_basilisk()),
        slash_commands::TESTS => Ok(slash_tests(args)),
        slash_commands::RUNTESTS => Ok(slash_runtests(args)),
        slash_commands::TESTFILE => Ok(slash_testfile(args)),
        _ => Err(format!("Unknown slash command: {command}")),
    }
}

fn slash_profile(args: &[String]) -> (String, String) {
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
    ("Profile Started".to_string(), text)
}

fn slash_profstop() -> (String, String) {
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
    ("Profile Results".to_string(), text)
}

fn slash_profsnapshot() -> (String, String) {
    let text = "\
        ## Profile Snapshot\n\n\
        Snapshot captured. **Profiling continues.**\n\n\
        Results sent via LSP diagnostics. \
        Use `/profstop` to stop, or `/profsnapshot` again for another snapshot."
        .to_string();
    ("Profile Snapshot".to_string(), text)
}

fn slash_memleak() -> (String, String) {
    let text = "\
        ## Memory Tracking Started\n\n\
        Tracking object allocations via debug session.\n\n\
        Use `/memstop` to stop and generate a leak report, \
        or `/memrefs <TypeName>` to query retention paths for a specific type."
        .to_string();
    ("Memory Tracking".to_string(), text)
}

fn slash_memstop() -> (String, String) {
    let text = "\
        ## Memory Leak Report\n\n\
        Memory tracking stopped. Leak report sent via LSP diagnostics.\n\n\
        | Metric | Value |\n\
        |--------|-------|\n\
        | Status | Stopped |\n\
        | Output | LSP diagnostics with confidence scores |\n\n\
        > Use `/memrefs <TypeName>` to inspect retention paths for specific types."
        .to_string();
    ("Memory Report".to_string(), text)
}

fn slash_memrefs(args: &[String]) -> (String, String) {
    let type_name = args.first().map_or("(unknown)", String::as_str);
    let text = format!(
        "## Reference Graph: `{type_name}`\n\n\
         Querying retention paths for `{type_name}`...\n\n\
         Results will show the reference chain from GC roots \
         to instances of `{type_name}`, helping identify why objects are not collected."
    );
    ("Reference Graph".to_string(), text)
}

fn slash_modules(args: &[String]) -> (String, String) {
    let scope = match args.first() {
        Some(prefix) => format!("prefix `{prefix}`"),
        None => "entire workspace".to_string(),
    };
    let text = format!(
        "## Workspace Modules\n\n\
         **Scope:** {scope}\n\n\
         Fetching module tree via `basilisk.workspaceModules`.\n\n\
         The module tree shows:\n\
         - **Packages** — directories with `__init__.py`\n\
         - **Modules** — individual `.py` files\n\
         - **Symbols** — classes, functions, variables, constants\n\n\
         Each symbol includes:\n\
         - Type annotation status (annotated/unannotated)\n\
         - Export status (`__all__`)\n\
         - Line number for navigation\n\n\
         > Use `/symbols <module>` to drill into a specific module."
    );
    ("Workspace Modules".to_string(), text)
}

fn slash_symbols(args: &[String]) -> (String, String) {
    let module = args.first().map_or("(all modules)", String::as_str);
    let text = format!(
        "## Module Symbols: `{module}`\n\n\
         Fetching symbols via `basilisk.workspaceModules` with scope `{module}`.\n\n\
         | Symbol | Kind | Annotated | Line |\n\
         |--------|------|-----------|------|\n\
         | *(loading...)* | | | |\n\n\
         > Symbols are extracted from the resolved AST, not from imports."
    );
    ("Module Symbols".to_string(), text)
}

fn slash_health() -> (String, String) {
    let text = "\
        ## Type Health\n\n\
        Fetching workspace health via `basilisk.typeHealth`.\n\n\
        | Metric | Value |\n\
        |--------|-------|\n\
        | Coverage | *(loading...)* |\n\
        | Errors | *(loading...)* |\n\
        | Warnings | *(loading...)* |\n\
        | Adopted Files | *(loading...)* |\n\n\
        Per-module breakdown sorted by coverage (worst first):\n\n\
        | Module | Coverage | Errors | Warnings | Status |\n\
        |--------|----------|--------|----------|--------|\n\
        | *(loading...)* | | | | |\n\n\
        > Unannotated symbols are listed per module. Use `/symbols <module>` to see details."
        .to_string();
    ("Type Health".to_string(), text)
}

fn slash_basilisk() -> (String, String) {
    let text = "\
        ## Basilisk Server Info\n\n\
        **Basilisk** — strict-by-default Python type checker and LSP built in Rust.\n\n\
        ### Features\n\
        - Type checking (strict-by-default, gradual adoption)\n\
        - Inlay hints (parameter names, variable types)\n\
        - Ruff integration (formatting, import organization)\n\
        - Test explorer (pytest + unittest)\n\
        - Debugger (debugpy integration)\n\
        - uv package manager integration\n\
        - Profiling and memory analysis\n\n\
        ### Quick Commands\n\
        | Command | Description |\n\
        |---------|-------------|\n\
        | `/modules` | Show workspace module tree |\n\
        | `/symbols <mod>` | Show symbols in a module |\n\
        | `/health` | Type health statistics |\n\
        | `/tests` | Discover tests |\n\
        | `/runtests` | Execute tests |\n\
        | `/profile` | Start CPU profiling |\n\
        | `/memleak` | Start memory tracking |\n\n\
        > Visit [basilisk-python.dev](https://www.basilisk-python.dev) for documentation."
        .to_string();
    ("Basilisk Info".to_string(), text)
}

fn slash_tests(args: &[String]) -> (String, String) {
    let scope = match args.first() {
        Some(file) => format!("file `{file}`"),
        None => "workspace".to_string(),
    };
    let text = format!(
        "## Test Discovery\n\n\
         **Scope:** {scope}\n\n\
         Discovering pytest and unittest tests from AST (no import needed).\n\n\
         Tests are sent to the LSP server via `basilisk.discoverTests` and \
         appear as inline run buttons via tree-sitter runnables.\n\n\
         **Detected patterns:**\n\
         - `def test_*()` — pytest test functions\n\
         - `class Test*` — pytest test classes\n\
         - `unittest.TestCase` subclasses and `def test_*` methods\n\n\
         > Use `/runtests` to execute tests, or click the inline run button."
    );
    ("Test Discovery".to_string(), text)
}

fn slash_runtests(args: &[String]) -> (String, String) {
    let target = match args.first() {
        Some(test_id) => format!("test `{test_id}`"),
        None => "all tests".to_string(),
    };
    let text = format!(
        "## Running Tests\n\n\
         **Target:** {target}\n\n\
         Executing via `pytest` subprocess (or `uv run pytest` in uv projects).\n\n\
         | Setting | Value |\n\
         |---------|-------|\n\
         | Runner | pytest |\n\
         | Output | `--tb=short -q` |\n\
         | uv-aware | auto-detected |\n\n\
         Results:\n\
         - **Per-test status** — pass/fail/skip/error for each test\n\
         - **Inline failures** — assertion errors and tracebacks\n\
         - **Exit code** — overall pass/fail\n\n\
         > Use `/testfile` to run tests in the current file only."
    );
    ("Running Tests".to_string(), text)
}

fn slash_testfile(args: &[String]) -> (String, String) {
    let file = args.first().map_or("(current file)", String::as_str);
    let text = format!(
        "## Running File Tests\n\n\
         **File:** `{file}`\n\n\
         Running all tests in this file via `basilisk.runTestFile`.\n\n\
         Uses `uv run pytest` when a uv project is detected, \
         otherwise bare `pytest` with `VIRTUAL_ENV` set from the workspace venv."
    );
    ("File Tests".to_string(), text)
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
        slash_commands::MODULES => {
            vec![("<module_prefix>".to_string(), String::new(), false)]
        }
        slash_commands::SYMBOLS => {
            vec![("<module_name>".to_string(), String::new(), false)]
        }
        slash_commands::RUNTESTS => {
            vec![("<test_id>".to_string(), String::new(), false)]
        }
        slash_commands::TESTFILE => {
            vec![("<file.py>".to_string(), String::new(), false)]
        }
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
        },
        config_keys::TEST_EXPLORER: {
            config_keys::TEST_EXPLORER_ENABLED: true,
            config_keys::TEST_EXPLORER_FRAMEWORK: "auto",
            config_keys::TEST_EXPLORER_PYTEST_PATH: "pytest",
            config_keys::TEST_EXPLORER_ARGS: [],
            config_keys::TEST_EXPLORER_AUTO_DISCOVER_ON_SAVE: true,
            config_keys::TEST_EXPLORER_USE_UV_RUN: true
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
