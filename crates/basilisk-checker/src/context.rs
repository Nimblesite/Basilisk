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
}

impl Default for CheckContext {
    fn default() -> Self {
        Self {
            target_version: DEFAULT_TARGET_VERSION,
            target_platform: None,
        }
    }
}

impl CheckContext {
    /// Build a context from project configuration.
    ///
    /// An absent or unparsable `python_version` falls back to
    /// [`DEFAULT_TARGET_VERSION`] so a malformed config behaves exactly like
    /// the default rather than panicking or disabling version gating.
    #[must_use]
    pub fn from_config(config: &basilisk_config::BasiliskConfig) -> Self {
        Self {
            target_version: config
                .python_version
                .as_deref()
                .and_then(parse_target_version)
                .unwrap_or(DEFAULT_TARGET_VERSION),
            target_platform: config.python_platform.clone(),
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
