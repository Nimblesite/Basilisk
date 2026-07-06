//! Implements [CHKARCH-VERSION-TARGET]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-VERSION-TARGET
//!
//! Per-run context threaded into every rule (issue #93).
//!
//! The configured `python_version` / `python_platform` flow from
//! `BasiliskConfig` (CLI: `pyproject.toml` / `basilisk.json`; LSP: the
//! `[LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]` cascade) into [`CheckContext`],
//! so rules evaluate version/platform conditionals against the *configured*
//! target instead of a hardcoded constant.

/// The single, centralized default target version (canonical Python 3.12).
pub const DEFAULT_TARGET_VERSION: (u32, u32) = (3, 12);

/// Per-run facts every rule may consult.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckContext {
    /// Target Python version as `(major, minor)`, e.g. `(3, 9)`.
    pub target_version: (u32, u32),
    /// Target platform (`"linux"`, `"darwin"`, `"win32"`), if configured.
    pub target_platform: Option<String>,
    /// Whether a custom typeshed (`typeshed-path`) is configured.
    ///
    /// When set, that directory is the canonical source for standard-library
    /// types ([STUBRES-CUSTOM-TYPESHED]): the bundled name-only stdlib set no
    /// longer rescues a module absent from it, so rules that suppress on stdlib
    /// membership must gate on
    /// [`crate::imports::bundled_stdlib_recognized`] instead of calling
    /// `is_stdlib_module` directly. Threaded here so every rule reads the same
    /// canonicality decision the resolver already applied.
    pub custom_typeshed_configured: bool,
    /// Line-start index over the module source, built once per check.
    ///
    /// Rules that need to map a byte offset to a line — or locate a function
    /// body without rescanning the whole file — share this instead of scanning
    /// `module.source` from the top on every lookup (which is O(offset) per call
    /// and O(n²) across a file). Built by [`Self::from_config_with_source`];
    /// [`Self::from_config`] / [`Self::default`] leave it empty for the focused
    /// rule tests that invoke a single rule without a real source.
    pub line_index: basilisk_common::text::LineIndex,
}

impl Default for CheckContext {
    fn default() -> Self {
        Self {
            target_version: DEFAULT_TARGET_VERSION,
            target_platform: None,
            custom_typeshed_configured: false,
            line_index: basilisk_common::text::LineIndex::default(),
        }
    }
}

impl CheckContext {
    /// Build a context from project configuration, with an empty line index.
    ///
    /// An absent or unparsable `python_version` falls back to
    /// [`DEFAULT_TARGET_VERSION`] so a malformed config behaves exactly like
    /// the default rather than panicking or disabling version gating.
    ///
    /// The full check pipeline uses [`Self::from_config_with_source`] so rules
    /// get a populated [`line_index`](Self::line_index); this variant is for
    /// callers that only read the version/platform fields.
    #[must_use]
    pub fn from_config(config: &basilisk_config::BasiliskConfig) -> Self {
        Self {
            target_version: config
                .python_version
                .as_deref()
                .and_then(parse_target_version)
                .unwrap_or(DEFAULT_TARGET_VERSION),
            target_platform: config.python_platform.clone(),
            custom_typeshed_configured: config.typeshed_path.is_some(),
            line_index: basilisk_common::text::LineIndex::default(),
        }
    }

    /// Build a context from configuration plus the module `source`, precomputing
    /// the shared [`line_index`](Self::line_index) once for every rule to reuse.
    #[must_use]
    pub fn from_config_with_source(config: &basilisk_config::BasiliskConfig, source: &str) -> Self {
        Self {
            line_index: basilisk_common::text::LineIndex::new(source),
            ..Self::from_config(config)
        }
    }
}

/// Parse `"3.9"` / `"3.12.1"` into `(major, minor)`.
fn parse_target_version(raw: &str) -> Option<(u32, u32)> {
    let mut parts = raw.trim().split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().map_or(Some(0), |m| m.parse::<u32>().ok())?;
    Some((major, minor))
}
