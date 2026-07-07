//! Implements [LSPARCH-CMDS]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-CMDS
//! Shared constants and types for Basilisk.
//!
//! This crate has **zero dependencies** so it compiles to both native targets
//! and `wasm32-wasip1` (required by the Zed extension). Any constant or type
//! that appears in more than one crate or editor extension belongs here.

pub mod datetime;
pub mod fs;
pub mod text;

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
    /// Fix all auto-fixable diagnostics in the current file (safe fixes only,
    /// the [AUTOFIX-CLASSIFY] default tier).
    pub const FIX_FILE: &str = "basilisk.fixFile";
    /// Fix all auto-fixable diagnostics in the current file, including Unsafe
    /// fixes — the explicit all-tier variant promised by [AUTOFIX-MASS-VSCODE].
    pub const FIX_FILE_ALL: &str = "basilisk.fixFileAll";
    /// Fix all auto-fixable diagnostics across the entire workspace (safe
    /// fixes only, the [AUTOFIX-CLASSIFY] default tier).
    pub const FIX_WORKSPACE: &str = "basilisk.fixWorkspace";
    /// Fix all auto-fixable diagnostics across the entire workspace, including
    /// Unsafe fixes — the explicit all-tier variant of [AUTOFIX-MASS-VSCODE].
    pub const FIX_WORKSPACE_ALL: &str = "basilisk.fixWorkspaceAll";
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
    /// Scaffold a local `.pyi` stub for an untyped package under
    /// `.basilisk/stubs/` (arg: module name). Quick fix for BSK-E0152 when no
    /// published typeshed stub exists. Implements [STUBRES-CREATE-LOCAL].
    pub const STUBS_CREATE_LOCAL: &str = "basilisk.stubs.createLocal";
    /// Append a missing member (method or attribute) to a local stub
    /// (args: stub path, snippet line). Quick fix for `imports_module_attribute`. Implements
    /// [STUBRES-ADD-MEMBER].
    pub const STUBS_ADD_MEMBER: &str = "basilisk.stubs.addMember";
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

    /// Start CPU profiling a Python process.
    pub const PROFILER_START: &str = "basilisk.profiler.start";
    /// Stop CPU profiling and return results.
    pub const PROFILER_STOP: &str = "basilisk.profiler.stop";
    /// Take a profiling snapshot without stopping.
    pub const PROFILER_SNAPSHOT: &str = "basilisk.profiler.snapshot";
    /// List active profiling sessions.
    pub const PROFILER_LIST: &str = "basilisk.profiler.list";
    /// Enumerate attachable Python processes for the profiler picker (#62).
    pub const PROFILER_PROCESSES: &str = "basilisk.profiler.processes";
    /// Mint the cooperative in-process sampling script for the active debug
    /// session (leg 1 of the courier round-trip — the OOTB macOS CPU path,
    /// no task ports). See [PROFILE-COOPERATIVE].
    pub const PROFILER_COOPERATIVE_SCRIPT: &str = "basilisk.profiler.cooperativeScript";
    /// Adopt the cooperative sample stream after the editor injected the
    /// script (leg 2); returns the same shape as `basilisk.profiler.start`.
    pub const PROFILER_COOPERATIVE_ATTACH: &str = "basilisk.profiler.cooperativeAttach";

    /// Start memory tracking in the active debug session.
    pub const MEMORY_START: &str = "basilisk.memory.start";
    /// Take a memory allocation snapshot.
    pub const MEMORY_SNAPSHOT: &str = "basilisk.memory.snapshot";
    /// Compare two memory snapshots to find leaks.
    pub const MEMORY_DIFF: &str = "basilisk.memory.diff";
    /// Walk the reference graph for a specific object type.
    pub const MEMORY_REFERENCES: &str = "basilisk.memory.references";
    /// List objects of a given type with sizes and reference counts.
    pub const MEMORY_OBJECTS_BY_TYPE: &str = "basilisk.memory.objectsByType";
    /// Force garbage collection and report what was collected.
    pub const MEMORY_GC_COLLECT: &str = "basilisk.memory.gcCollect";
    /// Ingest the raw output of a memory injection script run by the editor in
    /// the active debug session. The marker in the output (`__BASILISK_MEM__*`)
    /// selects the parser; the LSP updates session state, publishes memory
    /// diagnostics, and returns the structured result. This is the second leg
    /// of the editor-as-courier round-trip (the LSP holds no DAP connection).
    pub const MEMORY_INGEST: &str = "basilisk.memory.ingest";

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
        FIX_FILE_ALL,
        FIX_WORKSPACE,
        FIX_WORKSPACE_ALL,
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
        STUBS_CREATE_LOCAL,
        STUBS_ADD_MEMBER,
        DISCOVER_TESTS,
        RUN_TESTS,
        RUN_TEST_FILE,
        DEBUG_TEST,
        RUN_TESTS_COVERAGE,
        WORKSPACE_MODULES,
        TYPE_HEALTH,
        PROFILER_START,
        PROFILER_STOP,
        PROFILER_SNAPSHOT,
        PROFILER_LIST,
        PROFILER_PROCESSES,
        PROFILER_COOPERATIVE_SCRIPT,
        PROFILER_COOPERATIVE_ATTACH,
        MEMORY_START,
        MEMORY_SNAPSHOT,
        MEMORY_DIFF,
        MEMORY_REFERENCES,
        MEMORY_OBJECTS_BY_TYPE,
        MEMORY_GC_COLLECT,
        MEMORY_INGEST,
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

    /// CPU profiler configuration section.
    pub const PROFILER: &str = "profiler";
    /// Enable CPU profiling features.
    pub const PROFILER_ENABLED: &str = "enabled";
    /// Samples per second (Hz). Default: 100.
    pub const PROFILER_SAMPLE_RATE: &str = "sampleRate";
    /// Preset name: see [`crate::profiler_presets`].
    pub const PROFILER_PRESET: &str = "preset";
    /// Include native (C extension) frames in profiles.
    pub const PROFILER_INCLUDE_NATIVE: &str = "includeNative";
    /// Per-line hotspot threshold as a fraction (0.0–1.0). Default: 0.01 (1%).
    pub const PROFILER_LINE_THRESHOLD: &str = "lineThreshold";
    /// Per-function hotspot threshold as a fraction (0.0–1.0). Default: 0.02 (2%).
    pub const PROFILER_FUNC_THRESHOLD: &str = "funcThreshold";
    /// Maximum number of hotspot diagnostics per file. Default: 20.
    pub const PROFILER_MAX_DIAGNOSTICS: &str = "maxDiagnostics";
    /// Automatically start profiling when a debug session launches.
    pub const PROFILER_AUTO_ON_LAUNCH: &str = "autoOnLaunch";
    /// Default export format: `"speedscope"`, `"flamegraph"`, or `"summary"`.
    pub const PROFILER_DEFAULT_FORMAT: &str = "defaultFormat";

    /// Memory profiler configuration section.
    pub const MEMORY: &str = "memory";
    /// Tracemalloc traceback depth for allocation sites. Default: 25.
    pub const MEMORY_TRACEBACK_DEPTH: &str = "tracebackDepth";
    /// Seconds between automatic memory snapshots (0 = disabled). Default: 0.
    pub const MEMORY_AUTO_SNAPSHOT_INTERVAL: &str = "autoSnapshotInterval";
    /// Maximum number of memory allocation diagnostic entries per file. Default: 10.
    pub const MEMORY_MAX_DIAGNOSTICS: &str = "maxDiagnostics";
}

