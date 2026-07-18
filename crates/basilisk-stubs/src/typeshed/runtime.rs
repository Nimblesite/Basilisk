//! Production acquisition backend over injected HTTPS transport and disk cache.

mod custom;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use super::archive::{Archive, ArchiveEntry};
use super::bundle::{approved_license_manifest, bundled_snapshot};
use super::cache::{CacheKey, CacheRecord, CachedArchive, CachedTreeFile, DiskCache};
use super::codec::{decode_zip, DecodeLimits, ZipLayout};
use super::gate::manifest::sha256_hex;
use super::gate::{run_activation, GateConfig, GateError, SafetyLimits};
use super::gittree::{git_blob_oid, reconstruct_root_tree_oid, FileMode, GitFile, Oid};
use super::manager::TypeshedManager;
use super::selector::{AcquisitionBackend, BackendError};
use super::snapshot::Snapshot;
use super::source::{
    LicenseStatus, Provenance, SourceIdentity, SourceKind, StatusWarning,
    Transport as SourceTransport, TypeshedRequest, TypeshedStatus,
};
use super::transport::{CommitMetadata, HttpsTransport, Transport, TransportError, TreeEntry};

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrustedTree {
    root: Oid,
    files: BTreeMap<String, TreeEntry>,
}

/// Production policy backend shared by CLI/LSP/MCP managers.
pub struct RuntimeBackend {
    transport: Arc<dyn Transport>,
    cache: Option<DiskCache>,
}

impl std::fmt::Debug for RuntimeBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeBackend")
            .field("cache_enabled", &self.cache.is_some())
            .finish_non_exhaustive()
    }
}

impl RuntimeBackend {
    /// Construct a backend from the concrete HTTPS adapter and optional cache.
    #[must_use]
    pub fn new(transport: Arc<dyn Transport>, cache: Option<DiskCache>) -> Self {
        Self { transport, cache }
    }

    fn load_resolved(
        &self,
        metadata: &CommitMetadata,
        request: &TypeshedRequest,
        pinned: bool,
    ) -> Result<Snapshot, BackendError> {
        if request.use_cache {
            if let Some(cached) = self.load_cache(metadata.commit) {
                let cached_result = validate_cache_record(metadata.commit, metadata.tree, &cached)
                    .and_then(|()| trusted_from_cache(metadata.commit, &cached))
                    .and_then(|trusted| {
                        activate_zip(
                            metadata.commit,
                            &cached.zip,
                            &trusted,
                            request,
                            cached_transport(&cached)?,
                            pinned,
                        )
                    });
                if let Ok(snapshot) = cached_result {
                    return Ok(snapshot);
                }
                tracing::warn!("typeshed cached candidate failed gates; reacquiring archive");
            }
        }
        let retain_tree_metadata =
            request.verify_content || (request.use_cache && self.cache.is_some());
        let trusted = if retain_tree_metadata {
            let entries = self
                .transport
                .fetch_tree(metadata.tree)
                .map_err(|_error| BackendError::Metadata)?;
            trusted_from_entries(metadata.tree, entries)?
        } else {
            TrustedTree {
                root: metadata.tree,
                files: BTreeMap::new(),
            }
        };
        let bytes = self
            .transport
            .fetch_archive(metadata.commit)
            .map_err(|_error| BackendError::Download)?;
        let source_transport = self.transport.archive_transport();
        if !matches!(
            source_transport,
            SourceTransport::Codeload | SourceTransport::Mirror
        ) {
            return Err(BackendError::Validation);
        }
        let snapshot = activate_zip(
            metadata.commit,
            &bytes,
            &trusted,
            request,
            source_transport,
            pinned,
        )?;
        self.store_cache(metadata.commit, &trusted, &bytes, request, source_transport);
        Ok(snapshot)
    }

    fn load_cache(&self, commit: Oid) -> Option<CachedArchive> {
        let Some(cache) = &self.cache else {
            return None;
        };
        match cache.load(&cache_key(commit)) {
            Ok(cached) => cached,
            Err(_error) => {
                // A cache is an optimization, never an alternate trust root.
                // Corrupt/incomplete generations are ignored and acquisition
                // obtains fresh official bytes through the normal gates.
                tracing::warn!("typeshed cache reuse failed; reacquiring archive");
                None
            }
        }
    }

    fn store_cache(
        &self,
        commit: Oid,
        trusted: &TrustedTree,
        zip: &[u8],
        request: &TypeshedRequest,
        source_transport: SourceTransport,
    ) {
        if !request.use_cache {
            return;
        }
        let Some(cache) = &self.cache else {
            return;
        };
        let record = CacheRecord {
            commit: Some(commit.to_hex()),
            tree: Some(trusted.root.to_hex()),
            zip_sha256: sha256_hex(zip),
            verified: request.verify_content,
            transport: Some(transport_label(source_transport).to_owned()),
            tree_files: trusted
                .files
                .values()
                .map(|entry| CachedTreeFile {
                    path: entry.path.clone(),
                    oid: entry.oid.to_hex(),
                    mode: entry.mode.as_str().to_owned(),
                })
                .collect(),
        };
        if cache.store(&cache_key(commit), zip, &record).is_err() {
            tracing::warn!("typeshed cache store failed");
        }
    }
}

