//! Tests for [`super::logic`] — pure functions with no Zed API dependency.
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "Test assertions use expect() and JSON indexing for readability"
)]

use basilisk_common::slash_commands;

use super::*;

/// Run `slash_command_output(cmd, args)`, assert it succeeds with `label`, and
/// return `text` for further assertions. Replaces the repeated `expect`/
/// `assert_eq!(label, ...)` boilerplate that appears in every slash-command test.
fn run_slash(cmd: &str, args: &[String], expected_label: &str) -> String {
    let (label, text) = slash_command_output(cmd, args).expect("should succeed");
    assert_eq!(label, expected_label);
    text
}

/// Run a slash command and return only the body text (label is ignored).
fn slash_text(cmd: &str, args: &[String]) -> String {
    slash_command_output(cmd, args).expect("should succeed").1
}

/// Every slash command Basilisk advertises, in palette order.
///
/// Shared by the exhaustive "all commands" tests so the list lives in exactly
/// one place — adding a command here keeps both the non-empty-output and the
/// markdown-shape assertions in sync.
const ALL_SLASH_COMMANDS: [&str; 13] = [
    slash_commands::PROFILE,
    slash_commands::PROFSTOP,
    slash_commands::PROFSNAPSHOT,
    slash_commands::MEMLEAK,
    slash_commands::MEMSTOP,
    slash_commands::MEMREFS,
    slash_commands::MODULES,
    slash_commands::SYMBOLS,
    slash_commands::HEALTH,
    slash_commands::BASILISK,
    slash_commands::TESTS,
    slash_commands::RUNTESTS,
    slash_commands::TESTFILE,
];

// ── Slash command output ─────────────────────────────────────────────────
// Exercises [ZED-PROFILE]: every slash command's dispatch and markdown output.

#[test]
fn profile_without_pid() {
    let text = run_slash("profile", &[], "CPU Profiling");
    assert!(text.contains("active Python process"));
    assert!(text.contains("py-spy"));
    assert!(text.contains("basilisk.profiler.start"));
}

#[test]
fn profile_with_pid() {
    let text = run_slash("profile", &["1234".to_string()], "CPU Profiling");
    assert!(text.contains("PID `1234`"));
    assert!(text.contains("Speedscope"));
}

#[test]
fn profstop_output() {
    let text = run_slash("profstop", &[], "Stop Profiling");
    assert!(text.contains("basilisk.profiler.stop"));
    assert!(text.contains("flamegraph"));
}

#[test]
fn profsnapshot_output() {
    let text = run_slash("profsnapshot", &[], "Profile Snapshot");
    assert!(text.contains("basilisk.profiler.snapshot"));
    assert!(text.contains("continues"));
}

#[test]
fn memleak_output() {
    let text = run_slash("memleak", &[], "Memory Tracking");
    assert!(text.contains("tracemalloc"));
    assert!(text.contains("basilisk.memory.start"));
}

#[test]
fn memstop_output() {
    let text = run_slash("memstop", &[], "Memory Report");
    assert!(text.contains("Stops"));
    assert!(text.contains("/memrefs"));
}

#[test]
fn memrefs_with_type() {
    let text = run_slash("memrefs", &["DataFrame".to_string()], "Reference Graph");
    assert!(text.contains("DataFrame"));
    assert!(text.contains("gc.get_referrers"));
}

#[test]
fn memrefs_without_type() {
    let text = slash_text("memrefs", &[]);
    assert!(text.contains("(unknown)"));
}

#[test]
fn unknown_command_errors() {
    let result = slash_command_output("nonexistent", &[]);
    assert!(result.is_err());
    let err = result.expect_err("should be error");
    assert!(err.contains("nonexistent"));
}

// ── All slash commands produce non-empty markdown ────────────────────

