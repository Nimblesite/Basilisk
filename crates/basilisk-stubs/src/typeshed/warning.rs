//! Implements [STUBRES-TYPESHED-WARN]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN
//!
//! Composable typeshed source-status warnings.
//!
//! The typing specification defines resolution order, not source status, so
//! these warnings are **status, never Python diagnostics** — CLI prints them on
//! a stderr banner, the LSP surfaces them through `window/showMessage` plus
//! Service Info (never `publishDiagnostics`), and MCP returns them as structured
//! fields. They therefore cannot create conformance false positives
//! ([STUBRES-TYPESHED-WARN]).
//!
//! Warnings **compose**: a custom folder is simultaneously `UNPINNED` and
//! `USER-MANAGED SOURCE`, so the report carries an ordered list rather than one
//! enum. A missing or corrupt pinned source is NOT a warning — it is the
//! terminal `NO SOURCE` failure and analysis does not run
//! ([STUBRES-TYPESHED-OFFLINE]).

use std::cmp::Ordering;

/// Severity of a source-status warning.
///
/// An unpinned or user-managed source is advisory (it works, it is just not
/// reproducible or not official); a blocked license change is high — analysis
/// is refusing content pending review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WarningSeverity {
    /// Informational: the source works but is not reproducible or not official.
    Advisory,
    /// Elevated: activation is blocked or analysis needs attention.
    High,
}

/// Why a source is reported as unpinned ([STUBRES-TYPESHED-WARN]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum UnpinnedKind {
    /// No explicit `typeshed-commit`: the bundled snapshot serves by default,
    /// and a build-time pin is **not** a user pin.
    BundledDefault,
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
    /// The build-approved LICENSE/NOTICE identity changed; activation was blocked
    /// pending human review ([STUBRES-TYPESHED-LICENSE]).
    LicenseChanged,
    /// A custom, user-managed source supplies its own license and contents; it is
    /// never assigned typeshed's terms ([STUBRES-CUSTOM-TYPESHED]).
    UserManaged,
}

impl TypeshedWarning {
    /// Stable machine code, matching the spec's status table heading.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unpinned(_) => "UNPINNED",
            Self::LicenseChanged => "LICENSE CHANGED",
            Self::UserManaged => "USER-MANAGED SOURCE",
        }
    }

    /// Severity of this warning.
    #[must_use]
    pub const fn severity(&self) -> WarningSeverity {
        match self {
            Self::Unpinned(_) | Self::UserManaged => WarningSeverity::Advisory,
            Self::LicenseChanged => WarningSeverity::High,
        }
    }

    /// Full human-readable status line, verbatim from the spec's status table.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Unpinned(UnpinnedKind::BundledDefault) => {
                "UNPINNED — pin a commit to make this reproducible".to_owned()
            }
            Self::Unpinned(UnpinnedKind::CustomFolder) => {
                "UNPINNED — folder contents can change; version or content-address the folder externally"
                    .to_owned()
            }
            Self::LicenseChanged => "LICENSE CHANGED — Basilisk update/review required".to_owned(),
            Self::UserManaged => {
                "USER-MANAGED SOURCE — license and contents supplied by user".to_owned()
            }
        }
    }

    /// Canonical display priority (lower shows first), matching the normative
    /// status-table order shared by CLI, LSP, and MCP.
    #[must_use]
    const fn priority(&self) -> u8 {
        match self {
            Self::Unpinned(_) => 0,
            Self::UserManaged => 1,
            Self::LicenseChanged => 2,
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
            TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault).code(),
            "UNPINNED"
        );
        assert_eq!(TypeshedWarning::LicenseChanged.code(), "LICENSE CHANGED");
        assert_eq!(TypeshedWarning::UserManaged.code(), "USER-MANAGED SOURCE");
    }

    #[test]
    fn only_license_drift_is_high_severity() {
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault).severity(),
            WarningSeverity::Advisory
        );
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder).severity(),
            WarningSeverity::Advisory
        );
        assert_eq!(
            TypeshedWarning::UserManaged.severity(),
            WarningSeverity::Advisory
        );
        assert_eq!(
            TypeshedWarning::LicenseChanged.severity(),
            WarningSeverity::High
        );
    }

    #[test]
    fn messages_are_verbatim_from_the_spec_table() {
        // Exact strings — a mutant that edits a word is caught.
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault).message(),
            "UNPINNED — pin a commit to make this reproducible"
        );
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder).message(),
            "UNPINNED — folder contents can change; version or content-address the folder externally"
        );
        assert_eq!(
            TypeshedWarning::LicenseChanged.message(),
            "LICENSE CHANGED — Basilisk update/review required"
        );
        assert_eq!(
            TypeshedWarning::UserManaged.message(),
            "USER-MANAGED SOURCE — license and contents supplied by user"
        );
    }

    #[test]
    fn canonicalize_uses_normative_status_table_order_and_dedups() {
        let mut warnings = vec![
            TypeshedWarning::LicenseChanged,
            TypeshedWarning::UserManaged,
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
            TypeshedWarning::UserManaged, // duplicate
        ];
        canonicalize(&mut warnings);
        assert_eq!(
            warnings,
            vec![
                TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
                TypeshedWarning::UserManaged,
                TypeshedWarning::LicenseChanged,
            ]
        );
    }

    #[test]
    fn custom_folder_warnings_compose() {
        // A custom folder is both unpinned and user-managed: distinct variants
        // coexist in one list rather than collapsing to a single enum.
        let mut warnings = vec![
            TypeshedWarning::UserManaged,
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
        ];
        canonicalize(&mut warnings);
        assert_eq!(warnings.len(), 2);
        assert_eq!(
            warnings.first().map(TypeshedWarning::code),
            Some("UNPINNED")
        );
    }

    #[test]
    fn serde_round_trip() {
        let warning = TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault);
        let json = serde_json::to_string(&warning);
        assert!(json.is_ok(), "serialize");
        if let Ok(text) = json {
            let back = serde_json::from_str::<TypeshedWarning>(&text);
            assert_eq!(back.ok(), Some(warning));
        }
    }
}
