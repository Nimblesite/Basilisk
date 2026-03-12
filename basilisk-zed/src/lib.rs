use zed_extension_api::{self as zed, serde_json, Result};

use basilisk_common::{config_keys, slash_commands};

struct BasiliskExtension {
    /// Cached path to the resolved binary, so we don't re-resolve every call.
    cached_binary_path: Option<String>,
}

impl BasiliskExtension {
    /// Resolve the basilisk binary.
    ///
    /// In the WASM sandbox, `std::fs::metadata` cannot access the host
    /// filesystem, so we cannot probe well-known paths. Instead:
    /// 1. Check user-configured path from Zed LSP settings
    /// 2. Check the `BASILISK_PATH` environment variable
    /// 3. Return bare `"basilisk"` and let Zed resolve it from PATH
    fn resolve_binary(
        &mut self,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        // Return cached path if we already resolved it.
        if let Some(ref path) = self.cached_binary_path {
            return Ok(path.clone());
        }

        // 1. Check user-configured binary path from Zed LSP settings.
        if let Some(settings) = zed::settings::LspSettings::for_worktree("basilisk", worktree).ok()
        {
            if let Some(binary) = settings.binary.as_ref() {
                if let Some(ref path) = binary.path {
                    self.cached_binary_path = Some(path.clone());
                    return Ok(path.clone());
                }
            }
        }

        // 2. Check BASILISK_PATH environment variable.
        if let Some(path) = Self::env_configured_path(worktree) {
            self.cached_binary_path = Some(path.clone());
            return Ok(path);
        }

        // 3. Try bare "basilisk" — Zed will resolve it from the shell PATH.
        //    This works when `cargo install --path crates/basilisk-cli` has been run.
        self.cached_binary_path = Some("basilisk".to_string());
        Ok("basilisk".to_string())
    }

    /// Read binary path from BASILISK_PATH environment variable.
    fn env_configured_path(worktree: &zed::Worktree) -> Option<String> {
        worktree
            .shell_env()
            .into_iter()
            .find(|(key, _)| key == "BASILISK_PATH")
            .map(|(_, value)| value)
    }

    /// Build a `SlashCommandOutput` with a single labeled section.
    fn slash_output(label: &str, text: String) -> zed::SlashCommandOutput {
        zed::SlashCommandOutput {
            sections: vec![zed::SlashCommandOutputSection {
                range: (0..text.len()).into(),
                label: label.to_string(),
            }],
            text,
        }
    }
}

impl zed::Extension for BasiliskExtension {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary_path = self.resolve_binary(worktree)?;

        Ok(zed::Command {
            command: binary_path,
            args: vec!["lsp".into()],
            env: Default::default(),
        })
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(Some(serde_json::json!({
            "workspaceRoot": worktree.root_path(),
        })))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<Option<serde_json::Value>> {
        // Read settings from Zed's LSP configuration for basilisk.
        let settings = zed::settings::LspSettings::for_worktree(config_keys::ROOT, worktree)
            .ok()
            .and_then(|s| s.settings);

        // If user has settings, pass them through. Otherwise use sensible defaults.
        let config = settings.unwrap_or_else(|| {
            serde_json::json!({
                config_keys::INLAY_HINTS: {
                    config_keys::PARAM_NAMES: true,
                    config_keys::VAR_TYPES: true
                },
                config_keys::RUFF: {
                    config_keys::RUFF_ENABLED: true
                }
            })
        });

        Ok(Some(serde_json::json!({
            config_keys::ROOT: config
        })))
    }

    fn run_slash_command(
        &self,
        command: zed::SlashCommand,
        args: Vec<String>,
        _worktree: Option<&zed::Worktree>,
    ) -> Result<zed::SlashCommandOutput> {
        match command.name.as_str() {
            slash_commands::PROFILE => {
                let text = if let Some(pid) = args.first() {
                    format!("Profiling PID {pid}")
                } else {
                    "Profiling active Python process".to_string()
                };
                Ok(Self::slash_output("Profile Started", text))
            }
            slash_commands::PROFSTOP => Ok(Self::slash_output(
                "Profile Results",
                "Profiling stopped. Results sent via LSP diagnostics.".into(),
            )),
            slash_commands::PROFSNAPSHOT => Ok(Self::slash_output(
                "Profile Snapshot",
                "Profiling snapshot captured. Results sent via LSP diagnostics. Profiling continues.".into(),
            )),
            slash_commands::MEMLEAK => Ok(Self::slash_output(
                "Memory Tracking",
                "Memory leak tracking started.".into(),
            )),
            slash_commands::MEMSTOP => Ok(Self::slash_output(
                "Memory Report",
                "Memory tracking stopped. Leak report sent via LSP diagnostics.".into(),
            )),
            slash_commands::MEMREFS => {
                let type_name = args.first().map_or("(unknown)", String::as_str);
                Ok(Self::slash_output(
                    "Reference Graph",
                    format!("Querying retention paths for `{type_name}`..."),
                ))
            }
            _ => Err(format!("Unknown slash command: {}", command.name).into()),
        }
    }

    fn complete_slash_command_argument(
        &self,
        command: zed::SlashCommand,
        _args: Vec<String>,
    ) -> Result<Vec<zed::SlashCommandArgumentCompletion>> {
        match command.name.as_str() {
            slash_commands::PROFILE => {
                // In WASM we can't enumerate processes — return a placeholder hint.
                Ok(vec![zed::SlashCommandArgumentCompletion {
                    label: "<pid>".to_string(),
                    new_text: String::new(),
                    run_command: false,
                }])
            }
            slash_commands::MEMREFS => {
                // Suggest common Python types.
                Ok(["DataFrame", "dict", "list", "set", "ndarray", "Tensor"]
                    .iter()
                    .map(|t| zed::SlashCommandArgumentCompletion {
                        label: (*t).to_string(),
                        new_text: (*t).to_string(),
                        run_command: true,
                    })
                    .collect())
            }
            _ => Ok(vec![]),
        }
    }
}

zed::register_extension!(BasiliskExtension);