/// Custom LSP notification method names for coverage.
pub mod coverage_notifications {
    /// Notification sent with coverage data after a test run with `--cov`.
    pub const COVERAGE_RESULT: &str = "basilisk/coverageResult";
}

/// GitHub release asset naming for binary distribution.
///
/// The shapes here are the single source of truth for how editor extensions
/// locate, download, and unpack the binaries published by the release pipeline
/// (`.github/workflows/release.yml`). They MUST track that workflow exactly —
/// the Zed extension downloads a release asset, so any drift breaks zero-config
/// install. Implements the asset side of [ZED-DIST].
pub mod release {
    /// GitHub owner/repo for release downloads.
    pub const GITHUB_REPO: &str = "Nimblesite/Basilisk";

    /// OS token used in release asset names for macOS.
    const MACOS_OS: &str = "apple-darwin";

    /// Profiler helper binary name (bundled alongside `basilisk` on macOS).
    pub const PROFILER_HELPER: &str = "basilisk-profiler-helper";

    /// Parent directory embedded in the macOS release archive.
    ///
    /// The macOS archive is built with `ditto -c -k --keepParent` over a
    /// staging directory named `basilisk-darwin`, so every entry is prefixed
    /// with this directory. The Linux (`tar`) and Windows (`Compress-Archive`)
    /// archives are flat. Consumers that unpack the macOS zip must descend into
    /// this directory to reach the binary. Mirrors `release.yml`.
    pub const MACOS_ARCHIVE_DIR: &str = "basilisk-darwin";

