//! Authenticated-HTTPS GitHub metadata and archive adapter.

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

/// Production transport for official GitHub metadata and either codeload or a
/// configured authenticated-HTTPS `{sha}` archive mirror.
pub struct HttpsTransport {
    agent: ureq::Agent,
    mirror_template: Option<String>,
}

impl std::fmt::Debug for HttpsTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpsTransport")
            .field("mirror_configured", &self.mirror_template.is_some())
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
        if let Some(template) = mirror_template.as_deref() {
            validate_mirror(template)?;
        }
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(5)
            .max_redirects_will_error(true)
            .timeout_global(Some(Duration::from_mins(1)))
            .user_agent("basilisk-typeshed-runtime")
            .build();
        Ok(Self {
            agent: config.into(),
            mirror_template,
        })
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
        let mut response = self
            .agent
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .call()
            .map_err(|_error| TransportError::Metadata)?;
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
        let metadata = self.metadata(&expected)?;
        if metadata.commit != commit {
            return Err(TransportError::Metadata);
        }
        Ok(metadata)
    }

    fn fetch_tree(&self, root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError> {
        let url = format!("{API_ROOT}/git/trees/{}?recursive=1", root_tree.to_hex());
        let response: TreeResponse = self.get_json(&url)?;
        let expected_root = root_tree.to_hex();
        if response.truncated || response.sha.as_deref() != Some(expected_root.as_str()) {
            return Err(TransportError::Metadata);
        }
        response
            .tree
            .into_iter()
            .filter_map(convert_tree_entry)
            .collect()
    }

    fn fetch_archive(&self, commit: Oid) -> Result<Vec<u8>, TransportError> {
        let url = self.archive_url(commit);
        let mut response = self
            .agent
            .get(&url)
            .call()
            .map_err(|_error| TransportError::Download)?;
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

fn convert_tree_entry(entry: ApiTreeEntry) -> Option<Result<TreeEntry, TransportError>> {
    if entry.kind == "tree" {
        return None;
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
    }
}
