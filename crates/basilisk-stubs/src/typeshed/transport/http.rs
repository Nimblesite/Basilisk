//! Implements [STUBRES-TYPESHED-ACQUIRE] transport. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE
//!
//! Authenticated-HTTPS GitHub metadata and archive adapter.
//!
//! "Authenticated" is load-bearing: requests to official GitHub hosts carry a
//! `GITHUB_TOKEN`/`GH_TOKEN` bearer credential when the environment supplies
//! one. A user-configured mirror never does — see [`CREDENTIAL_HOSTS`].

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::{CommitMetadata, Transport, TransportError, TreeEntry};
use crate::typeshed::gittree::{FileMode, Oid};
use crate::typeshed::source::Transport as SourceTransport;

const API_ROOT: &str = "https://api.github.com/repos/python/typeshed";
const CODELOAD_ROOT: &str = "https://codeload.github.com/python/typeshed/zip";
const METADATA_LIMIT: usize = 32 * 1024 * 1024;
const ARCHIVE_LIMIT: usize = 128 * 1024 * 1024;

/// Hosts the GitHub credential may be presented to. Anything else — including
/// every user-configured mirror — is contacted anonymously.
const CREDENTIAL_HOSTS: [&str; 2] = ["api.github.com", "codeload.github.com"];

/// Environment variables carrying a GitHub token, in precedence order. These are
/// the names the GitHub CLI and Actions already export, so CI and developer
/// shells are authenticated without any Basilisk-specific setup.
const TOKEN_VARIABLES: [&str; 2] = ["GITHUB_TOKEN", "GH_TOKEN"];

/// Production transport for official GitHub metadata and either codeload or a
/// configured authenticated-HTTPS `{sha}` archive mirror.
pub struct HttpsTransport {
    agent: ureq::Agent,
    mirror_template: Option<String>,
    /// A GitHub token, when one was present in the environment. Anonymous
    /// requests share a 60/hour/IP budget that CI and busy networks exhaust,
    /// which surfaces as an opaque metadata failure; a token raises that to
    /// 5000/hour. Never logged, never rendered in `Debug`, and never sent
    /// anywhere but [`CREDENTIAL_HOSTS`].
    token: Option<String>,
}

impl std::fmt::Debug for HttpsTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpsTransport")
            .field("mirror_configured", &self.mirror_template.is_some())
            .field("credential", &credential_state(self.token.as_deref()))
            .finish_non_exhaustive()
    }
}

impl HttpsTransport {
    /// Build the production HTTPS adapter. Mirror URLs and credentials are
    /// retained privately and never included in public errors or debug output.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::InvalidMirror`] unless a mirror is an HTTPS
    /// URL containing exactly one `{sha}` placeholder.
    pub fn new(mirror_template: Option<String>) -> Result<Self, TransportError> {
        Self::with_token(mirror_template, token_from_environment())
    }

    fn with_token(
        mirror_template: Option<String>,
        token: Option<String>,
    ) -> Result<Self, TransportError> {
        if let Some(template) = mirror_template.as_deref() {
            validate_mirror(template)?;
        }
        // A blank credential is normalized away here rather than at the
        // environment boundary, so no future caller can reintroduce
        // `Authorization: Bearer ` — which 401s a request that would have
        // succeeded anonymously.
        let token = token.filter(|value| !value.trim().is_empty());
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(5)
            .max_redirects_will_error(true)
            .timeout_global(Some(Duration::from_mins(1)))
            .user_agent("basilisk-typeshed-runtime")
            .build();
        tracing::debug!(
            credential = credential_state(token.as_deref()),
            mirror_configured = mirror_template.is_some(),
            "typeshed https transport configured"
        );
        Ok(Self {
            agent: config.into(),
            mirror_template,
            token,
        })
    }

    /// Attach the GitHub credential when — and only when — `url` addresses an
    /// official GitHub host. A mirror is operated by a third party, so sending
    /// the user's token there would disclose it outside its trust boundary.
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

    fn metadata(&self, reference: &str) -> Result<CommitMetadata, TransportError> {
        let url = format!("{API_ROOT}/commits/{reference}");
        let response: CommitResponse = self.get_json(&url)?;
        let commit = Oid::from_hex(&response.sha).map_err(|_error| TransportError::Metadata)?;
        let tree =
            Oid::from_hex(&response.commit.tree.sha).map_err(|_error| TransportError::Metadata)?;
        Ok(CommitMetadata { commit, tree })
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

    fn archive_url(&self, commit: Oid) -> String {
        let sha = commit.to_hex();
        self.mirror_template.as_ref().map_or_else(
            || format!("{CODELOAD_ROOT}/{sha}"),
            |template| template.replace("{sha}", &sha),
        )
    }
}

impl Transport for HttpsTransport {
    fn resolve_latest(&self) -> Result<CommitMetadata, TransportError> {
        self.metadata("main")
    }