#[test]
fn all_slash_commands_produce_output() {
    for cmd in ALL_SLASH_COMMANDS {
        let (label, text) = slash_command_output(cmd, &[]).expect(cmd);
        assert!(!label.is_empty(), "empty label for {cmd}");
        assert!(!text.is_empty(), "empty text for {cmd}");
    }
}

#[test]
fn slash_output_is_markdown() {
    for cmd in ALL_SLASH_COMMANDS {
        let (_, text) = slash_command_output(cmd, &[]).expect(cmd);
        assert!(
            text.contains("##"),
            "slash command {cmd} should produce markdown with headers"
        );
    }
}

// ── Slash command completions ────────────────────────────────────────

#[test]
fn profile_completions() {
    let completions = slash_completions("profile");
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].0, "<pid>");
    assert!(!completions[0].2, "run_command should be false");
}

#[test]
fn memrefs_completions() {
    let completions = slash_completions("memrefs");
    assert_eq!(completions.len(), 6);
    let labels: Vec<&str> = completions.iter().map(|(l, _, _)| l.as_str()).collect();
    assert!(labels.contains(&"DataFrame"));
    assert!(labels.contains(&"dict"));
    assert!(labels.contains(&"Tensor"));
    for (_, _, run) in &completions {
        assert!(run, "memrefs completions should have run_command = true");
    }
}

#[test]
fn unknown_command_has_no_completions() {
    assert!(slash_completions("unknown").is_empty());
}

// ── Profiler slash command content quality ───────────────────────────

#[test]
fn profile_output_documents_all_four_commands() {
    let text = slash_text("profile", &[]);
    assert!(
        text.contains("basilisk.profiler.start"),
        "must document start command"
    );
    assert!(
        text.contains("basilisk.profiler.stop"),
        "must document stop command"
    );
    assert!(
        text.contains("basilisk.profiler.snapshot"),
        "must document snapshot command"
    );
    assert!(
        text.contains("basilisk.profiler.list"),
        "must document list command"
    );
}

#[test]
fn profile_output_documents_output_formats() {
    let text = slash_text("profile", &[]);
    assert!(
        text.contains("Speedscope"),
        "must mention speedscope format"
    );
    assert!(
        text.contains("Flamegraph") || text.contains("flamegraph"),
        "must mention flamegraph"
    );
    assert!(
        text.contains("BSK-PROF") || text.contains("diagnostics"),
        "must mention diagnostics output"
    );
}

#[test]
fn profstop_output_documents_output_format_options() {
    let text = slash_text("profstop", &[]);
    assert!(
        text.contains("speedscope"),
        "must mention speedscope format option"
    );
    assert!(
        text.contains("flamegraph"),
        "must mention flamegraph format option"
    );
    assert!(
        text.contains("summary"),
        "must mention summary format option"
    );
}

#[test]
fn profstop_output_documents_diagnostic_delivery() {
    let text = slash_text("profstop", &[]);
    assert!(
        text.contains("publishDiagnostics")
            || text.contains("diagnostics")
            || text.contains("hints"),
        "must explain how diagnostics are delivered"
    );
    assert!(
        text.contains("threshold") || text.contains("1%") || text.contains("2%"),
        "should mention threshold behavior"
    );
}

#[test]
fn memleak_output_documents_tracemalloc_and_commands() {
    let text = slash_text("memleak", &[]);
    assert!(
        text.contains("tracemalloc"),
        "must mention tracemalloc engine"
    );
    assert!(
        text.contains("basilisk.memory.start"),
        "must document start command"
    );
    assert!(
        text.contains("debug session") || text.contains("debugpy"),
        "must mention debug session requirement"
    );
}

#[test]
fn memleak_output_documents_diagnostic_codes() {
    let text = slash_text("memleak", &[]);
    assert!(
        text.contains("BSK-MEM") || text.contains("memory diagnostics"),
        "must mention memory diagnostic codes or diagnostics"
    );
}