impl AcquisitionBackend for RuntimeBackend {
    fn load_custom(&self, path: &str) -> Result<Snapshot, BackendError> {
        custom::load_custom_snapshot(path)
    }

    fn load_commit(
        &self,
        commit: Oid,
        request: &TypeshedRequest,
    ) -> Result<Snapshot, BackendError> {
        if request.use_cache {
            if let Some(cached) = self.load_cache(commit) {
                let cached_result = trusted_from_cache(commit, &cached).and_then(|trusted| {
                    activate_zip(
                        commit,
                        &cached.zip,
                        &trusted,
                        request,
                        cached_transport(&cached)?,
                        true,
                    )
                });
                if let Ok(snapshot) = cached_result {
                    return Ok(snapshot);
                }
                tracing::warn!("typeshed exact cache failed gates; reacquiring metadata");
            }
        }
        let metadata = self
            .transport
            .resolve_commit(commit)
            .map_err(|_error| BackendError::Metadata)?;
        if metadata.commit != commit {
            return Err(BackendError::Metadata);
        }
        self.load_resolved(&metadata, request, true)
    }

    fn load_latest(&self, request: &TypeshedRequest) -> Result<Snapshot, BackendError> {
        // Resolve B before any cache lookup. Therefore cached unpinned A can
        // never be selected when Latest moves or metadata resolution fails.
        let resolved = self
            .transport
            .resolve_latest()
            .map_err(|_error| BackendError::Metadata)?;
        self.load_resolved(&resolved, request, false)
    }

    fn load_bundled(&self) -> Result<Snapshot, BackendError> {
        bundled_snapshot().map_err(|_error| BackendError::Bundle)
    }
}

/// Construct the one-generation manager consumed by CLI/LSP/MCP.
#[must_use]
pub fn manager_for_request(
    request: TypeshedRequest,
    transport: Arc<dyn Transport>,
    cache: Option<DiskCache>,
) -> TypeshedManager {
    TypeshedManager::new(request, Arc::new(RuntimeBackend::new(transport, cache)))
}

/// Construct a production manager with authenticated HTTPS and the canonical
/// per-user OS cache when caching is enabled.
///
/// # Errors
///
/// Returns a redacted transport configuration error for an invalid mirror.
pub fn production_manager(
    request: TypeshedRequest,
    cache_path: Option<PathBuf>,
) -> Result<TypeshedManager, TransportError> {
    let cache = if request.use_cache {
        cache_path.map(DiskCache::new).or_else(default_cache)
    } else {
        None
    };
    let transport = Arc::new(HttpsTransport::new(request.url_template.clone())?);
    Ok(manager_for_request(request, transport, cache))
}

/// Canonical per-user typeshed cache directory for this platform.
#[must_use]
pub fn default_cache_path() -> Option<PathBuf> {
    platform_cache_base().map(|base| base.join("basilisk").join("typeshed"))
}

/// Canonical disk cache, or `None` when the platform exposes no user cache
/// location in the current environment.
#[must_use]
pub fn default_cache() -> Option<DiskCache> {
    default_cache_path().map(DiskCache::new)
}

fn activate_zip(
    commit: Oid,
    zip: &[u8],
    trusted: &TrustedTree,
    request: &TypeshedRequest,
    transport: SourceTransport,
    pinned: bool,
) -> Result<Snapshot, BackendError> {
    let decoded = decode_zip(zip, ZipLayout::CodeloadPrefixed, &DecodeLimits::default())
        .map_err(|_error| BackendError::Validation)?;
    let archive = if request.verify_content {
        bind_trusted_files(decoded, trusted)?
    } else {
        decoded
    };
    let approved = approved_license_manifest().map_err(|_error| BackendError::Bundle)?;
    let identity = SourceIdentity::Commit { commit, pinned };
    let config = GateConfig {
        limits: SafetyLimits::default(),
        approved_license: approved,
        expected_root_tree: trusted.root,
        verify_content: request.verify_content,
    };
    let activation = run_activation(archive, &config, identity.uri_component())
        .map_err(|error| gate_error(&error))?;
    let provenance = if request.verify_content {
        Provenance::GithubTlsAttested
    } else {
        Provenance::Unverified
    };
    let status = TypeshedStatus {
        active_source: if pinned {
            SourceKind::ExactCommit
        } else {
            SourceKind::Latest
        },
        commit: Some(commit),
        tree: activation.root_tree,
        transport,
        license_status: LicenseStatus::Approved,
        license_reference: Some(format!(
            "https://github.com/python/typeshed/blob/{commit}/LICENSE"
        )),
        provenance,
        signed_release: false,
        warnings: StatusWarning::list(&activation.warnings),
    };
    Snapshot::build(identity, status, activation.vfs, None)
        .map_err(|_error| BackendError::Validation)
}

