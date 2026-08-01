//! Implements [STUBRES-TYPESHED-PYPI] acquisition transport. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-PYPI.
//!
//! Anonymous-HTTPS `PyPI` adapter — the only typeshed network code that talks
//! to a host other than GitHub ([TYPESHEDRT-SEGREGATION]). There is deliberately
//! **no credential**: the GitHub token never leaves this crate's GitHub
//! adapter, and `PyPI`'s JSON + file hosts (`pypi.org`,
//! `files.pythonhosted.org`) are served anonymously. A wheel is selected by the
//! pinned SHA-256 from the project's release index, then streamed and re-hashed
//! by the caller — the index's reported digest is never trusted on its own.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::TransportError;

const PYPI_JSON_ROOT: &str = "https://pypi.org/pypi";
const METADATA_LIMIT: usize = 64 * 1024 * 1024;
const WHEEL_LIMIT: usize = 256 * 1024 * 1024;

/// The `PyPI` surface the package-download pipeline consumes. A wheel is fetched
/// by the pinned SHA-256: the index is resolved, the file whose digest matches
/// is selected, and its bytes are returned for the caller to re-hash and store.
///
/// Injected behind the download pipeline so it is testable offline (the
/// `test-support` `FakePypiApi`), exactly like [`crate::GithubApi`].
pub trait PypiApi: Send + Sync {
    /// Fetch the wheel bytes for `name` whose SHA-256 is `sha256`.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Metadata`] when the project index cannot be
    /// resolved or no wheel matches the pin, and [`TransportError::Download`]
    /// when the wheel bytes cannot be fetched.
    fn fetch_wheel(&self, name: &str, sha256: &str) -> Result<Vec<u8>, TransportError>;
}

/// Production `PyPI` client. Anonymous and HTTPS-only; no credential is held, so
/// the GitHub token can never reach `PyPI` even by accident.
pub struct PypiClient {
    agent: ureq::Agent,
}

impl std::fmt::Debug for PypiClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("PypiClient").finish_non_exhaustive()
    }
}

impl Default for PypiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PypiClient {
    /// Build the anonymous HTTPS adapter. No token is read or held.
    #[must_use]
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(5)
            .max_redirects_will_error(true)
            .timeout_global(Some(Duration::from_mins(5)))
            .user_agent("basilisk-typeshed-fetch")
            .build();
        Self {
            agent: config.into(),
        }
    }

    fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, TransportError> {
        let mut response = self
            .agent
            .get(url)
            .header("Accept", "application/json")
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

    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, TransportError> {
        let mut response = self.agent.get(url).call().map_err(|_error| {
            tracing::warn!("typeshed wheel download request failed");
            TransportError::Download
        })?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(u64::try_from(WHEEL_LIMIT.saturating_add(1)).unwrap_or(u64::MAX))
            .read_to_vec()
            .map_err(|_error| TransportError::Download)?;
        if bytes.len() > WHEEL_LIMIT {
            return Err(TransportError::Download);
        }
        Ok(bytes)
    }
}

impl PypiApi for PypiClient {
    fn fetch_wheel(&self, name: &str, sha256: &str) -> Result<Vec<u8>, TransportError> {
        // `name` becomes a path segment of the index URL, so it is constrained
        // to the PEP 508 alphabet *before* a request is built — a name that
        // could redirect the lookup to another resource never reaches the
        // network ([STUBRES-TYPESHED-PYPI]). The config layer rejects the same
        // shape at parse time; this is the boundary that makes it structural
        // rather than a caller convention, since `PypiApi` is public.
        if !basilisk_config::is_valid_distribution_name(name) {
            tracing::warn!("typeshed package name is not a PEP 508 distribution name; not fetched");
            return Err(TransportError::Metadata);
        }
        let url = format!("{PYPI_JSON_ROOT}/{name}/json");
        let project: PyPIProject = self.get_json(&url)?;
        let wheel_url = find_wheel(&project, sha256).ok_or(TransportError::Metadata)?;
        self.get_bytes(&wheel_url)
    }
}

