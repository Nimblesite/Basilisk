//! Basilisk extension for the Zed editor.
//!
//! Pure logic lives in [`logic`] (testable on native target).
//! This file is thin glue that bridges [`logic`] ↔ `zed_extension_api` types.
#![expect(
    missing_docs,
    reason = "zed::register_extension! generates undocumented items"
)]

mod logic;

use zed_extension_api::{self as zed, serde_json, Result};

use basilisk_common::{config_keys, release};

struct BasiliskExtension {
    /// Cached path to the resolved binary, so we don't re-resolve every call.
    cached_binary_path: Option<String>,
    /// Version of the currently resolved binary (from download dir name or "local").
    cached_binary_version: Option<String>,
}

// ── Binary resolution ────────────────────────────────────────────────────────

impl BasiliskExtension {
    /// Resolve the basilisk binary to an absolute path. Implements [ZED-DIST].
    ///
    /// Resolution order:
    /// 1. Explicit override — `binary.path` in the Zed LSP settings
    /// 2. Explicit override — the `BASILISK_PATH` environment variable
    /// 3. Default — download the matching binary from the latest GitHub release
    ///
    /// There is no filesystem default. Installing the extension alone is enough:
    /// with no explicit override, the binary is downloaded from the release, so
    /// users never install it separately (the Shipwright contract). The two
    /// overrides exist for development and for pointing at a system install.
    fn resolve_binary(&mut self, worktree: &zed::Worktree) -> Result<String> {
        if let Some(ref path) = self.cached_binary_path {
            return Ok(path.clone());
        }

        let settings_path = zed::settings::LspSettings::for_worktree("basilisk", worktree)
            .ok()
            .and_then(|settings| settings.binary)
            .and_then(|binary| binary.path);
        let env = worktree.shell_env();
        let env_path = logic::find_env_var(&env, "BASILISK_PATH");

        if let Some(path) = logic::resolve_binary_override(settings_path.as_deref(), env_path) {
            self.cached_binary_path = Some(path.clone());
            return Ok(path);
        }

        // Default: download from the latest GitHub release (zero-config install).
        let (path, version) = Self::download_binary()?;
        self.cached_binary_path = Some(path.clone());
        self.cached_binary_version = Some(version);
        Ok(path)
    }

    /// Check if a newer version is available and log a warning if so.
    ///
    /// Non-fatal — if the check fails we silently continue.
    fn check_for_updates(&self) {
        let current = match &self.cached_binary_version {
            Some(v) => v.as_str(),
            None => return,
        };

        if let Ok(latest_release) = zed::latest_github_release(
            release::GITHUB_REPO,
            zed::GithubReleaseOptions {
                require_assets: false,
                pre_release: false,
            },
        ) {
            if logic::is_newer_version(current, &latest_release.version) {
                eprintln!(
                    "[basilisk] Update available: {} → {} (restart Zed to upgrade)",
                    current, latest_release.version
                );
            }
        }
    }

