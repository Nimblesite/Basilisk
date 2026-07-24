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
//! Warnings **compose**: a custom folder is simultaneously `typeshed_source_unpinned` and
//! `typeshed_source_user_managed`, so the report carries an ordered list rather than one
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
    /// Stable, descriptive machine code, matching the spec's status table
    /// heading. Named like a conformance rule (`snake_case`, no numbers) so it is
    /// greppable and deep-linkable, and so `grep typeshed_source_` walks spec →
    /// code → tests in one shot ([STUBRES-TYPESHED-WARN]).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Unpinned(_) => "typeshed_source_unpinned",
            Self::LicenseChanged => "typeshed_source_license_changed",
            Self::UserManaged => "typeshed_source_user_managed",
        }
    }

    /// The canonical documentation page for this status code — the same
    /// code-addressed route every other Basilisk diagnostic deep-links to, so
    /// each surface can print `see: https://www.basilisk-python.dev/errors/<code>`
    /// ([STUBRES-TYPESHED-WARN], [WEBSITE-ERROR-PAGES]).
    #[must_use]
    pub fn docs_url(&self) -> String {
        format!("https://www.basilisk-python.dev/errors/{}", self.code())
    }

    /// Severity of this warning.
    #[must_use]
    pub const fn severity(&self) -> WarningSeverity {
        match self {
            Self::Unpinned(_) | Self::UserManaged => WarningSeverity::Advisory,
            Self::LicenseChanged => WarningSeverity::High,
        }
    }

    /// A plain-English status sentence for the human banner. The code is shown
    /// separately in the banner header (`warning[<code>]: <message>`), so the
    /// message reads as prose and never repeats the code
    /// ([STUBRES-TYPESHED-WARN]).
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Unpinned(UnpinnedKind::BundledDefault) => {
                "the typeshed stubs bundled with Basilisk are not pinned to a commit; \
                 set `typeshed-commit` to an exact SHA so type checks stay reproducible \
                 across machines and CI"
                    .to_owned()
            }
            Self::Unpinned(UnpinnedKind::CustomFolder) => {
                "the custom typeshed folder is not version-pinned, so its contents can \
                 change between runs and checks are not reproducible; version or \
                 content-address the folder externally"
                    .to_owned()
            }
            Self::LicenseChanged => {
                "the bundled typeshed's approved LICENSE/NOTICE changed and needs review; \
                 update Basilisk before relying on these stubs"
                    .to_owned()
            }
            Self::UserManaged => {
                "the custom typeshed is user-managed: you supply its license and contents, \
                 so typeshed's license terms are not applied to it"
                    .to_owned()
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
            "typeshed_source_unpinned"
        );
        assert_eq!(
            TypeshedWarning::LicenseChanged.code(),
            "typeshed_source_license_changed"
        );
        assert_eq!(
            TypeshedWarning::UserManaged.code(),
            "typeshed_source_user_managed"
        );
    }

    /// The contract this change establishes ([STUBRES-TYPESHED-WARN], issue
    /// #312): source-status advisories carry DESCRIPTIVE, greppable, number-free
    /// codes — one per condition — each deep-linking to its own `/errors/<code>`
    /// documentation page, exactly like every other diagnostic the CLI prints.
    /// Regression guard for the "these read like CLI-arg log spam" report.
    #[test]
    fn codes_are_descriptive_number_free_names_with_doc_links() {
        for warning in [
            TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault),
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
            TypeshedWarning::UserManaged,
            TypeshedWarning::LicenseChanged,
        ] {
            let code = warning.code();
            assert!(
                code.starts_with("typeshed_source_"),
                "code `{code}` must be a descriptive typeshed_source_* name"
            );
            assert!(
                code.bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_'),
                "code `{code}` must be snake_case with NO numbers or spaces"
            );
            assert_eq!(
                warning.docs_url(),
                format!("https://www.basilisk-python.dev/errors/{code}"),
                "every status code must deep-link to its own /errors/<code> page"
            );
        }
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
        // Exact strings — a mutant that edits a word is caught. Prose sentences
        // with NO leading code (the banner header carries the code separately).
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::BundledDefault).message(),
            "the typeshed stubs bundled with Basilisk are not pinned to a commit; \
             set `typeshed-commit` to an exact SHA so type checks stay reproducible \
             across machines and CI"
        );
        assert_eq!(
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder).message(),
            "the custom typeshed folder is not version-pinned, so its contents can \
             change between runs and checks are not reproducible; version or \
             content-address the folder externally"
        );
        assert_eq!(
            TypeshedWarning::LicenseChanged.message(),
            "the bundled typeshed's approved LICENSE/NOTICE changed and needs review; \
             update Basilisk before relying on these stubs"
        );
        assert_eq!(
            TypeshedWarning::UserManaged.message(),
            "the custom typeshed is user-managed: you supply its license and contents, \
             so typeshed's license terms are not applied to it"
        );
        // No message repeats its own code or leads with a SHOUTY tag.
        for warning in [
            TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
            TypeshedWarning::UserManaged,
            TypeshedWarning::LicenseChanged,
        ] {
            assert!(
                !warning.message().contains(warning.code())
                    && !warning.message().starts_with(char::is_uppercase),
                "message must be prose, not `CODE — ...`: {:?}",
                warning.message()
            );
        }
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
            Some("typeshed_source_unpinned")
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