#[derive(Deserialize)]
struct PyPIProject {
    /// The latest release's files (checked first — the common pin target).
    #[serde(default)]
    urls: Vec<PyPIFile>,
    /// Every historical release's files, keyed by version.
    #[serde(default)]
    releases: std::collections::BTreeMap<String, Vec<PyPIFile>>,
}

#[derive(Deserialize)]
struct PyPIFile {
    /// `bdist_wheel` selects wheels; sdists and other types are skipped.
    packagetype: String,
    url: String,
    digests: Digests,
}

#[derive(Deserialize)]
struct Digests {
    sha256: String,
}

/// Find the wheel URL whose SHA-256 matches `sha256`. The latest release's
/// `urls` are checked first, then every historical release, so a pin can name
/// any published version. Only `bdist_wheel` files are eligible: a pin always
/// addresses a wheel, never an sdist.
fn find_wheel(project: &PyPIProject, sha256: &str) -> Option<String> {
    let is_match = |file: &PyPIFile| {
        file.packagetype == "bdist_wheel" && file.digests.sha256.eq_ignore_ascii_case(sha256)
    };
    project
        .urls
        .iter()
        .chain(project.releases.values().flatten())
        .find(|file| is_match(file))
        .map(|file| file.url.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(packagetype: &str, sha256: &str, url: &str) -> PyPIFile {
        PyPIFile {
            packagetype: packagetype.to_owned(),
            url: url.to_owned(),
            digests: Digests {
                sha256: sha256.to_owned(),
            },
        }
    }

    const PINNED: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn find_wheel_prefers_the_latest_release_then_history() {
        let project = PyPIProject {
            urls: vec![file("bdist_wheel", PINNED, "https://latest")],
            releases: std::collections::BTreeMap::from([(
                "1.0.0".to_owned(),
                vec![file("bdist_wheel", OTHER, "https://old")],
            )]),
        };
        assert_eq!(
            find_wheel(&project, PINNED).as_deref(),
            Some("https://latest"),
            "the latest release must be checked first"
        );
    }

    #[test]
    fn find_wheel_falls_back_to_a_historical_release() {
        let project = PyPIProject {
            urls: vec![file("bdist_wheel", OTHER, "https://latest")],
            releases: std::collections::BTreeMap::from([(
                "0.9.0".to_owned(),
                vec![file("bdist_wheel", PINNED, "https://old")],
            )]),
        };
        assert_eq!(
            find_wheel(&project, PINNED).as_deref(),
            Some("https://old"),
            "a pin on an older version must resolve from history"
        );
    }

    #[test]
    fn find_wheel_skips_sdists_and_unmatched_digests() {
        let project = PyPIProject {
            urls: vec![
                file("sdist", PINNED, "https://sdist"),
                file("bdist_wheel", OTHER, "https://wrong-wheel"),
            ],
            releases: std::collections::BTreeMap::new(),
        };
        assert!(
            find_wheel(&project, PINNED).is_none(),
            "an sdist or a mismatched wheel must never be selected"
        );
    }

    /// [STUBRES-TYPESHED-PYPI]: the distribution name is a path segment of the
    /// index URL, so a name outside the PEP 508 alphabet is refused **before**
    /// a request exists. This runs offline precisely because the rejection
    /// happens ahead of the transport — a name that reached `get_json` would
    /// try to open a socket and this test would not be hermetic.
    #[test]
    fn fetch_wheel_refuses_a_name_outside_the_pep_508_alphabet() {
        let client = PypiClient::new();
        for name in [
            "../../etc/passwd",
            "name/json?x=",
            "name#frag",
            "name%2fjson",
            "",
            ".leading-dot",
            "trailing-dot.",
        ] {
            assert_eq!(
                client.fetch_wheel(name, PINNED).err(),
                Some(TransportError::Metadata),
                "`{name}` must be rejected before any request is built",
            );
        }
    }

    #[test]
    fn find_wheel_matches_the_pin_case_insensitively() {
        let project = PyPIProject {
            urls: vec![file("bdist_wheel", &PINNED.to_uppercase(), "https://wheel")],
            releases: std::collections::BTreeMap::new(),
        };
        assert_eq!(
            find_wheel(&project, PINNED).as_deref(),
            Some("https://wheel"),
            "a pin with different ASCII case must still match"
        );
    }
}