    fn resolve_commit(&self, commit: Oid) -> Result<CommitMetadata, TransportError> {
        let expected = commit.to_hex();
        confirm_requested_commit(self.metadata(&expected)?, commit)
    }

    fn fetch_tree(&self, root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError> {
        let url = format!("{API_ROOT}/git/trees/{}?recursive=1", root_tree.to_hex());
        tree_entries(self.get_json(&url)?, root_tree)
    }

    fn fetch_archive(&self, commit: Oid) -> Result<Vec<u8>, TransportError> {
        let url = self.archive_url(commit);
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

    fn archive_transport(&self) -> SourceTransport {
        if self.mirror_template.is_some() {
            SourceTransport::Mirror
        } else {
            SourceTransport::Codeload
        }
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

/// Confirm the API returned the commit that was asked for.
///
/// GitHub resolves a `commits/{ref}` request against branches and tags as well
/// as SHAs, so a pin must be re-checked against the response rather than
/// assumed. Substituting a different commit under a pin would silently break
/// the reproducibility the pin exists to provide.
fn confirm_requested_commit(
    metadata: CommitMetadata,
    requested: Oid,
) -> Result<CommitMetadata, TransportError> {
    if metadata.commit == requested {
        Ok(metadata)
    } else {
        Err(TransportError::Metadata)
    }
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
    // Selection is split from the environment read so precedence and the
    // blank-skip rule are testable without mutating process-wide state, which
    // would race every other test in the binary.
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

fn validate_mirror(template: &str) -> Result<(), TransportError> {
    if template.matches("{sha}").count() != 1 {
        return Err(TransportError::InvalidMirror);
    }
    let candidate = template.replace("{sha}", "0000000000000000000000000000000000000000");
    let uri = candidate
        .parse::<ureq::http::Uri>()
        .map_err(|_error| TransportError::InvalidMirror)?;
    if uri.scheme_str() != Some("https") || uri.host().is_none() {
        return Err(TransportError::InvalidMirror);
    }
    Ok(())
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

    #[test]
    fn mirror_requires_one_https_sha_placeholder() {
        assert!(HttpsTransport::new(Some("https://mirror.test/{sha}.zip".to_owned())).is_ok());
        assert_eq!(
            HttpsTransport::new(Some("http://mirror.test/{sha}.zip".to_owned())).err(),
            Some(TransportError::InvalidMirror)
        );
        assert_eq!(
            HttpsTransport::new(Some("https://mirror.test/archive.zip".to_owned())).err(),
            Some(TransportError::InvalidMirror)
        );
        assert_eq!(
            HttpsTransport::new(Some(
                "https://mirror.test/{sha}/duplicate-{sha}.zip".to_owned()
            ))
            .err(),
            Some(TransportError::InvalidMirror)
        );
        assert_eq!(
            HttpsTransport::new(Some("https:///{sha}.zip".to_owned())).err(),
            Some(TransportError::InvalidMirror)
        );
    }

    #[test]
    fn archive_identity_and_transport_follow_the_configured_source() -> Result<(), TransportError> {
        let commit = Oid::from_hex(SHA).map_err(|_error| TransportError::Metadata)?;
        let official = HttpsTransport::new(None)?;
        assert_eq!(
            official.archive_url(commit),
            format!("{CODELOAD_ROOT}/{SHA}")
        );
        assert_eq!(official.archive_transport(), SourceTransport::Codeload);

        let mirror_template = "https://mirror.test/private/{sha}.zip";
        let mirror = HttpsTransport::new(Some(mirror_template.to_owned()))?;
        assert_eq!(
            mirror.archive_url(commit),
            "https://mirror.test/private/0123456789abcdef0123456789abcdef01234567.zip"
        );
        assert_eq!(mirror.archive_transport(), SourceTransport::Mirror);

        let debug = format!("{mirror:?}");
        assert!(debug.contains("mirror_configured: true"));
        assert!(
            !debug.contains("private"),
            "mirror URLs must remain redacted"
        );
        Ok(())
    }

    /// The credential boundary. A mirror is third-party infrastructure, so a
    /// token that leaked into a mirror request would be disclosed outside the
    /// trust boundary it was issued for. Substring matching would be the easy
    /// way to get this wrong, so lookalike authorities are asserted explicitly.
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
            "https://mirror.test/private/0123456789abcdef.zip",
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
    fn authorization_for(transport: &HttpsTransport, url: &str) -> Option<String> {
        let request = transport.authorize(transport.agent.get(url), url);
        request
            .headers_ref()
            .and_then(|headers| headers.get("Authorization"))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }

    /// The security boundary, asserted end-to-end on the outgoing header.
    /// A leaked token would be disclosed to whoever operates the mirror, so
    /// "no Authorization header on a mirror request" is the property that
    /// actually matters — not merely that a helper returns false.
    #[test]
    fn the_bearer_header_reaches_github_and_never_a_mirror() -> Result<(), TransportError> {
        let secret = "ghp_boundary_probe";
        let mirror = "https://mirror.test/private/{sha}.zip";
        let transport =
            HttpsTransport::with_token(Some(mirror.to_owned()), Some(secret.to_owned()))?;

        assert_eq!(
            authorization_for(&transport, &format!("{API_ROOT}/commits/main")),
            Some(format!("Bearer {secret}")),
            "official metadata requests must be authenticated"
        );
        assert_eq!(
            authorization_for(&transport, &format!("{CODELOAD_ROOT}/{SHA}")),
            Some(format!("Bearer {secret}")),
            "official archive requests must be authenticated"
        );
        assert_eq!(
            authorization_for(&transport, &mirror.replace("{sha}", SHA)),
            None,
            "a third-party mirror must never receive the GitHub credential"
        );

        let anonymous = HttpsTransport::with_token(None, None)?;
        assert_eq!(
            authorization_for(&anonymous, &format!("{API_ROOT}/commits/main")),
            None,
            "with no credential the request stays anonymous rather than sending an empty bearer"
        );
        Ok(())
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
    /// CI runners export `GITHUB_TOKEN=` for credential-less jobs, so this is
    /// the common case, not an edge case.
    #[test]
    fn a_blank_credential_is_treated_as_absent() -> Result<(), TransportError> {
        let blank = HttpsTransport::with_token(None, Some("   ".to_owned()))?;
        assert_eq!(credential_state(blank.token.as_deref()), "absent");

        let real = HttpsTransport::with_token(None, Some("ghp_example".to_owned()))?;
        assert_eq!(credential_state(real.token.as_deref()), "present");
        Ok(())
    }

    /// `Debug` is reachable from tracing and error reporting, so it must expose
    /// only whether a credential exists — never its value.
    #[test]
    fn debug_output_reveals_credential_presence_but_never_its_value() -> Result<(), TransportError>
    {
        let secret = "ghp_thismustnotappearanywhere";
        let transport = HttpsTransport::with_token(None, Some(secret.to_owned()))?;
        let debug = format!("{transport:?}");
        assert!(debug.contains("credential: \"present\""), "got: {debug}");
        assert!(!debug.contains(secret), "the token must never be rendered");

        let anonymous = HttpsTransport::with_token(None, None)?;
        assert!(format!("{anonymous:?}").contains("credential: \"absent\""));
        Ok(())
    }

    /// A pin must be re-checked against what the API actually returned:
    /// `commits/{ref}` resolves branches and tags too, so accepting the
    /// response unverified would let a pin silently drift to another commit.
    #[test]
    fn a_resolved_commit_must_be_the_one_that_was_requested() -> Result<(), TransportError> {
        let requested = Oid::from_hex(SHA).map_err(|_error| TransportError::Metadata)?;
        let other = Oid::from_hex(OTHER_SHA).map_err(|_error| TransportError::Metadata)?;
        let tree = Oid::from_hex(TREE_SHA).map_err(|_error| TransportError::Metadata)?;

        let matching = CommitMetadata {
            commit: requested,
            tree,
        };
        assert_eq!(
            confirm_requested_commit(matching, requested),
            Ok(CommitMetadata {
                commit: requested,
                tree
            })
        );
        assert_eq!(
            confirm_requested_commit(
                CommitMetadata {
                    commit: other,
                    tree
                },
                requested
            ),
            Err(TransportError::Metadata),
            "a substituted commit must fail closed"
        );
        Ok(())
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
        assert_eq!(
            tree_entries(
                TreeResponse {
                    sha: Some(TREE_SHA.to_owned()),
                    truncated: false,
                    tree: vec![api_entry("blob", "160000", SHA)],
                },
                root
            ),
            Err(TransportError::Metadata),
            "one noncanonical leaf rejects the whole listing"
        );
        Ok(())
    }

    /// Rate limiting surfaces as 403 and was previously indistinguishable from
    /// a parse failure, which is exactly how it got misdiagnosed. The status
    /// must be recoverable for the log line; transport-level failures with no
    /// response report 0 rather than inventing one.
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