#[test]
fn memstop_output_documents_leak_detection() {
    let text = slash_text("memstop", &[]);
    assert!(
        text.contains("leak") || text.contains("Leak"),
        "must mention leak detection"
    );
    assert!(
        text.contains("confidence") || text.contains("snapshot"),
        "should mention confidence scoring or snapshots"
    );
}

#[test]
fn memrefs_output_documents_reference_graph() {
    let args = vec!["DataFrame".to_string()];
    let text = slash_text("memrefs", &args);
    assert!(text.contains("DataFrame"), "must include the target type");
    assert!(
        text.contains("gc.get_referrers")
            || text.contains("reference")
            || text.contains("retention"),
        "must explain reference graph walking"
    );
    assert!(
        text.contains("Cycle")
            || text.contains("cycle")
            || text.contains("Retention")
            || text.contains("retention"),
        "should explain what the graph reveals"
    );
}

#[test]
fn profile_with_pid_includes_pid_in_output() {
    for pid in ["1234", "99999", "1"] {
        let args = vec![pid.to_string()];
        let text = slash_text("profile", &args);
        assert!(
            text.contains(pid),
            "output must include PID {pid} when provided"
        );
    }
}

#[test]
fn all_profiler_commands_have_markdown_tables_or_lists() {
    let profiler_cmds = [
        slash_commands::PROFILE,
        slash_commands::PROFSTOP,
        slash_commands::MEMLEAK,
    ];
    for cmd in profiler_cmds {
        let (_, text) = slash_command_output(cmd, &[]).expect(cmd);
        let has_table = text.contains('|');
        let has_list = text.contains("- ") || text.contains("1.");
        assert!(
            has_table || has_list,
            "slash command {cmd} should have structured content (table or list)"
        );
    }
}

#[test]
fn profiler_slash_commands_dont_contain_raw_code_paths() {
    let cmds = [
        slash_commands::PROFILE,
        slash_commands::PROFSTOP,
        slash_commands::PROFSNAPSHOT,
        slash_commands::MEMLEAK,
        slash_commands::MEMSTOP,
        slash_commands::MEMREFS,
    ];
    for cmd in cmds {
        let (_, text) = slash_command_output(cmd, &[]).expect(cmd);
        assert!(
            !text.contains("crates/"),
            "slash command {cmd} should not expose internal code paths"
        );
        assert!(
            !text.contains(".rs"),
            "slash command {cmd} should not reference Rust source files"
        );
    }
}

// ── DAP config building ─────────────────────────────────────────────
// Exercises [ZED-DAP]: launch/attach config building, request-kind detection,
// and scenario builders matching basilisk-debug.json.

#[test]
fn build_dap_config_defaults() {
    let config = build_dap_config(&serde_json::json!({}));
    assert_eq!(config["program"], "");
    assert_eq!(config["python"], "python3");
    assert_eq!(config["justMyCode"], true);
    assert_eq!(config["stopOnEntry"], false);
    assert_eq!(config["console"], "integratedTerminal");
    assert!(config["args"].is_array());
}

#[test]
fn build_dap_config_with_values() {
    let input = serde_json::json!({
        "program": "main.py",
        "python": "/usr/bin/python3.12",
        "justMyCode": false,
        "stopOnEntry": true,
        "console": "internalConsole",
        "args": ["--verbose"],
        "cwd": "/home/user/project",
    });
    let config = build_dap_config(&input);
    assert_eq!(config["program"], "main.py");
    assert_eq!(config["python"], "/usr/bin/python3.12");
    assert_eq!(config["justMyCode"], false);
    assert_eq!(config["stopOnEntry"], true);
    assert_eq!(config["console"], "internalConsole");
    assert_eq!(config["args"][0], "--verbose");
    assert_eq!(config["cwd"], "/home/user/project");
}

// ── DAP request kind ────────────────────────────────────────────────

#[test]
fn launch_by_default() {
    assert!(!is_attach_request(&serde_json::json!({})).expect("should succeed"));
}

