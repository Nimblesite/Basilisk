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
    /// Disable a diagnostic rule in the project configuration (`pyproject.toml`).
    pub const DISABLE_RULE: &str = "basilisk.disableRule";
    /// Fix all auto-fixable diagnostics in the current file (safe fixes only).
    pub const FIX_FILE: &str = "basilisk.fixFile";
    /// Fix all auto-fixable diagnostics across the entire workspace.
    pub const FIX_WORKSPACE: &str = "basilisk.fixWorkspace";
    /// Adopt the current file — autofix + demote remaining errors to warnings.
    pub const ADOPT_FILE: &str = "basilisk.adoptFile";
    /// Adopt all files in the workspace.
    pub const ADOPT_WORKSPACE: &str = "basilisk.adoptWorkspace";
    /// Un-adopt the current file — restore full strictness.
    pub const UNADOPT_FILE: &str = "basilisk.unadoptFile";
    /// Run `uv sync` to synchronize the environment.
    pub const UV_SYNC: &str = "basilisk.uv.sync";
    /// Run `uv add <package>` to add a dependency.
    pub const UV_ADD: &str = "basilisk.uv.add";
    /// Run `uv add --dev <package>` to add a dev dependency.
    pub const UV_ADD_DEV: &str = "basilisk.uv.addDev";
    /// Run `uv remove <package>` to remove a dependency.
    pub const UV_REMOVE: &str = "basilisk.uv.remove";
    /// Run `uv lock` to update the lock file.
    pub const UV_LOCK: &str = "basilisk.uv.lock";
    /// Run `uv venv` to create a virtual environment.
    pub const UV_CREATE_ENV: &str = "basilisk.uv.createEnv";
    /// Move a symbol to an existing file (args: source URI, dest URI, symbol
    /// name, start line, end line).
    pub const MOVE_SYMBOL: &str = "basilisk.moveSymbol";
    /// Discover tests in the workspace or a specific file.
    pub const DISCOVER_TESTS: &str = "basilisk.discoverTests";
    /// Run one or more tests by node ID.
    pub const RUN_TESTS: &str = "basilisk.runTests";
    /// Run all tests in the current file.
    pub const RUN_TEST_FILE: &str = "basilisk.runTestFile";
    /// Debug a specific test by node ID.
    pub const DEBUG_TEST: &str = "basilisk.debugTest";
    /// Run tests with coverage and return coverage results.
    pub const RUN_TESTS_COVERAGE: &str = "basilisk.runTestsCoverage";
    /// Return the workspace module tree (packages, modules, symbols).
    pub const WORKSPACE_MODULES: &str = "basilisk.workspaceModules";
    /// Return type health statistics (coverage, errors, warnings per module).
    pub const TYPE_HEALTH: &str = "basilisk.typeHealth";

    /// Command names advertised via `executeCommandProvider` capabilities.
    ///
    /// **The server is the single source of truth for commands.** Every command
    /// the server can handle MUST be listed here. No editor extension (VS Code,
    /// Neovim, Zed) is allowed to pre-register these commands — the LSP client
    /// library discovers and registers them from the server's capabilities.
    ///
    /// See `LSP-ARCHITECTURE-SPEC.md` § Command Registration Rule.
    pub const ALL: &[&str] = &[
        ORGANIZE_IMPORTS,
        START_DEBUG_SESSION,
        STOP_DEBUG_SESSION,
        DISABLE_RULE,
        FIX_FILE,
        FIX_WORKSPACE,
        ADOPT_FILE,
        ADOPT_WORKSPACE,
        UNADOPT_FILE,
        UV_SYNC,
        UV_ADD,
        UV_ADD_DEV,
        UV_REMOVE,
        UV_LOCK,
        UV_CREATE_ENV,
        MOVE_SYMBOL,
        DISCOVER_TESTS,
        RUN_TESTS,
        RUN_TEST_FILE,
        DEBUG_TEST,
        RUN_TESTS_COVERAGE,
        WORKSPACE_MODULES,
        TYPE_HEALTH,
    ];
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
    /// Show workspace module tree.
    pub const MODULES: &str = "modules";
    /// Show symbols in a module.
    pub const SYMBOLS: &str = "symbols";
    /// Show type health statistics.
    pub const HEALTH: &str = "health";
    /// Show Basilisk server info.
    pub const BASILISK: &str = "basilisk";
    /// Discover tests in the workspace.
    pub const TESTS: &str = "tests";
    /// Run tests by node ID or file.
    pub const RUNTESTS: &str = "runtests";
    /// Run tests in the current file.
    pub const TESTFILE: &str = "testfile";
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

    /// uv package manager configuration section.
    pub const UV: &str = "uv";
    /// Enable/disable uv integration.
    pub const UV_ENABLED: &str = "enabled";
    /// Path to the uv executable.
    pub const UV_EXECUTABLE_PATH: &str = "executablePath";
    /// Auto-sync when pyproject.toml changes.
    pub const UV_AUTO_SYNC: &str = "autoSync";
    /// Show type stub installation suggestions.
    pub const UV_STUB_SUGGESTIONS: &str = "stubSuggestions";
    /// Show dependency hygiene diagnostics.
    pub const UV_DEPENDENCY_DIAGNOSTICS: &str = "dependencyDiagnostics";

    /// Test explorer configuration section.
    pub const TEST_EXPLORER: &str = "testExplorer";
    /// Enable/disable test discovery and execution.
    pub const TEST_EXPLORER_ENABLED: &str = "enabled";
    /// Test framework: `pytest`, `unittest`, or `auto`.
    pub const TEST_EXPLORER_FRAMEWORK: &str = "framework";
    /// Path to the pytest executable.
    pub const TEST_EXPLORER_PYTEST_PATH: &str = "pytestPath";
    /// Additional test runner arguments.
    pub const TEST_EXPLORER_ARGS: &str = "args";
    /// Re-discover tests on file save.
    pub const TEST_EXPLORER_AUTO_DISCOVER_ON_SAVE: &str = "autoDiscoverOnSave";
    /// Use `uv run` when a uv project is detected.
    pub const TEST_EXPLORER_USE_UV_RUN: &str = "useUvRun";
    /// Enable coverage gutter decorations after test runs.
    pub const TEST_EXPLORER_COVERAGE_ENABLED: &str = "coverageEnabled";
}

/// Custom LSP notification method names for coverage.
pub mod coverage_notifications {
    /// Notification sent with coverage data after a test run with `--cov`.
    pub const COVERAGE_RESULT: &str = "basilisk/coverageResult";
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

/// Custom LSP notification method names.
pub mod notifications {
    /// Notification sent when a module's symbol table changes after re-analysis.
    pub const MODULE_CHANGED: &str = "basilisk/moduleChanged";
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
