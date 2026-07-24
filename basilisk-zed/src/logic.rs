//! Pure logic extracted from the Zed extension glue layer.
//!
//! **Zero `zed_extension_api` imports.** Every function here takes and returns
//! only `serde_json::Value`, `String`, `&str`, or basic Rust types so the
//! module compiles and tests on any native target — no WASM host required.

use basilisk_common::{
    commands, config_keys, memory_diagnostics, notifications, profiler_diagnostics,
    profiler_formats, profiler_presets, slash_commands,
};
use serde_json::Value;

// ── Slash commands ───────────────────────────────────────────────────────────

/// A slash command: its `(title, body)` builder and the static completion
/// suggestions offered for its argument.
struct SlashCommand {
    /// Command name (without the leading slash).
    name: &'static str,
    /// Builds the `(panel title, Markdown body)` pair from the command's args.
    body: fn(&[String]) -> (String, String),
    /// Completion suggestions as `(label, new_text, run_command)` tuples.
    completions: &'static [(&'static str, &'static str, bool)],
}

/// Every slash command the extension exposes. Single source of truth for both
/// [`slash_command_output`] and [`slash_completions`] — adding a command here
/// wires up dispatch and completion in one place. Implements [ZED-PROFILE].
const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        name: slash_commands::PROFILE,
        body: slash_profile,
        completions: &[("<pid>", "", false)],
    },
    SlashCommand {
        name: slash_commands::PROFSTOP,
        body: slash_profstop,
        completions: &[],
    },
    SlashCommand {
        name: slash_commands::PROFSNAPSHOT,
        body: slash_profsnapshot,
        completions: &[],
    },
    SlashCommand {
        name: slash_commands::MEMLEAK,
        body: slash_memleak,
        completions: &[],
    },
    SlashCommand {
        name: slash_commands::MEMSTOP,
        body: slash_memstop,
        completions: &[],
    },
    SlashCommand {
        name: slash_commands::MEMREFS,
        body: slash_memrefs,
        completions: &[
            ("DataFrame", "DataFrame", true),
            ("dict", "dict", true),
            ("list", "list", true),
            ("set", "set", true),
            ("ndarray", "ndarray", true),
            ("Tensor", "Tensor", true),
        ],
    },
    SlashCommand {
        name: slash_commands::MODULES,
        body: slash_modules,
        completions: &[("<module_prefix>", "", false)],
    },
    SlashCommand {
        name: slash_commands::SYMBOLS,
        body: slash_symbols,
        completions: &[("<module_name>", "", false)],
    },
    SlashCommand {
        name: slash_commands::HEALTH,
        body: slash_health,
        completions: &[],
    },
    SlashCommand {
        name: slash_commands::BASILISK,
        body: slash_basilisk,
        completions: &[],
    },
    SlashCommand {
        name: slash_commands::TESTS,
        body: slash_tests,
        completions: &[],
    },
    SlashCommand {
        name: slash_commands::RUNTESTS,
        body: slash_runtests,
        completions: &[("<test_id>", "", false)],
    },
    SlashCommand {
        name: slash_commands::TESTFILE,
        body: slash_testfile,
        completions: &[("<file.py>", "", false)],
    },
];

/// Look up a slash command by name.
fn find_slash_command(command: &str) -> Option<&'static SlashCommand> {
    SLASH_COMMANDS.iter().find(|cmd| cmd.name == command)
}

/// Produce the (label, text) pair for a slash command invocation.
///
/// Output is formatted as Markdown for the Zed AI assistant panel.
/// Returns `Err` for unknown command names.
pub fn slash_command_output(command: &str, args: &[String]) -> Result<(String, String), String> {
    find_slash_command(command)
        .map(|cmd| (cmd.body)(args))
        .ok_or_else(|| format!("Unknown slash command: {command}"))
}