#[test]
fn launch_explicit() {
    let config = serde_json::json!({"request": "launch"});
    assert!(!is_attach_request(&config).expect("should succeed"));
}

#[test]
fn attach_by_process_id() {
    let config = serde_json::json!({"processId": 42});
    assert!(is_attach_request(&config).expect("should succeed"));
}

#[test]
fn attach_explicit() {
    let config = serde_json::json!({"request": "attach"});
    assert!(is_attach_request(&config).expect("should succeed"));
}

#[test]
fn attach_process_id_takes_precedence() {
    let config = serde_json::json!({"processId": 42, "request": "launch"});
    assert!(
        is_attach_request(&config).expect("should succeed"),
        "processId should override request field"
    );
}

#[test]
fn unknown_request_kind_errors() {
    let config = serde_json::json!({"request": "restart"});
    assert!(is_attach_request(&config).is_err());
}

// ── DAP scenario builders ───────────────────────────────────────────

#[test]
fn launch_scenario_fields() {
    let scenario =
        build_launch_scenario("app.py", &["--debug".to_string()], Some("/project"), true);
    assert_eq!(scenario["program"], "app.py");
    assert_eq!(scenario["args"][0], "--debug");
    assert_eq!(scenario["cwd"], "/project");
    assert_eq!(scenario["stopOnEntry"], true);
    assert_eq!(scenario["justMyCode"], true);
    assert_eq!(scenario["console"], "integratedTerminal");
}

#[test]
fn launch_scenario_no_cwd() {
    let scenario = build_launch_scenario("app.py", &[], None, false);
    assert!(scenario["cwd"].is_null());
    assert_eq!(scenario["stopOnEntry"], false);
}

#[test]
fn attach_scenario_with_pid() {
    let scenario = build_attach_scenario(Some(9876));
    assert_eq!(scenario["processId"], 9876);
    assert_eq!(scenario["request"], "attach");
}

#[test]
fn attach_scenario_no_pid() {
    let scenario = build_attach_scenario(None);
    assert!(scenario["processId"].is_null());
    assert_eq!(scenario["request"], "attach");
}

// ── Workspace configuration ─────────────────────────────────────────
// Exercises [ZED-CONFIG]: default config + wrapping under the "basilisk" key.

#[test]
fn default_config_has_inlay_hints() {
    let config = default_workspace_config();
    assert_eq!(config["inlayHints"]["parameterNames"], true);
    assert_eq!(config["inlayHints"]["variableTypes"], true);
}

#[test]
fn default_config_has_ruff_enabled() {
    let config = default_workspace_config();
    assert_eq!(config["ruff"]["enabled"], true);
}

#[test]
fn default_config_has_uv_settings() {
    let config = default_workspace_config();
    assert_eq!(config["uv"]["enabled"], true);
    assert_eq!(config["uv"]["executablePath"], "");
    assert_eq!(config["uv"]["autoSync"], false);
    assert!(config["uv"].get("stubSuggestions").is_none());
    assert!(config["uv"].get("dependencyDiagnostics").is_none());
}

#[test]
fn wrap_config_preserves_uv_settings() {
    let inner = serde_json::json!({
        "uv": {
            "enabled": false,
            "executablePath": "/usr/local/bin/uv",
            "autoSync": true
        }
    });
    let wrapped = wrap_config(&inner);
    assert_eq!(wrapped["basilisk"]["uv"]["enabled"], false);
    assert_eq!(
        wrapped["basilisk"]["uv"]["executablePath"],
        "/usr/local/bin/uv"
    );
    assert_eq!(wrapped["basilisk"]["uv"]["autoSync"], true);
}

#[test]
fn wrap_config_nests_under_basilisk() {
    let inner = serde_json::json!({"foo": "bar"});
    let wrapped = wrap_config(&inner);
    assert_eq!(wrapped["basilisk"]["foo"], "bar");
}

// ── Binary resolution helpers ───────────────────────────────────────

