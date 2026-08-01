//! Production source backend over the embedded bundle, the local store, and
//! user-managed custom trees. Implements the source model and work contract in
//! [TYPESHEDRT-MODEL], [TYPESHEDRT-WORK], and [TYPESHEDRT-SEGREGATION].
//!
//! Everything here is a local read: this crate carries no HTTP client, so the
//! analysis path cannot reach the network even by mistake
//! ([STUBRES-TYPESHED-OFFLINE]). Downloading lives in the separate
//! `basilisk-typeshed-fetch` crate, invoked only by explicit user action.

mod custom;

use std::path::PathBuf;
use std::sync::Arc;

use super::bundle::bundled_snapshot;
use super::gittree::Oid;
use super::manager::TypeshedManager;
use super::selector::{BackendError, SourceBackend};
use super::snapshot::Snapshot;
use super::source::TypeshedRequest;
use super::store::{self, StoreError};

/// Production policy backend shared by CLI/LSP/MCP managers.
#[derive(Debug)]
pub struct RuntimeBackend {
    store_root: Option<PathBuf>,
}

impl RuntimeBackend {
    /// Construct a backend resolving pins from `store_root`, or the per-user
    /// OS default when `None`.
    #[must_use]
    pub const fn new(store_root: Option<PathBuf>) -> Self {
        Self { store_root }
    }
}

impl SourceBackend for RuntimeBackend {
    fn load_custom(&self, path: &str) -> Result<Snapshot, BackendError> {
        custom::load_custom_snapshot(path)
    }

    fn load_pinned(&self, commit: Oid, explicit: bool) -> Result<Snapshot, BackendError> {
        let root = self
            .store_root
            .clone()
            .or_else(default_store_path)
            .ok_or(BackendError::Missing)?;
        store::read_snapshot(&root, commit, explicit).map_err(|error| match error {
            StoreError::Missing => BackendError::Missing,
            StoreError::Corrupt => BackendError::Corrupt,
            StoreError::LicenseChanged => BackendError::LicenseChanged,
        })
    }

    fn load_bundled(&self) -> Result<Snapshot, BackendError> {
        bundled_snapshot().map_err(|_error| BackendError::Bundle)
    }

    fn load_pypi_package(&self, _name: &str, _sha256: &str) -> Result<Snapshot, BackendError> {
        // On-disk SHA-256 verification of an installed PyPI package is the
        // next slice ([STUBRES-TYPESHED-PYPI]); until that backend lands,
        // a package pin fails closed rather than serving unverified bytes.
        Err(BackendError::PyPIPackage)
    }
}

/// Construct the one-generation manager consumed by CLI/LSP/MCP.
#[must_use]
pub fn manager_for_request(
    request: TypeshedRequest,
    backend: Arc<dyn SourceBackend>,
) -> TypeshedManager {
    TypeshedManager::new(request, backend)
}

/// Construct the production manager. Resolution is local and infallible to
/// build — a failing source surfaces from [`TypeshedManager::snapshot`].
#[must_use]
pub fn production_manager(request: TypeshedRequest) -> TypeshedManager {
    let backend = Arc::new(RuntimeBackend::new(request.store_path.clone()));
    manager_for_request(request, backend)
}

/// Canonical per-user typeshed store directory for this platform
/// ([STUBRES-TYPESHED-STORE]).
#[must_use]
pub fn default_store_path() -> Option<PathBuf> {
    platform_cache_base().map(|base| base.join("basilisk").join("typeshed"))
}

#[cfg(target_os = "windows")]
fn platform_cache_base() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn platform_cache_base() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library").join("Caches"))
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn platform_cache_base() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".cache"))
        })
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only resolution fixtures use fixed embedded assets and SHA constants"
)]
mod tests;