fn slash_profile(args: &[String]) -> (String, String) {
    let target = match args.first() {
        Some(pid) => format!("PID `{pid}`"),
        None => "active Python process".to_string(),
    };
    let start = commands::PROFILER_START;
    let stop = commands::PROFILER_STOP;
    let snapshot = commands::PROFILER_SNAPSHOT;
    let list = commands::PROFILER_LIST;
    let line_code = profiler_diagnostics::LINE;
    let func_code = profiler_diagnostics::FUNC;
    let progress = notifications::PROFILER_PROGRESS;
    let quick = profiler_presets::QUICK;
    let detailed = profiler_presets::DETAILED;
    let long_running = profiler_presets::LONG_RUNNING;
    let text = format!(
        "## CPU Profiling\n\n\
         **Target:** {target}\n\n\
         Basilisk profiles Python processes via `py-spy` (zero overhead, no instrumentation).\n\
         Diagnostics appear inline as `{line_code}` / `{func_code}` hints on hot lines and functions.\n\
         Live progress is delivered via `{progress}` notifications.\n\n\
         ### How to start\n\
         Open the command palette and run **`{start}`** with:\n\
         ```json\n\
         {{\"pid\": <PID>, \"sampleRate\": 100, \"includeNative\": false}}\n\
         ```\n\
         If a debug session is active, the PID is auto-detected — omit the `pid` field.\n\n\
         ### Presets\n\
         | Preset | Sample Rate | Duration | Best for |\n\
         |--------|-------------|----------|----------|\n\
         | `{quick}` | 100 Hz | 10 s | Quick hotspot checks |\n\
         | `{detailed}` | 200 Hz | 60 s | Thorough, higher-fidelity analysis |\n\
         | `{long_running}` | 50 Hz | Unlimited | Servers and batch jobs, minimal overhead |\n\n\
         ### Results\n\
         - **Inline diagnostics** — `{line_code}` / `{func_code}` hints on hot lines (≥1% / ≥2% threshold)\n\
         - **Speedscope JSON** — written to `/tmp/`, open at speedscope.app for interactive flamegraph\n\
         - **Flamegraph SVG** — request `\"format\": \"flamegraph\"` on stop (inferno rendering)\n\n\
         ### Commands\n\
         | Command | Description |\n\
         |---------|-------------|\n\
         | `{start}` | Begin sampling (PID optional if debug session active) |\n\
         | `{stop}` | Stop and export results (speedscope / flamegraph / summary) |\n\
         | `{snapshot}` | Snapshot without stopping — diagnostics updated immediately |\n\
         | `{list}` | List active profiling sessions with sample counts |"
    );
    ("CPU Profiling".to_string(), text)
}

fn slash_profstop(_args: &[String]) -> (String, String) {
    let stop = commands::PROFILER_STOP;
    let speedscope = profiler_formats::SPEEDSCOPE;
    let flamegraph = profiler_formats::FLAMEGRAPH;
    let summary = profiler_formats::SUMMARY;
    let text = format!(
        "## Stop Profiling\n\n\
        Run **`{stop}`** from the command palette with:\n\
        ```json\n\
        {{\"sessionId\": \"<session-id>\", \"format\": \"{speedscope}\"}}\n\
        ```\n\n\
        ### Output formats\n\
        | Format | Description |\n\
        |--------|-------------|\n\
        | `{speedscope}` | JSON for speedscope.app (default) |\n\
        | `{flamegraph}` | SVG flamegraph via inferno |\n\
        | `{summary}` | Text-only, no file export |\n\n\
        ### What happens on stop\n\
        1. Sampling thread stops, remaining samples drained\n\
        2. Hot lines/functions computed (above 1%/2% threshold)\n\
        3. `publishDiagnostics` sent for each profiled file — hints appear inline\n\
        4. Export file written to temp directory\n\n\
        > Open the speedscope JSON at speedscope.app for an interactive flamegraph."
    );
    ("Stop Profiling".to_string(), text)
}

fn slash_profsnapshot(_args: &[String]) -> (String, String) {
    let snapshot = commands::PROFILER_SNAPSHOT;
    let text = format!(
        "## Profile Snapshot\n\n\
        Run **`{snapshot}`** from the command palette.\n\
        Takes a point-in-time snapshot without stopping the session.\n\n\
        Diagnostics are published immediately for the snapshot data.\n\
        Profiling continues — use `/profstop` to end the session.\n\n\
        > Useful for checking hotspots during a long-running profiling session."
    );
    ("Profile Snapshot".to_string(), text)
}

fn slash_memleak(_args: &[String]) -> (String, String) {
    let start = commands::MEMORY_START;
    let snapshot = commands::MEMORY_SNAPSHOT;
    let diff = commands::MEMORY_DIFF;
    let refs = commands::MEMORY_REFERENCES;
    let gc = commands::MEMORY_GC_COLLECT;
    let alloc = memory_diagnostics::ALLOC;
    let growth = memory_diagnostics::GROWTH;
    let leak = memory_diagnostics::LEAK;
    let cycle = memory_diagnostics::CYCLE;
    let timeline = notifications::MEMORY_TIMELINE;
    let text = format!(
        "## Memory Leak Tracking\n\n\
        Tracks object allocations via `tracemalloc` injection into an active debug session.\n\
        Memory timeline data is delivered via `{timeline}` notifications.\n\n\
        ### How to start\n\
        Run **`{start}`** from the command palette.\n\
        Requires an active debug session (debugpy) — the LSP injects Python code via DAP evaluate.\n\n\
        ### Commands\n\
        | Command | Description |\n\
        |---------|-------------|\n\
        | `{start}` | Begin tracking allocations |\n\
        | `{snapshot}` | Capture allocation snapshot |\n\
        | `{diff}` | Compare two snapshots for growth |\n\
        | `{refs}` | Walk object reference graph |\n\
        | `{gc}` | Force GC and report uncollectable |\n\n\
        ### Diagnostics\n\
        - `{alloc}` — top allocation sites (Hint)\n\
        - `{growth}` — memory growth detected (Warning)\n\
        - `{leak}` — suspected leak (Warning)\n\
        - `{cycle}` — reference cycle with `__del__` (Error)"
    );
    ("Memory Tracking".to_string(), text)
}