#[test]
fn find_env_var_present() {
    let env = vec![
        ("HOME".to_string(), "/home/user".to_string()),
        ("PATH".to_string(), "/usr/bin".to_string()),
    ];
    assert_eq!(find_env_var(&env, "HOME"), Some("/home/user"));
}

#[test]
fn find_env_var_absent() {
    let env = vec![("HOME".to_string(), "/home/user".to_string())];
    assert_eq!(find_env_var(&env, "BASILISK_PATH"), None);
}

#[test]
fn find_env_var_empty_list() {
    let env: Vec<(String, String)> = vec![];
    assert_eq!(find_env_var(&env, "HOME"), None);
}

#[test]
fn binary_override_prefers_settings_path() {
    assert_eq!(
        resolve_binary_override(Some("/custom/basilisk"), Some("/env/basilisk")),
        Some("/custom/basilisk".to_string())
    );
}

#[test]
fn binary_override_falls_back_to_env() {
    assert_eq!(
        resolve_binary_override(None, Some("/env/basilisk")),
        Some("/env/basilisk".to_string())
    );
}

#[test]
fn binary_override_none_triggers_download() {
    // No explicit override -> None, which signals the managed GitHub-release
    // download. There is no `~/.cargo/bin` (or any) filesystem default, so
    // installing the extension alone is enough. Guards [ZED-DIST].
    assert_eq!(resolve_binary_override(None, None), None);
}

// ── Version check ───────────────────────────────────────────────────

#[test]
fn newer_major() {
    assert!(is_newer_version("0.1.0", "1.0.0"));
}

#[test]
fn newer_minor() {
    assert!(is_newer_version("0.1.0", "0.2.0"));
}

#[test]
fn newer_patch() {
    assert!(is_newer_version("0.1.0", "0.1.1"));
}

#[test]
fn same_version() {
    assert!(!is_newer_version("0.1.0", "0.1.0"));
}

#[test]
fn older_version() {
    assert!(!is_newer_version("1.0.0", "0.9.0"));
}

#[test]
fn v_prefix_stripped() {
    assert!(is_newer_version("v0.1.0", "v0.2.0"));
    assert!(is_newer_version("0.1.0", "v0.2.0"));
    assert!(is_newer_version("v0.1.0", "0.2.0"));
}

#[test]
fn v_prefix_same() {
    assert!(!is_newer_version("v0.1.0", "v0.1.0"));
}

// ── Test slash commands ─────────────────────────────────────────────

#[test]
fn tests_discovery_workspace() {
    let text = run_slash("tests", &[], "Test Discovery");
    assert!(text.contains("workspace"));
    assert!(text.contains("pytest"));
    assert!(text.contains("unittest"));
    assert!(text.contains("def test_*"));
}

#[test]
fn tests_discovery_file() {
    let text = run_slash("tests", &["test_api.py".to_string()], "Test Discovery");
    assert!(text.contains("test_api.py"));
}

#[test]
fn runtests_all() {
    let text = run_slash("runtests", &[], "Running Tests");
    assert!(text.contains("all tests"));
    assert!(text.contains("pytest"));
    assert!(text.contains("uv run pytest"));
}

#[test]
fn runtests_specific() {
    let text = run_slash(
        "runtests",
        &["tests/test_api.py::test_login".to_string()],
        "Running Tests",
    );
    assert!(text.contains("test_login"));
}

#[test]
fn testfile_default() {
    let text = run_slash("testfile", &[], "File Tests");
    assert!(text.contains("current file"));
    assert!(text.contains("uv run pytest"));
}

#[test]
fn testfile_specific() {
    let text = run_slash("testfile", &["test_models.py".to_string()], "File Tests");
    assert!(text.contains("test_models.py"));
}

#[test]
fn runtests_completions() {
    let completions = slash_completions("runtests");
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].0, "<test_id>");
    assert!(!completions[0].2, "run_command should be false");
}

