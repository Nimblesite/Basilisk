//! Implements [STUBRES-TYPESHED-DOWNLOAD] transport. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-DOWNLOAD.
//!
//! Authenticated-HTTPS GitHub metadata and archive adapter — the only typeshed
//! network code in the workspace ([TYPESHEDRT-SEGREGATION]). There is no
//! mirror setting: downloads come only from `api.github.com` and
//! `codeload.github.com`, and the credential goes nowhere else.

use std::time::Duration;

use basilisk_stubs::typeshed::gittree::{FileMode, Oid};
use serde::de::DeserializeOwned;
use serde::Deserialize;

const API_ROOT: &str = "https://api.github.com/repos/python/typeshed";
const CODELOAD_ROOT: &str = "https://codeload.github.com/python/typeshed/zip";
const METADATA_LIMIT: usize = 32 * 1024 * 1024;
const ARCHIVE_LIMIT: usize = 128 * 1024 * 1024;

/// Hosts the GitHub credential may be presented to — nothing else exists in
/// this crate to contact.
const CREDENTIAL_HOSTS: [&str; 2] = ["api.github.com", "codeload.github.com"];

/// Environment variables carrying a GitHub token, in precedence order. These are
/// the names the GitHub CLI and Actions already export, so CI and developer
/// shells are authenticated without any Basilisk-specific setup.
const TOKEN_VARIABLES: [&str; 2] = ["GITHUB_TOKEN", "GH_TOKEN"];

/// A transport failure. URLs and credentials are redacted before this crosses
/// the download boundary; raw detail belongs only in redacted tracing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// Commit or tree metadata could not be resolved.
    #[error("metadata resolution failed")]
    Metadata,
    /// The archive could not be downloaded.
    #[error("archive download failed")]
    Download,
}

/// Commit metadata with the raw material for offline re-verification: the
/// signed payload and signature reconstruct the raw commit object
/// ([STUBRES-TYPESHED-PIN]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    /// The resolved full commit SHA.
    pub commit: Oid,
    /// The commit's root-tree SHA as reported by the API (cross-checked
    /// against the reconstructed commit object).
    pub tree: Oid,
    /// The commit content GitHub attests (raw object minus any signature header).
    pub payload: String,
    /// The PGP/SSH signature, when the commit is signed.
    pub signature: Option<String>,
}

/// One trusted recursive-tree entry: a repo-relative path, its blob object ID,
/// and its Git mode. These trusted modes and OIDs drive content attestation
/// because codeload archives do not preserve them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// Repo-relative path.
    pub path: String,
    /// The blob (or submodule commit) object ID.
    pub oid: Oid,
    /// The Git file mode.
    pub mode: FileMode,
}

/// Production GitHub client. Injectable via [`GithubApi`] so the download
/// pipeline is testable offline.
pub struct GithubClient {
    agent: ureq::Agent,
    token: Option<String>,
}

impl std::fmt::Debug for GithubClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GithubClient")
            .field("credential", &credential_state(self.token.as_deref()))
            .finish_non_exhaustive()
    }
}

/// The GitHub surface the download pipeline consumes.
pub trait GithubApi: Send + Sync {
    /// Resolve a reference (`main` or a full SHA) to commit metadata carrying
    /// the raw commit-object material.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when official metadata cannot be resolved.
    fn resolve(&self, reference: &str) -> Result<CommitInfo, TransportError>;

    /// Fetch the trusted recursive tree for a commit (path → blob OID + mode).
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the tree metadata cannot be fetched.
    fn fetch_tree(&self, root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError>;

    /// Fetch a commit's archive (zipball) bytes from codeload.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the archive cannot be downloaded.
    fn fetch_archive(&self, commit: Oid) -> Result<Vec<u8>, TransportError>;
}

impl GithubClient {
    /// Build the production HTTPS adapter, reading the credential from the
    /// environment. The token is retained privately and never appears in
    /// errors, logs, or `Debug` output.
    #[must_use]
    pub fn new() -> Self {
        Self::with_token(token_from_environment())
    }