fn bind_trusted_files(archive: Archive, trusted: &TrustedTree) -> Result<Archive, BackendError> {
    if trusted.files.is_empty() {
        return Ok(archive);
    }
    if archive.len() != trusted.files.len() {
        return Err(BackendError::Validation);
    }
    let mut seen = BTreeSet::new();
    let mut entries = Vec::with_capacity(archive.len());
    for entry in archive.entries() {
        let metadata = trusted
            .files
            .get(&entry.path)
            .ok_or(BackendError::Validation)?;
        if !matches!(metadata.mode, FileMode::Regular | FileMode::Executable)
            || git_blob_oid(&entry.data) != metadata.oid
            || !seen.insert(entry.path.clone())
        {
            return Err(BackendError::Validation);
        }
        entries.push(ArchiveEntry {
            path: entry.path.clone(),
            mode: metadata.mode,
            data: entry.data.clone(),
        });
    }
    if seen.len() != trusted.files.len() {
        return Err(BackendError::Validation);
    }
    Ok(Archive::new(entries))
}

fn validate_cache_record(
    commit: Oid,
    tree: Oid,
    cached: &CachedArchive,
) -> Result<(), BackendError> {
    let expected_commit = commit.to_hex();
    let expected_tree = tree.to_hex();
    if cached.record.commit.as_deref() != Some(expected_commit.as_str())
        || cached.record.tree.as_deref() != Some(expected_tree.as_str())
    {
        return Err(BackendError::Validation);
    }
    Ok(())
}

fn trusted_from_cache(commit: Oid, cached: &CachedArchive) -> Result<TrustedTree, BackendError> {
    let expected_commit = commit.to_hex();
    if cached.record.commit.as_deref() != Some(expected_commit.as_str()) {
        return Err(BackendError::Validation);
    }
    let tree = cached
        .record
        .tree
        .as_deref()
        .ok_or(BackendError::Validation)
        .and_then(|value| Oid::from_hex(value).map_err(|_error| BackendError::Validation))?;
    let entries = cached
        .record
        .tree_files
        .iter()
        .map(|file| {
            let oid = Oid::from_hex(&file.oid).map_err(|_error| BackendError::Validation)?;
            let mode = match file.mode.as_str() {
                "100644" => FileMode::Regular,
                "100755" => FileMode::Executable,
                "120000" => FileMode::Symlink,
                "160000" => FileMode::Submodule,
                _ => return Err(BackendError::Validation),
            };
            Ok(TreeEntry {
                path: file.path.clone(),
                oid,
                mode,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if entries.is_empty() {
        return Ok(TrustedTree {
            root: tree,
            files: BTreeMap::new(),
        });
    }
    trusted_from_entries(tree, entries)
}

fn trusted_from_entries(root: Oid, entries: Vec<TreeEntry>) -> Result<TrustedTree, BackendError> {
    let mut files = BTreeMap::new();
    for entry in entries {
        if files.insert(entry.path.clone(), entry).is_some() {
            return Err(BackendError::Validation);
        }
    }
    let git_files: Vec<_> = files
        .values()
        .map(|entry| GitFile {
            path: entry.path.clone(),
            oid: entry.oid,
            mode: entry.mode,
        })
        .collect();
    let reconstructed =
        reconstruct_root_tree_oid(&git_files).map_err(|_error| BackendError::Validation)?;
    if reconstructed != root {
        return Err(BackendError::Validation);
    }
    Ok(TrustedTree { root, files })
}

fn cache_key(commit: Oid) -> CacheKey {
    CacheKey::from_identity(&commit.to_hex())
}

fn cached_transport(cached: &CachedArchive) -> Result<SourceTransport, BackendError> {
    match cached.record.transport.as_deref() {
        Some("codeload") => Ok(SourceTransport::Codeload),
        Some("mirror") => Ok(SourceTransport::Mirror),
        _ => Err(BackendError::Validation),
    }
}

fn transport_label(transport: SourceTransport) -> &'static str {
    match transport {
        SourceTransport::Codeload => "codeload",
        SourceTransport::Mirror => "mirror",
        SourceTransport::CustomPath | SourceTransport::EmbeddedZip => "invalid",
    }
}

fn gate_error(error: &GateError) -> BackendError {
    if matches!(error, GateError::License(_)) {
        BackendError::LicenseChanged
    } else {
        BackendError::Validation
    }
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
    reason = "test-only acquisition fixtures use fixed ZIPs and SHA constants"
)]
mod tests;
