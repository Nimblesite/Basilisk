//! Implements [WASM-API]. See docs/specs/WASM-SPEC.md#WASM-API
//!
//! What a caller may vary per check. Every field is optional, so `{}` is a
//! valid request.

/// Per-call options, deserialized from the `options_json` argument.
///
/// `deny_unknown_fields` makes a typo a reported error rather than a silently
/// ignored setting — a playground that quietly discarded `python_verison` would
/// answer a question the user did not ask.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckOptions {
    /// The path diagnostics are reported against. Purely a label: nothing is
    /// read from it, and no sibling module is searched beside it
    /// ([WASM-LIMITS]).
    pub path: Option<String>,
    /// Target Python version as `"MAJOR.MINOR"`, e.g. `"3.13"`.
    ///
    /// Absent means no version evidence at all — deliberately *not* a default
    /// release, matching the checker's rule that version boundaries come only
    /// from real evidence ([CHKARCH-VERSION-TARGET]). An unparseable value is
    /// treated the same as absent by the shared parser, so it can never select
    /// a wrong version.
    pub python_version: Option<String>,
}

/// The label used when the caller supplies no `path`.
pub const DEFAULT_PATH: &str = "<playground>.py";

impl CheckOptions {
    /// The path to report diagnostics against.
    #[must_use]
    pub fn path(&self) -> &str {
        self.path.as_deref().unwrap_or(DEFAULT_PATH)
    }

    /// Project configuration for this call.
    ///
    /// The default enables every PEP typing-spec rule and no house-style rule
    /// ([CHKARCH-CONFIGURATION-ONLY]), so the playground answers what a user
    /// gets out of the box. Only the target version is layered on top, and it
    /// is stored as the same `python_version` string a `pyproject.toml` would
    /// carry so that every downstream consumer parses it through the one
    /// existing code path rather than a second one here.
    #[must_use]
    pub fn to_config(&self) -> basilisk_config::BasiliskConfig {
        basilisk_config::BasiliskConfig {
            python_version: self.python_version.clone(),
            ..basilisk_config::BasiliskConfig::default()
        }
    }
}