    /// Well-known filesystem locations where the basilisk binary might live.
    pub const WELL_KNOWN_PATHS: &[&str] = &[
        "~/.cargo/bin/basilisk",
        "/usr/local/bin/basilisk",
        "/opt/homebrew/bin/basilisk",
    ];

    /// Whether the release archive for a platform is a zip (vs. a gzipped tar).
    ///
    /// macOS (`ditto` zip) and Windows (`Compress-Archive` zip) ship `.zip`;
    /// Linux (`tar czf`) ships `.tar.gz`. Mirrors `release.yml`.
    ///
    /// # Examples
    /// ```
    /// # use basilisk_common::release::is_zip_archive;
    /// assert!(is_zip_archive("apple-darwin", false));
    /// assert!(is_zip_archive("pc-windows-msvc", true));
    /// assert!(!is_zip_archive("unknown-linux-gnu", false));
    /// ```
    #[must_use]
    pub fn is_zip_archive(os: &str, is_windows: bool) -> bool {
        is_windows || os == MACOS_OS
    }

    /// Build a release asset filename from OS and architecture strings.
    ///
    /// # Examples
    /// ```
    /// # use basilisk_common::release::asset_name;
    /// assert_eq!(
    ///     asset_name("apple-darwin", "aarch64", false),
    ///     "basilisk-aarch64-apple-darwin.zip"
    /// );
    /// assert_eq!(
    ///     asset_name("unknown-linux-gnu", "x86_64", false),
    ///     "basilisk-x86_64-unknown-linux-gnu.tar.gz"
    /// );
    /// assert_eq!(
    ///     asset_name("pc-windows-msvc", "x86_64", true),
    ///     "basilisk-x86_64-pc-windows-msvc.zip"
    /// );
    /// ```
    #[must_use]
    pub fn asset_name(os: &str, arch: &str, is_windows: bool) -> String {
        let ext = if is_zip_archive(os, is_windows) {
            "zip"
        } else {
            "tar.gz"
        };
        format!("basilisk-{arch}-{os}.{ext}")
    }

    /// Path to the extracted binary relative to the download directory.
    ///
    /// The macOS zip nests entries under [`MACOS_ARCHIVE_DIR`]; the Linux and
    /// Windows archives are flat. `binary_name` is `basilisk` (POSIX) or
    /// `basilisk.exe` (Windows). Used to locate either the main binary or the
    /// profiler helper after unpacking.
    ///
    /// # Examples
    /// ```
    /// # use basilisk_common::release::extracted_binary_path;
    /// assert_eq!(
    ///     extracted_binary_path("basilisk", "apple-darwin"),
    ///     "basilisk-darwin/basilisk"
    /// );
    /// assert_eq!(extracted_binary_path("basilisk", "unknown-linux-gnu"), "basilisk");
    /// assert_eq!(extracted_binary_path("basilisk.exe", "pc-windows-msvc"), "basilisk.exe");
    /// ```
    #[must_use]
    pub fn extracted_binary_path(binary_name: &str, os: &str) -> String {
        if os == MACOS_OS {
            format!("{MACOS_ARCHIVE_DIR}/{binary_name}")
        } else {
            binary_name.to_string()
        }
    }
}

