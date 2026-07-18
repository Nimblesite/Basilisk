//! Implements [STUBRES-TYPESHED-WARN] source status. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-WARN
//!
//! The public source-selection, identity, and serializable status surface.
//!
//! `basilisk-stubs` never depends on `basilisk-config`: the CLI/LSP build a
//! config-free [`TypeshedRequest`] from `[tool.basilisk]` and hand it to the
//! acquisition orchestrator. The orchestrator produces a [`SourceIdentity`]
//! (shared by status, cache fingerprinting, and the VFS) and a
//! [`TypeshedStatus`] reported verbatim by every surface — CLI banner, LSP
//! Service Info, and MCP. Status is **never a Python diagnostic**, so it can
//! never create a conformance false positive ([STUBRES-TYPESHED-WARN]).

use serde::Serialize;

use super::gittree::Oid;
use super::warning::{canonicalize, TypeshedWarning, WarningSeverity};

/// Which step-3 source is active ([STUBRES-TYPESHED]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    /// A custom `typeshed-path` folder (user-managed).
    Custom,
    /// An explicit `typeshed-commit` archive (reproducible).
    ExactCommit,
    /// The latest `python/typeshed@main` archive (unpinned).
    Latest,
    /// The bundled offline snapshot (unpinned unless it equals a user pin).
    Bundled,
}

impl SourceKind {
    /// A stable lowercase identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Custom => "custom",
            Self::ExactCommit => "exact-commit",
            Self::Latest => "latest",
            Self::Bundled => "bundled",
        }
    }
}

/// How the active source's bytes were obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    /// A user-managed filesystem tree selected by `typeshed-path`.
    CustomPath,
    /// The build-vetted ZIP embedded in the Basilisk artifact.
    EmbeddedZip,
    /// GitHub codeload archive over HTTPS.
    Codeload,
    /// A configured `typeshed-url` `{sha}` archive mirror over HTTPS.
    Mirror,
}

/// The license standing of the active source ([STUBRES-TYPESHED-LICENSE]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LicenseStatus {
    /// The approved legal-file identity matched.
    Approved,
    /// The approved identity drifted; activation was blocked.
    Changed,
    /// No license reference is available (custom source, `not supplied`).
    NotSupplied,
}

/// The provenance strength behind the active source.
///
/// Integrity (bytes match a SHA) is never authenticity (the SHA is an official
/// typeshed release): no typeshed release signature exists, so official
/// provenance ultimately rests on GitHub/TLS ([STUBRES-TYPESHED-ACQUIRE]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    /// Content attested against a tree bound to a commit by GitHub/TLS metadata.
    /// There is no signed typeshed release, so authenticity rests on GitHub/TLS.
    GithubTlsAttested,
    /// Content was not attested against the selected tree.
    Unverified,
    /// A build-vetted bundled snapshot (verified by embedded ZIP digest + manifest).
    BundleVetted,
    /// A user-managed custom source; Basilisk attests nothing about it.
    UserManaged,
}

/// The immutable identity of the active step-3 source, shared by status, cache
/// fingerprinting, and the VFS. Two sources are the same iff their identity is
/// equal, so it is a valid cache key and a safe URI component.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum SourceIdentity {
    /// An explicit or resolved commit, identified by its full commit SHA.
    Commit {
        /// The full 40-hex commit SHA.
        commit: Oid,
        /// Whether the user pinned it (`true`) or it was resolved from `main`.
        pinned: bool,
    },
    /// The bundled snapshot, identified by its build-time commit SHA.
    Bundled {
        /// The bundle's build-time commit SHA.
        commit: Oid,
    },
    /// A custom `typeshed-path` folder, identified by a digest of its resolved
    /// path — never the raw path, which must never become URI syntax.
    Custom {
        /// A digest (e.g. SHA-256 hex) of the resolved custom path.
        digest: String,
    },
}

impl SourceIdentity {
    /// A safe, opaque URI component for the archive VFS — a SHA or path digest,
    /// never a raw filesystem path.
    #[must_use]
    pub fn uri_component(&self) -> String {
        match self {
            Self::Commit { commit, .. } => commit.to_hex(),
            Self::Bundled { commit } => format!("bundled-{}", commit.to_hex()),
            Self::Custom { digest } => format!("custom-{digest}"),
        }
    }

    /// The commit SHA behind this identity, if any (a custom folder has none).
    #[must_use]
    pub const fn commit(&self) -> Option<Oid> {
        match self {
            Self::Commit { commit, .. } | Self::Bundled { commit } => Some(*commit),
            Self::Custom { .. } => None,
        }
    }

    /// Whether the user explicitly pinned this source. Only an explicit
    /// `typeshed-commit` suppresses the `UNPINNED` advisory.
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        matches!(self, Self::Commit { pinned: true, .. })
    }
}

/// Which source the user configured, free of any `basilisk-config` type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSelection {
    /// A custom `typeshed-path` folder (resolved, absolute).
    Custom {
        /// The resolved custom path.
        path: String,
    },
    /// An explicit `typeshed-commit`.
    ExactCommit {
        /// The pinned full commit SHA.
        commit: Oid,
    },
    /// No pin — resolve and download the latest `main`.
    Latest,
}

/// A config-free acquisition request the CLI/LSP builds from `[tool.basilisk]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeshedRequest {
    /// Which source to use.
    pub selection: SourceSelection,
    /// Run the Content gate (`false` waives only content attestation).
    pub verify_content: bool,
    /// Reuse the on-disk immutable-ZIP cache.
    pub use_cache: bool,
    /// A `typeshed-url` `{sha}` archive-mirror template, if configured.
    pub url_template: Option<String>,
}

