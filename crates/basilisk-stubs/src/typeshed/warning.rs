//! Implements [STUBRES-TYPESHED-WARN]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN
//!
//! Composable typeshed source-status warnings.
//!
//! The typing specification defines resolution order, not transport status, so
//! these warnings are **status, never Python diagnostics** — CLI prints them on
//! a stderr banner, the LSP surfaces them through `window/showMessage` plus
//! Service Info (never `publishDiagnostics`), and MCP returns them as structured
//! fields. They therefore cannot create conformance false positives
//! ([STUBRES-TYPESHED-WARN]).
//!
//! Warnings **compose**: a single source can be simultaneously `UNPINNED` and
//! `UNVERIFIED`, so the report carries an ordered list rather than one enum.

use std::cmp::Ordering;

/// Severity of a source-status warning.
///
/// Three orthogonal signals drive this: an unpinned/user-managed source is
/// advisory (it works, it is just not reproducible), whereas a bundled fallback,
/// a blocked license change, or disabled verification is high — analysis is
/// running against fallback or unattested content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WarningSeverity {
    /// Informational: the source works but is not reproducible or not official.
    Advisory,
    /// Elevated: analysis is on a fallback, blocked, or unverified source.
    High,
}

/// Why a source is reported as unpinned ([STUBRES-TYPESHED-WARN]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum UnpinnedKind {
    /// Latest `main` or the bundled snapshot — a build-time pin is **not** a user
    /// pin, so even the shipped snapshot reports unpinned until the user pins.
    LatestOrBundled,
    /// A custom `typeshed-path` folder, whose contents can change on disk.
    CustomFolder,
}

/// A single composable typeshed source-status warning ([STUBRES-TYPESHED-WARN]).
///
/// The variants mirror the spec's status table one-for-one. Each carries a
/// stable machine [`code`](TypeshedWarning::code), a [`severity`], and a
/// human-readable [`message`](TypeshedWarning::message).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TypeshedWarning {
    /// The active source carries no explicit `typeshed-commit`.
    Unpinned(UnpinnedKind),
    /// Latest could not resolve, download, or validate; the bundled snapshot is
    /// in use and may be behind upstream.
    DownloadFailed {
        /// Full SHA of the bundled snapshot now serving step 3.
        bundled_sha: String,
    },
    /// The build-approved LICENSE/NOTICE identity changed; activation was blocked
    /// pending human review ([STUBRES-TYPESHED-LICENSE]).
    LicenseChanged,
    /// Content verification was disabled, so contents were not checked against
    /// the selected tree. Never implies verified provenance.
    Unverified,
    /// A custom, user-managed source supplies its own license and contents; it is
    /// never assigned typeshed's terms ([STUBRES-CUSTOM-TYPESHED]).
    UserManaged,
}

impl TypeshedWarning {
    /// Stable machine code, matching the spec's status table heading.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unpinned(_) => "UNPINNED",
            Self::DownloadFailed { .. } => "DOWNLOAD FAILED",
            Self::LicenseChanged => "LICENSE CHANGED",
            Self::Unverified => "UNVERIFIED",
            Self::UserManaged => "USER-MANAGED SOURCE",
        }
    }

    /// Severity of this warning.
    #[must_use]
    pub fn severity(&self) -> WarningSeverity {
        match self {
            Self::Unpinned(_) | Self::UserManaged => WarningSeverity::Advisory,
            Self::DownloadFailed { .. } | Self::LicenseChanged | Self::Unverified => {
                WarningSeverity::High
            }
        }
    }

    /// Full human-readable status line, verbatim from the spec's status table.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Unpinned(UnpinnedKind::LatestOrBundled) => {
                "UNPINNED — choose the pinned-commit source to make this reproducible".to_owned()
            }
            Self::Unpinned(UnpinnedKind::CustomFolder) => {
                "UNPINNED — folder contents can change; version or content-address the folder externally"
                    .to_owned()
            }
            Self::DownloadFailed { bundled_sha } => {
                format!("DOWNLOAD FAILED — using bundled {bundled_sha}; may be behind upstream")
            }
            Self::LicenseChanged => "LICENSE CHANGED — Basilisk update/review required".to_owned(),
            Self::Unverified => {
                "UNVERIFIED — contents were not checked against the selected tree".to_owned()
            }
            Self::UserManaged => {
                "USER-MANAGED SOURCE — license and contents supplied by user".to_owned()
            }
        }
    }

    /// Canonical display priority (lower shows first), matching the normative
    /// status-table order shared by CLI, LSP, and MCP.
    #[must_use]
    fn priority(&self) -> u8 {
        match self {
            Self::Unpinned(_) => 0,
            Self::DownloadFailed { .. } => 1,
            Self::LicenseChanged => 2,
            Self::Unverified => 3,
            Self::UserManaged => 4,
        }
    }
}