fn slash_memstop(_args: &[String]) -> (String, String) {
    let leak = memory_diagnostics::LEAK;
    let cycle = memory_diagnostics::CYCLE;
    let text = format!(
        "## Stop Memory Tracking\n\n\
        Stops `tracemalloc` injection and generates the final leak report.\n\n\
        Diagnostics published for each file with significant allocations.\n\
        Leak confidence: **Definite** > **High** > **Medium** > **Low**.\n\n\
        - `{leak}` — suspected leak with confidence score (Warning)\n\
        - `{cycle}` — reference cycle involving `__del__` finalizers (Error)\n\n\
        > Use `/memrefs <TypeName>` to inspect retention paths for leaked types."
    );
    ("Memory Report".to_string(), text)
}

fn slash_memrefs(args: &[String]) -> (String, String) {
    let type_name = args.first().map_or("(unknown)", String::as_str);
    let refs_cmd = commands::MEMORY_REFERENCES;
    let cycle_code = memory_diagnostics::CYCLE;
    let text = format!(
        "## Reference Graph: `{type_name}`\n\n\
         Run **`{refs_cmd}`** from the command palette with:\n\
         ```json\n\
         {{\"targetType\": \"{type_name}\", \"maxDepth\": 5, \"maxNodes\": 200}}\n\
         ```\n\n\
         Walks `gc.get_referrers()` from GC roots to all live instances of `{type_name}`.\n\n\
         ### Output\n\
         - **Nodes** — objects with type, size, repr\n\
         - **Edges** — reference relationships with labels (`.attr`, `[key]`)\n\
         - **Cycles** — detected via DFS, flagged as `{cycle_code}`\n\
         - **Retention path** — human-readable chain from GC root to target"
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

fn slash_health(_args: &[String]) -> (String, String) {
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

fn slash_basilisk(_args: &[String]) -> (String, String) {
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
    find_slash_command(command).map_or_else(Vec::new, |cmd| {
        cmd.completions
            .iter()
            .map(|(label, new_text, run)| ((*label).to_string(), (*new_text).to_string(), *run))
            .collect()
    })
}

// ── DAP config building ──────────────────────────────────────────────────────
// Launch/attach config normalisation for the basilisk-debug adapter; mirrors
// debug_adapter_schemas/basilisk-debug.json. Implements [ZED-DAP].

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
/// explicit `basilisk` settings in Zed. Maps shared LSP config into Zed's
/// settings structure — implements [ZED-CONFIG].
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
            config_keys::UV_AUTO_SYNC: false
        },
        config_keys::TEST_EXPLORER: {
            config_keys::TEST_EXPLORER_ENABLED: true,
            config_keys::TEST_EXPLORER_FRAMEWORK: "auto",
            config_keys::TEST_EXPLORER_PYTEST_PATH: "pytest",
            config_keys::TEST_EXPLORER_ARGS: [],
            config_keys::TEST_EXPLORER_AUTO_DISCOVER_ON_SAVE: true,
            config_keys::TEST_EXPLORER_USE_UV_RUN: true
        },
        config_keys::PROFILER: {
            config_keys::PROFILER_ENABLED: true,
            config_keys::PROFILER_SAMPLE_RATE: 100,
            config_keys::PROFILER_INCLUDE_NATIVE: false,
            config_keys::PROFILER_LINE_THRESHOLD: 0.01,
            config_keys::PROFILER_FUNC_THRESHOLD: 0.02,
            config_keys::PROFILER_MAX_DIAGNOSTICS: 20,
            config_keys::PROFILER_AUTO_ON_LAUNCH: false,
            config_keys::PROFILER_DEFAULT_FORMAT: "speedscope"
        },
        config_keys::MEMORY: {
            config_keys::MEMORY_TRACEBACK_DEPTH: 25,
            config_keys::MEMORY_AUTO_SNAPSHOT_INTERVAL: 0,
            config_keys::MEMORY_MAX_DIAGNOSTICS: 10
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

/// Resolve an explicit, user-provided binary override, if any.
///
/// Precedence: the Zed LSP `binary.path` setting, then the `BASILISK_PATH`
/// environment variable. Returns `None` when neither is set — the signal to
/// fall back to the managed GitHub-release download.
///
/// There is deliberately **no** filesystem default (e.g. `~/.cargo/bin`):
/// installing the extension alone must be enough to get a working binary, so
/// the absence of an explicit override means "download the matching release
/// asset", never "guess a path that probably does not exist". Implements
/// [ZED-DIST].
#[must_use]
pub fn resolve_binary_override(
    settings_path: Option<&str>,
    env_path: Option<&str>,
) -> Option<String> {
    settings_path.or(env_path).map(str::to_string)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