#[test]
fn testfile_completions() {
    let completions = slash_completions("testfile");
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].0, "<file.py>");
}

#[test]
fn tests_no_completions() {
    let completions = slash_completions("tests");
    assert!(completions.is_empty());
}

// ── Test explorer workspace config ──────────────────────────────────

#[test]
fn default_config_has_test_explorer() {
    let config = default_workspace_config();
    assert_eq!(config["testExplorer"]["enabled"], true);
    assert_eq!(config["testExplorer"]["framework"], "auto");
    assert_eq!(config["testExplorer"]["pytestPath"], "pytest");
    assert!(config["testExplorer"]["args"].is_array());
    assert_eq!(config["testExplorer"]["autoDiscoverOnSave"], true);
    assert_eq!(config["testExplorer"]["useUvRun"], true);
}

#[test]
fn wrap_config_preserves_test_explorer() {
    let inner = serde_json::json!({
        "testExplorer": {
            "enabled": false,
            "framework": "unittest",
            "pytestPath": "/usr/bin/pytest",
            "useUvRun": false
        }
    });
    let wrapped = wrap_config(&inner);
    assert_eq!(wrapped["basilisk"]["testExplorer"]["enabled"], false);
    assert_eq!(wrapped["basilisk"]["testExplorer"]["framework"], "unittest");
    assert_eq!(
        wrapped["basilisk"]["testExplorer"]["pytestPath"],
        "/usr/bin/pytest"
    );
    assert_eq!(wrapped["basilisk"]["testExplorer"]["useUvRun"], false);
}

// ── Activity panel slash command tests ────────────────────────────

#[test]
fn modules_workspace_scope() {
    let text = run_slash(slash_commands::MODULES, &[], "Workspace Modules");
    assert!(text.contains("entire workspace"));
    assert!(text.contains("basilisk.workspaceModules"));
}

#[test]
fn modules_prefix_scope() {
    let text = run_slash(
        slash_commands::MODULES,
        &["myapp.api".to_string()],
        "Workspace Modules",
    );
    assert!(text.contains("prefix `myapp.api`"));
}

#[test]
fn symbols_output() {
    let text = run_slash(
        slash_commands::SYMBOLS,
        &["myapp.models".to_string()],
        "Module Symbols",
    );
    assert!(text.contains("myapp.models"));
    assert!(text.contains("basilisk.workspaceModules"));
}

#[test]
fn symbols_default() {
    let text = run_slash(slash_commands::SYMBOLS, &[], "Module Symbols");
    assert!(text.contains("all modules"));
}

#[test]
fn health_output() {
    let text = run_slash(slash_commands::HEALTH, &[], "Type Health");
    assert!(text.contains("basilisk.typeHealth"));
    assert!(text.contains("Coverage"));
    assert!(text.contains("Module"));
}

#[test]
fn basilisk_info_output() {
    let text = run_slash(slash_commands::BASILISK, &[], "Basilisk Info");
    assert!(text.contains("strict-by-default"));
    assert!(text.contains("/modules"));
    assert!(text.contains("/health"));
    assert!(text.contains("basilisk-python.dev"));
}

#[test]
fn modules_completions() {
    let completions = slash_completions(slash_commands::MODULES);
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].0, "<module_prefix>");
}

#[test]
fn symbols_completions() {
    let completions = slash_completions(slash_commands::SYMBOLS);
    assert_eq!(completions.len(), 1);
    assert_eq!(completions[0].0, "<module_name>");
}

#[test]
fn health_no_completions() {
    let completions = slash_completions(slash_commands::HEALTH);
    assert!(completions.is_empty());
}

#[test]
fn basilisk_no_completions() {
    let completions = slash_completions(slash_commands::BASILISK);
    assert!(completions.is_empty());
}

// ── Profiler workspace config defaults ──────────────────────────────

#[test]
fn default_config_has_profiler_section() {
    let config = default_workspace_config();
    assert!(
        !config["profiler"].is_null(),
        "profiler section must be present in default config"
    );
}