    /// Download the basilisk binary from the latest GitHub release.
    fn download_binary() -> Result<(String, String)> {
        let release = zed::latest_github_release(
            release::GITHUB_REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (platform, arch) = zed::current_platform();

        let (os_str, is_windows) = match platform {
            zed::Os::Mac => ("apple-darwin", false),
            zed::Os::Linux => ("unknown-linux-gnu", false),
            zed::Os::Windows => ("pc-windows-msvc", true),
        };
        let is_mac = matches!(platform, zed::Os::Mac);

        let arch_str = match arch {
            zed::Architecture::Aarch64 => "aarch64",
            zed::Architecture::X8664 => "x86_64",
            zed::Architecture::X86 => {
                return Err("32-bit x86 is not supported".into());
            }
        };

        let expected_asset = release::asset_name(os_str, arch_str, is_windows);

        let asset = release
            .assets
            .iter()
            .find(|a| a.name == expected_asset)
            .ok_or_else(|| {
                format!(
                    "No release asset found for {expected_asset} in {}",
                    release.version
                )
            })?;

        let binary_name = if is_windows {
            "basilisk.exe"
        } else {
            "basilisk"
        };

        // The archive type and the binary's location inside it are
        // platform-specific (macOS nests under basilisk-darwin/, Linux/Windows
        // are flat) — both derived from the same single source of truth as the
        // asset name so they can never drift from release.yml.
        let download_dir = format!("basilisk-{}", release.version);
        let binary_path = format!(
            "{download_dir}/{}",
            release::extracted_binary_path(binary_name, os_str)
        );

        // Only download if the binary isn't already cached in the extension dir.
        if std::fs::metadata(&binary_path).is_err() {
            let file_type = if release::is_zip_archive(os_str, is_windows) {
                zed::DownloadedFileType::Zip
            } else {
                zed::DownloadedFileType::GzipTar
            };
            zed::download_file(&asset.download_url, &download_dir, file_type)
                .map_err(|err| format!("Failed to download basilisk: {err}"))?;

            zed::make_file_executable(&binary_path)
                .map_err(|err| format!("Failed to make basilisk executable: {err}"))?;

            // Zed's zip extraction drops the Unix exec bit, and the macOS
            // archive also carries the profiler helper next to the binary —
            // restore its bit so profiling works without a separate install.
            if is_mac {
                let helper = format!(
                    "{download_dir}/{}",
                    release::extracted_binary_path(release::PROFILER_HELPER, os_str)
                );
                zed::make_file_executable(&helper)
                    .map_err(|err| format!("Failed to make profiler helper executable: {err}"))?;
            }
        }

        Ok((binary_path, release.version))
    }

    /// Build a `SlashCommandOutput` from a `(label, text)` pair.
    fn slash_output(label: String, text: String) -> zed::SlashCommandOutput {
        zed::SlashCommandOutput {
            sections: vec![zed::SlashCommandOutputSection {
                range: (0..text.len()).into(),
                label,
            }],
            text,
        }
    }
}

// ── Extension trait ──────────────────────────────────────────────────────────

// The `zed::Extension` impl + `register_extension!` below is the extension
// entry point. Implements [ZED-LIBRS].
impl zed::Extension for BasiliskExtension {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            cached_binary_path: None,
            cached_binary_version: None,
        }
    }

    // Launch `basilisk lsp` and pass workspace root + mapped settings — all 21
    // LSP features then flow through Zed's built-in client. Implements [ZED-LSP].
    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let binary_path = self.resolve_binary(worktree)?;
        self.check_for_updates();
        Ok(zed::Command {
            command: binary_path,
            args: vec!["lsp".into()],
            env: Vec::new(),
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
        let settings = zed::settings::LspSettings::for_worktree(config_keys::ROOT, worktree)
            .ok()
            .and_then(|s| s.settings);

        let config = settings.unwrap_or_else(logic::default_workspace_config);
        Ok(Some(logic::wrap_config(&config)))
    }

    // Profiling / memory / activity-panel slash commands. Implements [ZED-PROFILE].
    fn run_slash_command(
        &self,
        command: zed::SlashCommand,
        args: Vec<String>,
        _worktree: Option<&zed::Worktree>,
    ) -> Result<zed::SlashCommandOutput> {
        let (label, text) = logic::slash_command_output(&command.name, &args)?;
        Ok(Self::slash_output(label, text))
    }

    fn complete_slash_command_argument(
        &self,
        command: zed::SlashCommand,
        _args: Vec<String>,
    ) -> Result<Vec<zed::SlashCommandArgumentCompletion>> {
        Ok(logic::slash_completions(&command.name)
            .into_iter()
            .map(
                |(label, new_text, run_command)| zed::SlashCommandArgumentCompletion {
                    label,
                    new_text,
                    run_command,
                },
            )
            .collect())
    }

    // DAP integration: resolve the basilisk binary and hand Zed a
    // launch/attach config for `basilisk debug-adapter`. Implements [ZED-DAP].
    fn get_dap_binary(
        &mut self,
        _adapter_name: String,
        config: zed::DebugTaskDefinition,
        user_provided_debug_adapter_path: Option<String>,
        worktree: &zed::Worktree,
    ) -> core::result::Result<zed::DebugAdapterBinary, String> {
        let binary_path = match user_provided_debug_adapter_path {
            Some(path) => path,
            None => self.resolve_binary(worktree)?,
        };

        let adapter_config: serde_json::Value =
            serde_json::from_str(&config.config).unwrap_or_default();

        let dap_config = logic::build_dap_config(&adapter_config);

        let request = if logic::is_attach_request(&adapter_config)? {
            zed::StartDebuggingRequestArgumentsRequest::Attach
        } else {
            zed::StartDebuggingRequestArgumentsRequest::Launch
        };

        Ok(zed::DebugAdapterBinary {
            command: Some(binary_path),
            arguments: vec!["debug-adapter".into()],
            envs: Vec::new(),
            cwd: adapter_config
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .map(String::from),
            connection: None,
            request_args: zed::StartDebuggingRequestArguments {
                configuration: dap_config.to_string(),
                request,
            },
        })
    }

    fn dap_request_kind(
        &mut self,
        _adapter_name: String,
        config: serde_json::Value,
    ) -> core::result::Result<zed::StartDebuggingRequestArgumentsRequest, String> {
        if logic::is_attach_request(&config)? {
            Ok(zed::StartDebuggingRequestArgumentsRequest::Attach)
        } else {
            Ok(zed::StartDebuggingRequestArgumentsRequest::Launch)
        }
    }

    fn dap_config_to_scenario(
        &mut self,
        config: zed::DebugConfig,
    ) -> core::result::Result<zed::DebugScenario, String> {
        let adapter_config = match &config.request {
            zed::DebugRequest::Launch(launch) => logic::build_launch_scenario(
                &launch.program,
                &launch.args,
                launch.cwd.as_deref(),
                config.stop_on_entry.unwrap_or(false),
            ),
            zed::DebugRequest::Attach(attach) => logic::build_attach_scenario(attach.process_id),
        };

        Ok(zed::DebugScenario {
            label: config.label,
            adapter: "basilisk-debug".to_string(),
            build: None,
            config: adapter_config.to_string(),
            tcp_connection: None,
        })
    }
}

zed::register_extension!(BasiliskExtension);
