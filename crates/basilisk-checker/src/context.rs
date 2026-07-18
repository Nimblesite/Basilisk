//! Implements [CHKARCH-VERSION-TARGET]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-VERSION-TARGET
//!
//! Per-run context threaded into every rule (issue #93).
//!
//! The configured `python_version` / `python_platform` flow from
//! `BasiliskConfig` (CLI: `pyproject.toml` `[tool.basilisk]`; LSP: the
//! `[LSPUV-PYTHON-VERSION-RESOLUTION-ORDER]` cascade) into [`CheckContext`],
//! so rules evaluate version/platform conditionals against the *configured*
//! target instead of a hardcoded constant.

/// Per-run facts every rule may consult.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckContext {
    /// Target Python version as `(major, minor)`, e.g. `(3, 9)`.
    pub target_version: Option<(u32, u32)>,
    /// Target platform (`"linux"`, `"darwin"`, `"win32"`), if configured.
    pub target_platform: Option<String>,
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

impl CheckContext {
    /// Build a context from project configuration, with an empty line index.
    ///
    /// An absent or unparsable `python_version` remains unknown. Rules that
    /// require a concrete target must stay silent rather than manufacture one.
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
                .and_then(parse_target_version),
            target_platform: config.python_platform.clone(),
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

#[cfg(test)]
mod tests {
    use super::{parse_target_version, CheckContext};
    use basilisk_config::BasiliskConfig;

    /// [TYPESHEDRT-ACCEPTANCE-TARGET] "No manufactured target": with nothing
    /// configured, the checker holds NO concrete Python target — a fixed
    /// version never appears without project/interpreter evidence. Rules that
    /// need a target stay silent instead of assuming e.g. 3.12.
    #[test]
    fn default_config_manufactures_no_target() {
        let ctx = CheckContext::from_config(&BasiliskConfig::default());
        assert_eq!(
            ctx.target_version, None,
            "an unconfigured project must have no manufactured Python target"
        );
        assert_eq!(ctx.target_platform, None);
    }

    /// An explicitly configured version IS honoured (evidence exists).
    #[test]
    fn configured_version_is_used_verbatim() {
        let cfg = BasiliskConfig {
            python_version: Some("3.9".to_owned()),
            ..BasiliskConfig::default()
        };
        assert_eq!(CheckContext::from_config(&cfg).target_version, Some((3, 9)));
    }

    /// A malformed `python-version` stays unknown rather than defaulting to a
    /// manufactured target ([TYPESHEDRT-ACCEPTANCE-TARGET]).
    #[test]
    fn unparsable_version_stays_none() {
        assert_eq!(parse_target_version("not-a-version"), None);
        let cfg = BasiliskConfig {
            python_version: Some("frobnicate".to_owned()),
            ..BasiliskConfig::default()
        };
        assert_eq!(CheckContext::from_config(&cfg).target_version, None);
    }
}