#[test]
fn default_config_profiler_enabled_by_default() {
    let config = default_workspace_config();
    assert_eq!(
        config["profiler"]["enabled"], true,
        "profiler must be enabled by default"
    );
}

#[test]
fn default_config_profiler_sample_rate() {
    let config = default_workspace_config();
    assert_eq!(
        config["profiler"]["sampleRate"], 100,
        "default sample rate must be 100 Hz"
    );
}

#[test]
fn default_config_profiler_native_frames_disabled() {
    let config = default_workspace_config();
    assert_eq!(
        config["profiler"]["includeNative"], false,
        "native frames must be off by default (low overhead)"
    );
}

#[test]
fn default_config_profiler_thresholds() {
    let config = default_workspace_config();
    let line = config["profiler"]["lineThreshold"]
        .as_f64()
        .expect("lineThreshold must be a number");
    let func = config["profiler"]["funcThreshold"]
        .as_f64()
        .expect("funcThreshold must be a number");
    assert!(
        (line - 0.01).abs() < f64::EPSILON,
        "line threshold must be 1%"
    );
    assert!(
        (func - 0.02).abs() < f64::EPSILON,
        "function threshold must be 2%"
    );
}

#[test]
fn default_config_profiler_max_diagnostics() {
    let config = default_workspace_config();
    assert_eq!(
        config["profiler"]["maxDiagnostics"], 20,
        "max diagnostics per file must be 20"
    );
}

#[test]
fn default_config_profiler_auto_on_launch_disabled() {
    let config = default_workspace_config();
    assert_eq!(
        config["profiler"]["autoOnLaunch"], false,
        "auto-profile on launch must be opt-in"
    );
}

#[test]
fn default_config_profiler_default_format_is_speedscope() {
    let config = default_workspace_config();
    assert_eq!(
        config["profiler"]["defaultFormat"], "speedscope",
        "default export format must be speedscope"
    );
}

#[test]
fn default_config_has_memory_section() {
    let config = default_workspace_config();
    assert!(
        !config["memory"].is_null(),
        "memory section must be present in default config"
    );
}

#[test]
fn default_config_memory_traceback_depth() {
    let config = default_workspace_config();
    assert_eq!(
        config["memory"]["tracebackDepth"], 25,
        "traceback depth must default to 25 frames"
    );
}

#[test]
fn default_config_memory_auto_snapshot_disabled() {
    let config = default_workspace_config();
    assert_eq!(
        config["memory"]["autoSnapshotInterval"], 0,
        "auto-snapshot must be disabled by default"
    );
}

#[test]
fn default_config_memory_max_diagnostics() {
    let config = default_workspace_config();
    assert_eq!(
        config["memory"]["maxDiagnostics"], 10,
        "max memory diagnostics per file must be 10"
    );
}

#[test]
fn wrap_config_preserves_profiler_settings() {
    let inner = serde_json::json!({
        "profiler": {
            "enabled": false,
            "sampleRate": 200,
            "includeNative": true,
            "autoOnLaunch": true,
            "defaultFormat": "flamegraph"
        }
    });
    let wrapped = wrap_config(&inner);
    assert_eq!(wrapped["basilisk"]["profiler"]["enabled"], false);
    assert_eq!(wrapped["basilisk"]["profiler"]["sampleRate"], 200);
    assert_eq!(wrapped["basilisk"]["profiler"]["includeNative"], true);
    assert_eq!(wrapped["basilisk"]["profiler"]["autoOnLaunch"], true);
    assert_eq!(
        wrapped["basilisk"]["profiler"]["defaultFormat"],
        "flamegraph"
    );
}