    fn with_token(token: Option<String>) -> Self {
        // A blank credential is normalized away here rather than at the
        // environment boundary, so no future caller can reintroduce
        // `Authorization: Bearer ` — which 401s a request that would have
        // succeeded anonymously.
        let token = token.filter(|value| !value.trim().is_empty());
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(5)
            .max_redirects_will_error(true)
            .timeout_global(Some(Duration::from_mins(2)))
            .user_agent("basilisk-typeshed-fetch")
            .build();
        tracing::debug!(
            credential = credential_state(token.as_deref()),
            "typeshed download transport configured"
        );
        Self {
            agent: config.into(),
            token,
        }
    }

    /// Attach the GitHub credential when — and only when — `url` addresses an
    /// official GitHub host.
    fn authorize(
        &self,
        request: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
        url: &str,
    ) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
        match self
            .token
            .as_deref()
            .filter(|_token| is_credential_host(url))
        {
            Some(token) => request.header("Authorization", &format!("Bearer {token}")),
            None => request,
        }
    }

    fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, TransportError> {
        let request = self
            .agent
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        let mut response = self.authorize(request, url).call().map_err(|error| {
            // The public error is deliberately opaque, but a silent one made a
            // plain 403-from-rate-limiting indistinguishable from a parse
            // failure. Record the status so the cause is never guessed again.
            tracing::warn!(
                status = status_of(&error),
                credential = credential_state(self.token.as_deref()),
                "typeshed metadata request failed"
            );
            TransportError::Metadata
        })?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(u64::try_from(METADATA_LIMIT.saturating_add(1)).unwrap_or(u64::MAX))
            .read_to_vec()
            .map_err(|_error| TransportError::Metadata)?;
        if bytes.len() > METADATA_LIMIT {
            return Err(TransportError::Metadata);
        }
        serde_json::from_slice(&bytes).map_err(|_error| TransportError::Metadata)
    }
}

impl Default for GithubClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GithubApi for GithubClient {
    fn resolve(&self, reference: &str) -> Result<CommitInfo, TransportError> {
        let url = format!("{API_ROOT}/commits/{reference}");
        let response: CommitResponse = self.get_json(&url)?;
        let commit = Oid::from_hex(&response.sha).map_err(|_error| TransportError::Metadata)?;
        let tree =
            Oid::from_hex(&response.commit.tree.sha).map_err(|_error| TransportError::Metadata)?;
        let payload = response
            .commit
            .verification
            .payload
            .ok_or(TransportError::Metadata)?;
        Ok(CommitInfo {
            commit,
            tree,
            payload,
            signature: response.commit.verification.signature,
        })
    }

