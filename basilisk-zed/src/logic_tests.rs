//! Tests for [`super::logic`] — pure functions with no Zed API dependency.
#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "Test assertions use expect() and JSON indexing for readability"
)]

use basilisk_common::slash_commands;

use super::*;

// ── Slash command output ─────────────────────────────────────────────────

#[test]
fn profile_without_pid() {
    let (label, text) = slash_command_output("profile", &[]).expect("should succeed");
    assert_eq!(label, "Profile Started");
    assert!(text.contains("Profiling Started"));
    assert!(text.contains("active Python process"));
    assert!(text.contains("py-spy"));
    assert!(text.contains("/profstop"));
}

#[test]
fn profile_with_pid() {
    let args = vec!["1234".to_string()];
    let (label, text) = slash_command_output("profile", &args).expect("should succeed");
    assert_eq!(label, "Profile Started");
    assert!(text.contains("PID `1234`"));
    assert!(text.contains("Speedscope"));
}

#[test]
fn profstop_output() {
    let (label, text) = slash_command_output("profstop", &[]).expect("should succeed");
    assert_eq!(label, "Profile Results");
    assert!(text.contains("Profiling stopped"));
    assert!(text.contains("flamegraph"));
}

#[test]
fn profsnapshot_output() {
    let (label, text) = slash_command_output("profsnapshot", &[]).expect("should succeed");
    assert_eq!(label, "Profile Snapshot");
    assert!(text.contains("Snapshot captured"));
    assert!(text.contains("continues"));
}

#[test]
fn memleak_output() {
    let (label, text) = slash_command_output("memleak", &[]).expect("should succeed");
    assert_eq!(label, "Memory Tracking");
    assert!(text.contains("Tracking"));
    assert!(text.contains("/memstop"));
}

#[test]
fn memstop_output() {
    let (label, text) = slash_command_output("memstop", &[]).expect("should succeed");
    assert_eq!(label, "Memory Report");
    assert!(text.contains("stopped"));
    assert!(text.contains("/memrefs"));
}

#[test]
fn memrefs_with_type() {
    let args = vec!["DataFrame".to_string()];
    let (label, text) = slash_command_output("memrefs", &args).expect("should succeed");
    assert_eq!(label, "Reference Graph");
    assert!(text.contains("DataFrame"));
    assert!(text.contains("retention paths"));
}

#[test]
fn memrefs_without_type() {
    let (_, text) = slash_command_output("memrefs", &[]).expect("should succeed");
    assert!(text.contains("(unknown)"));
}

#[test]
fn unknown_command_errors() {
    let result = slash_command_output("nonexistent", &[]);
    assert!(result.is_err());
    let err = result.expect_err("should be error");
    assert!(err.contains("nonexistent"));
}

// ── All six slash commands produce non-empty markdown ────────────────────

#[test]
fn all_slash_commands_produce_output() {
    let commands = [
        slash_commands::PROFILE,
        slash_commands::PROFSTOP,
        slash_commands::PROFSNAPSHOT,
        slash_commands::MEMLEAK,
        slash_commands::MEMSTOP,
        slash_commands::MEMREFS,
    ];
    for cmd in commands {
        let (label, text) = slash_command_output(cmd, &[]).expect(cmd);
        assert!(!label.is_empty(), "empty label for {cmd}");
        assert!(!text.is_empty(), "empty text for {cmd}");
    }
}

#[test]
fn slash_output_is_markdown() {
    let commands = [
        slash_commands::PROFILE,
        slash_commands::PROFSTOP,
        slash_commands::PROFSNAPSHOT,
        slash_commands::MEMLEAK,
        slash_commands::MEMSTOP,
        slash_commands::MEMREFS,
    ];
    for cmd in commands {
        let (_, text) = slash_command_output(cmd, &[]).expect(cmd);
        assert!(
            text.contains("##"),
            "slash command {cmd} should produce markdown with headers"
        );
    }
}

// ── Slash command completions ────────────────────────────────────────────

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

// ── DAP config building ─────────────────────────────────────────────────

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

// ── DAP request kind ────────────────────────────────────────────────────

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

// ── DAP scenario builders ───────────────────────────────────────────────

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

// ── Workspace configuration ─────────────────────────────────────────────

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
    assert_eq!(config["uv"]["stubSuggestions"], true);
    assert_eq!(config["uv"]["dependencyDiagnostics"], true);
}

#[test]
fn wrap_config_preserves_uv_settings() {
    let inner = serde_json::json!({
        "uv": {
            "enabled": false,
            "executablePath": "/usr/local/bin/uv",
            "autoSync": true,
            "stubSuggestions": false,
            "dependencyDiagnostics": false
        }
    });
    let wrapped = wrap_config(&inner);
    assert_eq!(wrapped["basilisk"]["uv"]["enabled"], false);
    assert_eq!(
        wrapped["basilisk"]["uv"]["executablePath"],
        "/usr/local/bin/uv"
    );
    assert_eq!(wrapped["basilisk"]["uv"]["autoSync"], true);
    assert_eq!(wrapped["basilisk"]["uv"]["stubSuggestions"], false);
    assert_eq!(wrapped["basilisk"]["uv"]["dependencyDiagnostics"], false);
}

#[test]
fn wrap_config_nests_under_basilisk() {
    let inner = serde_json::json!({"foo": "bar"});
    let wrapped = wrap_config(&inner);
    assert_eq!(wrapped["basilisk"]["foo"], "bar");
}

// ── Binary resolution helpers ───────────────────────────────────────────

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
fn cargo_bin_path_construction() {
    assert_eq!(
        cargo_bin_path("/home/user"),
        "/home/user/.cargo/bin/basilisk"
    );
}

#[test]
fn cargo_bin_path_trailing_slash() {
    assert_eq!(
        cargo_bin_path("/home/user/"),
        "/home/user//.cargo/bin/basilisk"
    );
}

// ── Version check ───────────────────────────────────────────────────────

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