#[test]
fn wrap_config_preserves_memory_settings() {
    let inner = serde_json::json!({
        "memory": {
            "tracebackDepth": 10,
            "autoSnapshotInterval": 30,
            "maxDiagnostics": 5
        }
    });
    let wrapped = wrap_config(&inner);
    assert_eq!(wrapped["basilisk"]["memory"]["tracebackDepth"], 10);
    assert_eq!(wrapped["basilisk"]["memory"]["autoSnapshotInterval"], 30);
    assert_eq!(wrapped["basilisk"]["memory"]["maxDiagnostics"], 5);
}

// ── Profile command uses shared format constants ─────────────────────

#[test]
fn profstop_output_uses_canonical_format_names() {
    let text = slash_text("profstop", &[]);
    assert!(
        text.contains(basilisk_common::profiler_formats::SPEEDSCOPE),
        "must use canonical speedscope format name"
    );
    assert!(
        text.contains(basilisk_common::profiler_formats::FLAMEGRAPH),
        "must use canonical flamegraph format name"
    );
    assert!(
        text.contains(basilisk_common::profiler_formats::SUMMARY),
        "must use canonical summary format name"
    );
}

#[test]
fn profile_output_documents_presets() {
    let text = slash_text("profile", &[]);
    // Every preset the server parses must be documented — and only those
    // (a documented-but-ignored preset was the old "memory" defect).
    for preset in basilisk_common::profiler_presets::ALL {
        assert!(text.contains(preset), "must document the {preset} preset");
    }
    assert!(
        !text.contains("lightweight") && !text.contains("`memory`"),
        "must not document presets the server does not parse"
    );
}

#[test]
fn memleak_output_uses_canonical_command_names() {
    let text = slash_text("memleak", &[]);
    assert!(
        text.contains(basilisk_common::commands::MEMORY_START),
        "must use canonical memory start command"
    );
    assert!(
        text.contains(basilisk_common::commands::MEMORY_SNAPSHOT),
        "must use canonical memory snapshot command"
    );
    assert!(
        text.contains(basilisk_common::commands::MEMORY_DIFF),
        "must use canonical memory diff command"
    );
    assert!(
        text.contains(basilisk_common::commands::MEMORY_REFERENCES),
        "must use canonical memory references command"
    );
    assert!(
        text.contains(basilisk_common::commands::MEMORY_GC_COLLECT),
        "must use canonical gc collect command"
    );
}

#[test]
fn memleak_output_uses_canonical_diagnostic_codes() {
    let text = slash_text("memleak", &[]);
    assert!(
        text.contains(basilisk_common::memory_diagnostics::ALLOC),
        "must use canonical BSK-MEM-ALLOC code"
    );
    assert!(
        text.contains(basilisk_common::memory_diagnostics::GROWTH),
        "must use canonical BSK-MEM-GROWTH code"
    );
    assert!(
        text.contains(basilisk_common::memory_diagnostics::LEAK),
        "must use canonical BSK-MEM-LEAK code"
    );
    assert!(
        text.contains(basilisk_common::memory_diagnostics::CYCLE),
        "must use canonical BSK-MEM-CYCLE code"
    );
}

#[test]
fn profile_output_uses_canonical_notification_name() {
    let text = slash_text("profile", &[]);
    assert!(
        text.contains(basilisk_common::notifications::PROFILER_PROGRESS),
        "must reference canonical profiler progress notification"
    );
}

#[test]
fn memleak_output_uses_canonical_timeline_notification() {
    let text = slash_text("memleak", &[]);
    assert!(
        text.contains(basilisk_common::notifications::MEMORY_TIMELINE),
        "must reference canonical memory timeline notification"
    );
}

#[test]
fn memrefs_output_uses_canonical_command_and_diagnostic() {
    let args = vec!["MyModel".to_string()];
    let text = slash_text("memrefs", &args);
    assert!(
        text.contains(basilisk_common::commands::MEMORY_REFERENCES),
        "must use canonical memory references command"
    );
    assert!(
        text.contains(basilisk_common::memory_diagnostics::CYCLE),
        "must use canonical BSK-MEM-CYCLE diagnostic code"
    );
}