    fn fetch_tree(&self, root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError> {
        let url = format!("{API_ROOT}/git/trees/{}?recursive=1", root_tree.to_hex());
        tree_entries(self.get_json(&url)?, root_tree)
    }

    fn fetch_archive(&self, commit: Oid) -> Result<Vec<u8>, TransportError> {
        let url = format!("{CODELOAD_ROOT}/{}", commit.to_hex());
        let request = self.agent.get(&url);
        let mut response = self.authorize(request, &url).call().map_err(|error| {
            tracing::warn!(
                status = status_of(&error),
                credential = credential_state(self.token.as_deref()),
                "typeshed archive request failed"
            );
            TransportError::Download
        })?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(u64::try_from(ARCHIVE_LIMIT.saturating_add(1)).unwrap_or(u64::MAX))
            .read_to_vec()
            .map_err(|_error| TransportError::Download)?;
        if bytes.len() > ARCHIVE_LIMIT {
            return Err(TransportError::Download);
        }
        Ok(bytes)
    }
}

#[derive(Deserialize)]
struct CommitResponse {
    sha: String,
    commit: CommitDetails,
}

#[derive(Deserialize)]
struct CommitDetails {
    tree: TreeIdentity,
    #[serde(default)]
    verification: Verification,
}

#[derive(Deserialize, Default)]
struct Verification {
    payload: Option<String>,
    signature: Option<String>,
}

#[derive(Deserialize)]
struct TreeIdentity {
    sha: String,
}

#[derive(Deserialize)]
struct TreeResponse {
    sha: Option<String>,
    truncated: bool,
    tree: Vec<ApiTreeEntry>,
}

#[derive(Deserialize)]
struct ApiTreeEntry {
    path: String,
    mode: String,
    #[serde(rename = "type")]
    kind: String,
    sha: String,
}

/// Convert a recursive-tree response into trusted leaves.
///
/// Fails closed on a truncated listing or a root that is not the one requested:
/// a partial tree would under-report files, and content verification would then
/// bind the archive to an incomplete tree rather than rejecting it.
fn tree_entries(
    response: TreeResponse,
    expected_root: Oid,
) -> Result<Vec<TreeEntry>, TransportError> {
    let expected = expected_root.to_hex();
    if response.truncated || response.sha.as_deref() != Some(expected.as_str()) {
        return Err(TransportError::Metadata);
    }
    response
        .tree
        .into_iter()
        .filter_map(convert_tree_entry)
        .collect()
}

fn convert_tree_entry(entry: ApiTreeEntry) -> Option<Result<TreeEntry, TransportError>> {
    if entry.kind == "tree" {
        return if entry.mode == "040000" {
            None
        } else {
            Some(Err(TransportError::Metadata))
        };
    }
    let mode = match (entry.kind.as_str(), entry.mode.as_str()) {
        ("blob", "100644") => FileMode::Regular,
        ("blob", "100755") => FileMode::Executable,
        ("blob", "120000") => FileMode::Symlink,
        ("commit", "160000") => FileMode::Submodule,
        _ => return Some(Err(TransportError::Metadata)),
    };
    let oid = match Oid::from_hex(&entry.sha) {
        Ok(oid) => oid,
        Err(_error) => return Some(Err(TransportError::Metadata)),
    };
    Some(Ok(TreeEntry {
        path: entry.path,
        oid,
        mode,
    }))
}

/// Read the first non-blank token from the environment.
///
/// A variable set to an empty or whitespace-only string is treated as absent:
/// CI runners routinely export `GITHUB_TOKEN=` for jobs without a credential,
/// and sending `Authorization: Bearer ` would turn an anonymous-but-working
/// request into a hard 401.
fn token_from_environment() -> Option<String> {
    first_non_blank(TOKEN_VARIABLES.iter().map(|name| std::env::var(name).ok()))
}

/// The first candidate that is present and not blank, preserving input order.
fn first_non_blank(candidates: impl Iterator<Item = Option<String>>) -> Option<String> {
    candidates.flatten().find(|value| !value.trim().is_empty())
}

/// Whether `url` addresses a host the GitHub credential may be sent to.
///
/// Matching is on the parsed authority, never a substring: `https://evil.test/
/// ?x=api.github.com` and `https://api.github.com.evil.test/` must both fail.
fn is_credential_host(url: &str) -> bool {
    url.parse::<ureq::http::Uri>()
        .ok()
        .and_then(|uri| uri.host().map(str::to_ascii_lowercase))
        .is_some_and(|host| CREDENTIAL_HOSTS.contains(&host.as_str()))
}

/// Presence of a credential, in the only form that may ever reach a log.
fn credential_state(token: Option<&str>) -> &'static str {
    if token.is_some() {
        "present"
    } else {
        "absent"
    }
}

