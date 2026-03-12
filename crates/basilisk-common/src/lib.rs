//! Shared constants and types for Basilisk.
//!
//! This crate has **zero dependencies** so it compiles to both native targets
//! and `wasm32-wasip1` (required by the Zed extension). Any constant or type
//! that appears in more than one crate or editor extension belongs here.

/// Custom LSP method names used by Basilisk.
///
/// These are the method strings registered as execute-command capabilities
/// and dispatched by the LSP server. Every editor extension must use these
/// exact strings when sending workspace/executeCommand requests.
pub mod commands {
    /// Organize imports in the active document.
    pub const ORGANIZE_IMPORTS: &str = "basilisk.organizeImports";
    /// Start a debug session (spawns debugpy, returns host:port).
    pub const START_DEBUG_SESSION: &str = "basilisk.startDebugSession";
    /// Stop an active debug session by session ID.
    pub const STOP_DEBUG_SESSION: &str = "basilisk.stopDebugSession";

    /// All registered command names, for capability advertisement.
    pub const ALL: &[&str] = &[ORGANIZE_IMPORTS, START_DEBUG_SESSION, STOP_DEBUG_SESSION];
}

/// Slash command names used in the Zed extension's AI assistant panel.
///
/// These are also used as the canonical identifiers for profiling and memory
/// analysis features across the codebase.
pub mod slash_commands {
    /// Start CPU profiling (optionally targeting a specific PID).
    pub const PROFILE: &str = "profile";
    /// Stop CPU profiling and return results.
    pub const PROFSTOP: &str = "profstop";
    /// Take a profiling snapshot without stopping.
    pub const PROFSNAPSHOT: &str = "profsnapshot";
    /// Start memory leak tracking.
    pub const MEMLEAK: &str = "memleak";
    /// Stop memory tracking and return leak report.
    pub const MEMSTOP: &str = "memstop";
    /// Query reference/retention graph for a type.
    pub const MEMREFS: &str = "memrefs";
}

/// Configuration key names shared between editor extensions and the LSP.
///
/// These appear in both VS Code's `package.json` contributes and Zed's
/// `language_server_workspace_configuration()`.
pub mod config_keys {
    /// Root key for all Basilisk settings.
    pub const ROOT: &str = "basilisk";

    /// Inlay hints configuration section.
    pub const INLAY_HINTS: &str = "inlayHints";
    /// Show parameter name hints.
    pub const PARAM_NAMES: &str = "parameterNames";
    /// Show variable type hints.
    pub const VAR_TYPES: &str = "variableTypes";

    /// Ruff integration configuration section.
    pub const RUFF: &str = "ruff";
    /// Enable/disable Ruff integration.
    pub const RUFF_ENABLED: &str = "enabled";
}

/// GitHub release asset naming for binary distribution.
pub mod release {
    /// GitHub owner/repo for release downloads.
    pub const GITHUB_REPO: &str = "MelbourneDeveloper/Basilisk";

    /// Well-known filesystem locations where the basilisk binary might live.
    pub const WELL_KNOWN_PATHS: &[&str] = &[
        "~/.cargo/bin/basilisk",
        "/usr/local/bin/basilisk",
        "/opt/homebrew/bin/basilisk",
    ];

    /// Build a release asset filename from OS and architecture strings.
    ///
    /// # Examples
    /// ```
    /// # use basilisk_common::release::asset_name;
    /// assert_eq!(
    ///     asset_name("apple-darwin", "aarch64", false),
    ///     "basilisk-aarch64-apple-darwin.tar.gz"
    /// );
    /// assert_eq!(
    ///     asset_name("pc-windows-msvc", "x86_64", true),
    ///     "basilisk-x86_64-pc-windows-msvc.zip"
    /// );
    /// ```
    #[must_use]
    pub fn asset_name(os: &str, arch: &str, is_windows: bool) -> String {
        let ext = if is_windows { "zip" } else { "tar.gz" };
        format!("basilisk-{arch}-{os}.{ext}")
    }
}

/// Diagnostic code ranges defined in the Basilisk specification.
pub mod diagnostics {
    /// Fallback documentation URL for diagnostic codes.
    pub const DOCS_URL: &str = "https://www.basilisk-python.dev";

    /// Diagnostic code prefix for all Basilisk errors.
    pub const ERROR_PREFIX: &str = "BSK-E";
    /// Diagnostic code prefix for all Basilisk warnings.
    pub const WARNING_PREFIX: &str = "BSK-W";
}
