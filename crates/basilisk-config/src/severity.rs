//! Implements [CHKARCH-STRICTNESS-SEVERITY]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-SEVERITY

/// Severity level for a rule or tag entry.
///
/// The four values an entry can state ([CHKARCH-CONFIG-MODEL]): `error`,
/// `warning`, `info`, `disabled`. The checker applies them in
/// `basilisk-checker/src/lib.rs`; `disabled` never applies to a `pep`-tagged
/// rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum RuleSeverity {
    /// Full error (blocks CI).
    Error,
    /// Downgraded to warning.
    Warning,
    /// Downgraded to informational hint.
    Info,
    /// Rule emits nothing.
    Disabled,
}

impl RuleSeverity {
    /// Parse a severity string from config.
    ///
    /// Implements [CHKARCH-STRICTNESS-SEVERITY]: maps the four severity names
    /// (plus the documented aliases `warn`/`information`/`off`/`none`).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "error" => Some(Self::Error),
            "warning" | "warn" => Some(Self::Warning),
            "info" | "information" => Some(Self::Info),
            "disabled" | "off" | "none" => Some(Self::Disabled),
            _ => None,
        }
    }

    /// Canonical lowercase spelling used in config files.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Disabled => "disabled",
        }
    }

    /// Strictness rank for tag-entry resolution ([CHKARCH-CONFIG-MODEL]):
    /// among matching tag entries the strictest severity wins,
    /// `error` > `warning` > `info` > `disabled`.
    #[must_use]
    pub const fn strictness(self) -> u8 {
        match self {
            Self::Error => 3,
            Self::Warning => 2,
            Self::Info => 1,
            Self::Disabled => 0,
        }
    }
}