/// The HTTP status of a failed request, or `0` when the request never got one
/// (DNS, TLS, or timeout). Used for diagnostics only; it never widens the
/// public error, which stays redacted.
fn status_of(error: &ureq::Error) -> u16 {
    match error {
        ureq::Error::StatusCode(code) => *code,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
    const OTHER_SHA: &str = "89abcdef0123456789abcdef0123456789abcdef";
    const TREE_SHA: &str = "fedcba9876543210fedcba9876543210fedcba98";

    fn api_entry(kind: &str, mode: &str, sha: &str) -> ApiTreeEntry {
        ApiTreeEntry {
            path: "stdlib/example.pyi".to_owned(),
            mode: mode.to_owned(),
            kind: kind.to_owned(),
            sha: sha.to_owned(),
        }
    }

    /// The credential boundary. Substring matching would be the easy way to
    /// get this wrong, so lookalike authorities are asserted explicitly.
    #[test]
    fn the_github_credential_is_confined_to_official_github_hosts() {
        for official in [
            "https://api.github.com/repos/python/typeshed/commits/main",
            "https://codeload.github.com/python/typeshed/zip/0123456789abcdef",
            "https://API.GitHub.com/repos/python/typeshed/commits/main",
        ] {
            assert!(
                is_credential_host(official),
                "{official} is an official GitHub host"
            );
        }

        for foreign in [
            "https://api.github.com.evil.test/repos/python/typeshed",
            "https://evil.test/proxy?upstream=api.github.com",
            "https://notapi.github.com/repos/python/typeshed",
            "https://evil.test/api.github.com/repos",
            "not a url at all",
        ] {
            assert!(
                !is_credential_host(foreign),
                "{foreign} must never receive the GitHub credential"
            );
        }
    }

    /// Read the `Authorization` header value that `authorize` actually put on
    /// a request for `url`, so the boundary is asserted on the real outgoing
    /// header rather than on the host predicate alone.
    fn authorization_for(client: &GithubClient, url: &str) -> Option<String> {
        let request = client.authorize(client.agent.get(url), url);
        request
            .headers_ref()
            .and_then(|headers| headers.get("Authorization"))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }

    /// The security boundary, asserted end-to-end on the outgoing header.
    #[test]
    fn the_bearer_header_reaches_github_and_nothing_else() {
        let secret = "ghp_boundary_probe";
        let client = GithubClient::with_token(Some(secret.to_owned()));

        assert_eq!(
            authorization_for(&client, &format!("{API_ROOT}/commits/main")),
            Some(format!("Bearer {secret}")),
            "official metadata requests must be authenticated"
        );
        assert_eq!(
            authorization_for(&client, &format!("{CODELOAD_ROOT}/{SHA}")),
            Some(format!("Bearer {secret}")),
            "official archive requests must be authenticated"
        );
        assert_eq!(
            authorization_for(&client, "https://mirror.test/private/archive.zip"),
            None,
            "a third-party host must never receive the GitHub credential"
        );

        let anonymous = GithubClient::with_token(None);
        assert_eq!(
            authorization_for(&anonymous, &format!("{API_ROOT}/commits/main")),
            None,
            "with no credential the request stays anonymous rather than sending an empty bearer"
        );
    }

    /// Precedence and the blank-skip rule, asserted without mutating the
    /// process environment (which would race every other test in the binary).
    #[test]
    fn credential_selection_prefers_the_first_present_non_blank_variable() {
        let cases: [(&[Option<&str>], Option<&str>); 5] = [
            (&[Some("first"), Some("second")], Some("first")),
            (&[None, Some("second")], Some("second")),
            (&[Some(""), Some("second")], Some("second")),
            (&[Some("  \t "), Some("second")], Some("second")),
            (&[None, None], None),
        ];
        for (inputs, expected) in cases {
            let selected = first_non_blank(inputs.iter().map(|value| value.map(str::to_owned)));
            assert_eq!(selected.as_deref(), expected, "inputs: {inputs:?}");
        }
    }

    /// A blank token is worse than none: `Authorization: Bearer ` is rejected
    /// with 401, turning a working anonymous request into a hard failure.
    #[test]
    fn a_blank_credential_is_treated_as_absent() {
        let blank = GithubClient::with_token(Some("   ".to_owned()));
        assert_eq!(credential_state(blank.token.as_deref()), "absent");

        let real = GithubClient::with_token(Some("ghp_example".to_owned()));
        assert_eq!(credential_state(real.token.as_deref()), "present");
    }

    /// `Debug` is reachable from tracing and error reporting, so it must expose
    /// only whether a credential exists — never its value.
    #[test]
    fn debug_output_reveals_credential_presence_but_never_its_value() {
        let secret = "ghp_thismustnotappearanywhere";
        let client = GithubClient::with_token(Some(secret.to_owned()));
        let debug = format!("{client:?}");
        assert!(debug.contains("credential: \"present\""), "got: {debug}");
        assert!(!debug.contains(secret), "the token must never be rendered");

        let anonymous = GithubClient::with_token(None);
        assert!(format!("{anonymous:?}").contains("credential: \"absent\""));
    }

    /// A truncated listing or a mismatched root would under-report files, and
    /// content verification would then bind the archive to an incomplete tree
    /// instead of rejecting it outright.
    #[test]
    fn tree_listings_fail_closed_when_truncated_or_rooted_elsewhere() -> Result<(), TransportError>
    {
        let root = Oid::from_hex(TREE_SHA).map_err(|_error| TransportError::Metadata)?;
        let response = |sha: Option<&str>, truncated: bool| TreeResponse {
            sha: sha.map(str::to_owned),
            truncated,
            tree: vec![api_entry("blob", "100644", SHA)],
        };

        let accepted = tree_entries(response(Some(TREE_SHA), false), root)?;
        let only = accepted.split_first().filter(|(_, rest)| rest.is_empty());
        let Some((leaf, _)) = only else {
            return Err(TransportError::Metadata);
        };
        assert_eq!(leaf.path, "stdlib/example.pyi");
        assert_eq!(leaf.mode, FileMode::Regular);

        assert_eq!(
            tree_entries(response(Some(TREE_SHA), true), root),
            Err(TransportError::Metadata),
            "a truncated listing must never be treated as a complete tree"
        );
        assert_eq!(
            tree_entries(response(Some(OTHER_SHA), false), root),
            Err(TransportError::Metadata),
            "a listing rooted at another tree must fail closed"
        );
        assert_eq!(
            tree_entries(response(None, false), root),
            Err(TransportError::Metadata),
            "a listing with no root SHA must fail closed"
        );
        Ok(())
    }

    /// Rate limiting surfaces as 403 and was previously indistinguishable from
    /// a parse failure, which is exactly how it got misdiagnosed.
    #[test]
    fn failed_requests_expose_a_status_for_diagnostics() {
        assert_eq!(status_of(&ureq::Error::StatusCode(403)), 403);
        assert_eq!(status_of(&ureq::Error::StatusCode(401)), 401);
        assert_eq!(status_of(&ureq::Error::HostNotFound), 0);
        assert_eq!(status_of(&ureq::Error::Timeout(ureq::Timeout::Global)), 0);
    }

    #[test]
    fn tree_metadata_accepts_only_canonical_git_leaf_modes() -> Result<(), TransportError> {
        let expected_oid = Oid::from_hex(SHA).map_err(|_error| TransportError::Metadata)?;
        let cases = [
            ("blob", "100644", FileMode::Regular),
            ("blob", "100755", FileMode::Executable),
            ("blob", "120000", FileMode::Symlink),
            ("commit", "160000", FileMode::Submodule),
        ];
        for (kind, mode, expected) in cases {
            let converted = convert_tree_entry(api_entry(kind, mode, SHA))
                .ok_or(TransportError::Metadata)??;
            assert_eq!(converted.path, "stdlib/example.pyi");
            assert_eq!(converted.oid, expected_oid);
            assert_eq!(converted.mode, expected);
        }

        assert!(convert_tree_entry(api_entry("tree", "040000", SHA)).is_none());
        assert_eq!(
            convert_tree_entry(api_entry("tree", "100644", SHA)),
            Some(Err(TransportError::Metadata)),
            "noncanonical tree modes must fail closed"
        );
        assert_eq!(
            convert_tree_entry(api_entry("blob", "160000", SHA)),
            Some(Err(TransportError::Metadata))
        );
        assert_eq!(
            convert_tree_entry(api_entry("blob", "100644", "not-a-sha")),
            Some(Err(TransportError::Metadata))
        );
        Ok(())
    }
}