/// A warning projected into `{code, message, severity}` for machine surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusWarning {
    /// Stable machine code (e.g. `UNPINNED`).
    pub code: String,
    /// Full human-readable status line.
    pub message: String,
    /// Warning severity.
    pub severity: WarningSeverity,
}

impl From<&TypeshedWarning> for StatusWarning {
    fn from(warning: &TypeshedWarning) -> Self {
        Self {
            code: warning.code().to_owned(),
            message: warning.message(),
            severity: warning.severity(),
        }
    }
}

impl StatusWarning {
    /// Project a warning list into canonical display order (`{code,message,severity}`).
    #[must_use]
    pub fn list(warnings: &[TypeshedWarning]) -> Vec<Self> {
        let mut owned = warnings.to_vec();
        canonicalize(&mut owned);
        owned.iter().map(Self::from).collect()
    }
}

/// The complete, serializable typeshed source status shared by CLI, LSP, and MCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypeshedStatus {
    /// Which source is serving step 3.
    pub active_source: SourceKind,
    /// The full commit SHA, when the source has one.
    #[serde(rename = "commit_identity")]
    pub commit: Option<Oid>,
    /// The verified root-tree SHA, when content was attested.
    #[serde(rename = "tree_identity")]
    pub tree: Option<Oid>,
    /// How the bytes were transported.
    pub transport: Transport,
    /// License standing.
    pub license_status: LicenseStatus,
    /// Immutable license reference (a pinned URL), or `None` for `not supplied`.
    pub license_reference: Option<String>,
    /// Provenance strength and trust boundary.
    pub provenance: Provenance,
    /// Whether the activated source was authenticated as a signed release.
    ///
    /// Typeshed currently publishes commits and archives rather than signed
    /// releases, so every built-in acquisition path reports `false`.
    pub signed_release: bool,
    /// Ordered, composable status warnings.
    pub warnings: Vec<StatusWarning>,
}

impl TypeshedStatus {
    /// Whether any warning is elevated (fallback, blocked, or unverified).
    #[must_use]
    pub fn has_high_severity(&self) -> bool {
        self.warnings
            .iter()
            .any(|warning| warning.severity == WarningSeverity::High)
    }
}

#[cfg(test)]
mod tests {
    use super::super::warning::UnpinnedKind;
    use super::*;

    const FULL_SHA: &str = "83c2518a9e6abbda0c44592c3483de459198f887";

    fn oid() -> Option<Oid> {
        Oid::from_hex(FULL_SHA).ok()
    }

    #[test]
    fn source_kind_labels_are_stable() {
        assert_eq!(SourceKind::ExactCommit.as_str(), "exact-commit");
        assert_eq!(SourceKind::Bundled.as_str(), "bundled");
    }

    #[test]
    fn identity_uri_component_never_leaks_a_raw_path() {
        let Some(commit) = oid() else {
            return;
        };
        assert_eq!(
            SourceIdentity::Bundled { commit }.uri_component(),
            format!("bundled-{FULL_SHA}")
        );
        let custom = SourceIdentity::Custom {
            digest: "abc123".to_owned(),
        };
        assert_eq!(custom.uri_component(), "custom-abc123");
        assert!(custom.commit().is_none());
    }

    #[test]
    fn only_explicit_pin_is_pinned() {
        let Some(commit) = oid() else {
            return;
        };
        assert!(SourceIdentity::Commit {
            commit,
            pinned: true
        }
        .is_pinned());
        assert!(!SourceIdentity::Commit {
            commit,
            pinned: false
        }
        .is_pinned());
        assert!(!SourceIdentity::Bundled { commit }.is_pinned());
    }

    #[test]
    fn status_projects_and_orders_warnings() {
        let warnings = StatusWarning::list(&[
            TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled),
            TypeshedWarning::Unverified,
        ]);
        // Canonical spec-table order: UNPINNED precedes UNVERIFIED.
        let codes: Vec<&str> = warnings.iter().map(|w| w.code.as_str()).collect();
        assert_eq!(codes, vec!["UNPINNED", "UNVERIFIED"]);
        let status = TypeshedStatus {
            active_source: SourceKind::Latest,
            commit: None,
            tree: None,
            transport: Transport::Codeload,
            license_status: LicenseStatus::Approved,
            license_reference: None,
            provenance: Provenance::Unverified,
            signed_release: false,
            warnings,
        };
        assert!(status.has_high_severity());
    }

    #[test]
    fn status_serializes_full_sha_with_requested_field_names() {
        let status = TypeshedStatus {
            active_source: SourceKind::ExactCommit,
            commit: oid(),
            tree: None,
            transport: Transport::Codeload,
            license_status: LicenseStatus::Approved,
            license_reference: Some(
                "https://github.com/python/typeshed/blob/83c2518/LICENSE".to_owned(),
            ),
            provenance: Provenance::GithubTlsAttested,
            signed_release: false,
            warnings: vec![],
        };
        let value = serde_json::to_value(&status);
        assert!(value.is_ok());
        if let Ok(json) = value {
            assert_eq!(
                json.get("active_source").and_then(|v| v.as_str()),
                Some("exact-commit")
            );
            // Full 40-char SHA on the wire, never a byte array or short form.
            assert_eq!(
                json.get("commit_identity").and_then(|v| v.as_str()),
                Some(FULL_SHA)
            );
            assert_eq!(
                json.get("provenance").and_then(|v| v.as_str()),
                Some("github-tls-attested")
            );
            assert_eq!(
                json.get("signed_release")
                    .and_then(serde_json::Value::as_bool),
                Some(false)
            );
        }
    }
}