/// Sort a composed warning list into canonical display order and drop exact
/// duplicates, so every surface (CLI, LSP, MCP) presents the same ordered set.
pub fn canonicalize(warnings: &mut Vec<TypeshedWarning>) {
    warnings.sort_by(order);
    warnings.dedup();
}

/// Total order over warnings: by [`priority`](TypeshedWarning::priority), then by
/// code so equal-priority variants remain deterministic.
fn order(a: &TypeshedWarning, b: &TypeshedWarning) -> Ordering {
    a.priority()
        .cmp(&b.priority())
        .then_with(|| a.code().cmp(b.code()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_spec_table() {
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled).code(),
            "UNPINNED"
        );
        assert_eq!(
            TypeshedWarning::DownloadFailed {
                bundled_sha: "abc".to_owned()
            }
            .code(),
            "DOWNLOAD FAILED"
        );
        assert_eq!(TypeshedWarning::LicenseChanged.code(), "LICENSE CHANGED");
        assert_eq!(TypeshedWarning::Unverified.code(), "UNVERIFIED");
        assert_eq!(TypeshedWarning::UserManaged.code(), "USER-MANAGED SOURCE");
    }

    #[test]
    fn severities_are_three_orthogonal_signals() {
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled).severity(),
            WarningSeverity::Advisory
        );
        assert_eq!(
            TypeshedWarning::UserManaged.severity(),
            WarningSeverity::Advisory
        );
        assert_eq!(
            TypeshedWarning::DownloadFailed {
                bundled_sha: "x".to_owned()
            }
            .severity(),
            WarningSeverity::High
        );
        assert_eq!(
            TypeshedWarning::LicenseChanged.severity(),
            WarningSeverity::High
        );
        assert_eq!(
            TypeshedWarning::Unverified.severity(),
            WarningSeverity::High
        );
    }

    #[test]
    fn messages_are_verbatim_and_carry_the_sha() {
        // Exact strings — a mutant that edits a word is caught.
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled).message(),
            "UNPINNED — choose the pinned-commit source to make this reproducible"
        );
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder).message(),
            "UNPINNED — folder contents can change; version or content-address the folder externally"
        );
        let msg = TypeshedWarning::DownloadFailed {
            bundled_sha: "83c2518".to_owned(),
        }
        .message();
        assert_eq!(
            msg,
            "DOWNLOAD FAILED — using bundled 83c2518; may be behind upstream"
        );
        assert!(msg.contains("83c2518"));
    }

    #[test]
    fn canonicalize_uses_normative_status_table_order_and_dedups() {
        let mut warnings = vec![
            TypeshedWarning::UserManaged,
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
            TypeshedWarning::Unverified,
            TypeshedWarning::LicenseChanged,
            TypeshedWarning::UserManaged, // duplicate
        ];
        canonicalize(&mut warnings);
        assert_eq!(
            warnings,
            vec![
                TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
                TypeshedWarning::LicenseChanged,
                TypeshedWarning::Unverified,
                TypeshedWarning::UserManaged,
            ]
        );
    }

    #[test]
    fn unpinned_and_unverified_compose() {
        // A single source can be both unpinned and unverified: distinct variants
        // coexist in one list rather than collapsing to a single enum.
        let mut warnings = vec![
            TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled),
            TypeshedWarning::Unverified,
        ];
        canonicalize(&mut warnings);
        assert_eq!(warnings.len(), 2);
        assert_eq!(
            warnings.first().map(TypeshedWarning::severity),
            Some(WarningSeverity::Advisory)
        );
    }

    #[test]
    fn serde_round_trip() {
        let warning = TypeshedWarning::DownloadFailed {
            bundled_sha: "deadbeef".to_owned(),
        };
        let json = serde_json::to_string(&warning);
        assert!(json.is_ok(), "serialize");
        if let Ok(text) = json {
            let back = serde_json::from_str::<TypeshedWarning>(&text);
            assert_eq!(back.ok(), Some(warning));
        }
    }
}