/// Custom LSP notification method names.
pub mod notifications {
    /// Notification sent when a module's symbol table changes after re-analysis.
    pub const MODULE_CHANGED: &str = "basilisk/moduleChanged";
    /// Notification sent when a workspace scan finishes, so panels can settle
    /// their loading state even when the scan published nothing
    /// ([EXTACT-MODULES-HEADER-LOADING], GitHub #144).
    pub const SCAN_COMPLETE: &str = "basilisk/scanComplete";
    /// Periodic profiling progress update during active sessions.
    pub const PROFILER_PROGRESS: &str = "basilisk/profiler/progress";
    /// Memory timeline data from auto-snapshot mode.
    pub const MEMORY_TIMELINE: &str = "basilisk/memory/timeline";
}

/// Diagnostic code prefixes for profiling features.
pub mod profiler_diagnostics {
    /// Hot line — above the configured CPU sample threshold.
    pub const LINE: &str = "BSK-PROF-LINE";
    /// Hot function — above the configured function threshold.
    pub const FUNC: &str = "BSK-PROF-FUNC";
    /// GIL contention detected on a thread.
    pub const GIL: &str = "BSK-PROF-GIL";
}

/// Diagnostic code prefixes for memory profiling features.
pub mod memory_diagnostics {
    /// Memory allocation hotspot — high allocation volume at a source line.
    pub const ALLOC: &str = "BSK-MEM-ALLOC";
    /// Memory growth — allocation size increased between snapshots.
    pub const GROWTH: &str = "BSK-MEM-GROWTH";
    /// Suspected memory leak — consistent growth across multiple snapshots.
    pub const LEAK: &str = "BSK-MEM-LEAK";
    /// Reference cycle detected — objects with `__del__` in a cycle (uncollectable).
    pub const CYCLE: &str = "BSK-MEM-CYCLE";
}

/// Profiler export format names and sampling preset names.
///
/// These string values are used in LSP request parameters (`basilisk.profiler.stop`
/// `format` field) and workspace configuration (`profiler.preset`). Both the LSP
/// server and editor extensions share these constants so there is a single source
/// of truth for the allowed values.
pub mod profiler_formats {
    /// Speedscope JSON format — the default. Opens in speedscope.app.
    pub const SPEEDSCOPE: &str = "speedscope";
    /// Flamegraph SVG via the inferno crate.
    pub const FLAMEGRAPH: &str = "flamegraph";
    /// Text summary only — no file written to disk.
    pub const SUMMARY: &str = "summary";

    /// All valid format strings, in order of preference.
    pub const ALL: &[&str] = &[SPEEDSCOPE, FLAMEGRAPH, SUMMARY];
}

/// Profiler preset names used in `basilisk.profiler.start` and workspace config.
///
/// Must mirror `ProfilingPreset::parse_name` in the LSP — advertising a name
/// the server does not parse silently degrades to a default CPU session.
pub mod profiler_presets {
    /// Short burst: 10 seconds at 100 Hz, for quick hotspot checks.
    pub const QUICK: &str = "quick";
    /// Thorough analysis: 60 seconds at 200 Hz, higher-fidelity data.
    pub const DETAILED: &str = "detailed";
    /// No time limit at 50 Hz, for long-running servers and batch jobs.
    pub const LONG_RUNNING: &str = "longRunning";
    /// Not a preset: use the user-tuned `sampleRate`/`includeNative` settings.
    pub const DEFAULT: &str = "default";

    /// All preset names the server parses (excludes the `default` sentinel).
    pub const ALL: &[&str] = &[QUICK, DETAILED, LONG_RUNNING];
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
